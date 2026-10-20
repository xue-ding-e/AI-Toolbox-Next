//! Request-observability markers for native/hosted tools and gateway rewrites.
//!
//! Everything collected here is persisted **only** in the JSONL request detail
//! (`GatewayRequestLogDetail`). It deliberately never enters
//! `proxy_request_logs`: the request list reads that compact table, and adding
//! columns for these markers would require a migration so that a detail-only
//! diagnostic could be filtered on. The detail dialog and the export are the
//! only consumers.
//!
//! Collection must not depend on body storage: `store_request_body` and
//! `store_response_body` are off by default, so a design that inspected saved
//! bodies after the fact would always report "nothing" for most users. The
//! request side reuses the `Value` already parsed by
//! `build_upstream_body_for_provider`; the response side scans the body it is
//! already parsing, or bounded structured SSE events before response conversion.

use crate::coding::proxy_gateway::request_log::safe_lossy_observation_detail;
use super::http_io::DebugBodyStream;
use crate::coding::proxy_gateway::transformer::AiProtocol;
use crate::coding::proxy_gateway::types::{GatewayNativeToolRecord, GatewayRequestRewrite};
use crate::coding::proxy_gateway::usage_parser::{
    find_sse_block_delimiter, flattened_flush_boundary, for_each_flattened_sse_field,
    parse_sse_data_block,
};
use futures_util::StreamExt;
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// Canonical tool kinds. A single vocabulary shared by every protocol keeps the
/// detail dialog from having to translate per-dialect names.
pub(super) const TOOL_WEB_SEARCH: &str = "web_search";
pub(super) const TOOL_IMAGE_GENERATION: &str = "image_generation";
pub(super) const TOOL_CODE_INTERPRETER: &str = "code_interpreter";
pub(super) const TOOL_COMPUTER_USE: &str = "computer_use";
pub(super) const TOOL_FILE_SEARCH: &str = "file_search";
pub(super) const TOOL_MCP: &str = "mcp";
pub(super) const TOOL_LOCAL_SHELL: &str = "local_shell";

/// Rewrite kinds recorded on the request side.
pub(super) const REWRITE_PROMPT_CACHE_KEY: &str = "prompt_cache_key";
pub(super) const REWRITE_SERVICE_TIER: &str = "service_tier";
pub(super) const REWRITE_CCH_STRIPPED: &str = "billing_cch_stripped";
pub(super) const REWRITE_CAPSULE_EXPANDED: &str = "compaction_capsule_expanded";
pub(super) const REWRITE_COMPACTION_SUMMARIZED: &str = "compaction_summarized";
pub(super) const REWRITE_NATIVE_TOOL_FILTERED: &str = "native_tool_filtered";
pub(super) const REWRITE_LOSSY_CONVERSION: &str = "lossy_conversion";

/// Bounds that keep a malformed or hostile request from bloating the JSONL
/// detail. Both lists are diagnostics, not a ledger.
pub(super) const MAX_TOOL_RECORDS: usize = 16;
pub(super) const MAX_REWRITE_RECORDS: usize = 32;
const MAX_DETAIL_CHARS: usize = 256;
/// Diagnostics never retain a full stream. An oversized, incomplete event is
/// ignored through its next delimiter; later events can still be observed.
const MAX_TOOL_EVENT_BYTES: usize = 256 * 1024;

/// Runtime accumulator handed through request preparation and the response
/// path. `native_tools` is keyed by kind; `rewrites` is append-only and
/// capped.
#[derive(Debug, Clone, Default)]
pub(super) struct RequestObservations {
    pub(super) native_tools: Vec<GatewayNativeToolRecord>,
    pub(super) rewrites: Vec<GatewayRequestRewrite>,
    /// Only bounded canonical kinds cross the streaming/response boundary.
    /// No response payload is retained here, even when body logging is enabled.
    streamed_used_tools: Option<Arc<Mutex<Vec<&'static str>>>>,
}

impl RequestObservations {
    /// Records the tools declared by `body`, interpreted as `protocol`.
    pub(super) fn observe_declared(&mut self, protocol: AiProtocol, body: &Value) {
        self.native_tools = declared_tools(protocol, body);
    }

    /// Marks whether each declared tool survived into the forwarded body.
    ///
    /// Only declared records are updated: a declaration the gateway injected
    /// itself is not part of "what the client asked for", and this pass is
    /// about whether the client's declaration made it through.
    pub(super) fn observe_forwarded(&mut self, protocol: AiProtocol, body: &Value) {
        let forwarded = declared_tools(protocol, body);
        for record in &mut self.native_tools {
            record.forwarded = forwarded
                .iter()
                .any(|forwarded| forwarded.kind == record.kind);
        }
    }

    /// Marks the tools the upstream actually invoked in its reply.
    pub(super) fn observe_used(&mut self, protocol: AiProtocol, body: &Value) {
        for kind in used_tools(protocol, body) {
            self.mark_used(&kind, true);
        }
    }

    /// Streaming counterpart: tool kinds recognized in structured upstream events.
    pub(super) fn observe_used_markers(&mut self, kinds: &[&'static str]) {
        for kind in kinds {
            self.mark_used(kind, true);
        }
    }

    /// Observe bytes on demand, before any provider filter or response conversion.
    /// The original chunks, errors, EOF and polling/backpressure pass through
    /// unchanged; there is no background reader or queue.
    pub(super) fn observe_sse_stream(
        &mut self,
        stream: DebugBodyStream,
        protocol: AiProtocol,
    ) -> DebugBodyStream {
        // Chat (including Ollama NDJSON) has no native hosted-tool vocabulary.
        if protocol == AiProtocol::OpenAiChat {
            return stream;
        }
        let shared = Arc::new(Mutex::new(Vec::new()));
        self.streamed_used_tools = Some(shared.clone());
        let mut scanner = UsedToolScanner::with_protocol(protocol);
        scanner.shared = Some(shared);
        Box::pin(futures_util::stream::unfold(
            (stream, scanner),
            |(mut stream, mut scanner)| async move {
                let next = stream.next().await;
                match &next {
                    Some(Ok(chunk)) => scanner.push_chunk(chunk),
                    Some(Err(_)) | None => scanner.finish(),
                }
                scanner.publish();
                next.map(|chunk| (chunk, (stream, scanner)))
            },
        ))
    }

    /// Merge at the logging boundary, including first-chunk rejection or client
    /// cancellation, which may bypass normal streaming-writer completion.
    pub(super) fn native_tools_snapshot(&self) -> Vec<GatewayNativeToolRecord> {
        let mut snapshot = Self {
            native_tools: self.native_tools.clone(),
            ..Self::default()
        };
        if let Some(shared) = &self.streamed_used_tools {
            let kinds = shared.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            snapshot.observe_used_markers(&kinds);
        }
        snapshot.native_tools
    }

    fn mark_used(&mut self, kind: &str, used: bool) {
        if let Some(record) = self
            .native_tools
            .iter_mut()
            .find(|record| record.kind == kind)
        {
            record.used = used;
            return;
        }
        if self.native_tools.len() >= MAX_TOOL_RECORDS {
            return;
        }
        // A tool call can be observed even when the client never declared it
        // (for example a provider-side default search). Keep it: the point of
        // the marker is what actually ran.
        self.native_tools.push(GatewayNativeToolRecord {
            kind: kind.to_string(),
            declared: false,
            forwarded: false,
            used,
        });
    }

    pub(super) fn push_rewrite(&mut self, kind: &str, detail: Option<String>) {
        if self.rewrites.len() >= MAX_REWRITE_RECORDS {
            return;
        }
        self.rewrites.push(GatewayRequestRewrite {
            kind: kind.to_string(),
            detail: detail.map(|value| {
                if kind == REWRITE_LOSSY_CONVERSION {
                    safe_lossy_observation_detail(&value).to_string()
                } else {
                    truncate(value, MAX_DETAIL_CHARS)
                }
            }),
        });
    }

    /// Records safe structural summaries. The original warnings remain available
    /// to the protocol response header, but their request-derived values must
    /// never enter this body-independent diagnostic log.
    pub(super) fn push_lossy_warnings(&mut self, warnings: &[String]) {
        for warning in warnings {
            self.push_rewrite(REWRITE_LOSSY_CONVERSION, Some(warning.clone()));
        }
    }

    /// Adds a `native_tool_filtered` entry once per tool that was declared but
    /// did not reach the upstream.
    pub(super) fn push_filtered_tools(&mut self) {
        let filtered: Vec<String> = self
            .native_tools
            .iter()
            .filter(|record| record.declared && !record.forwarded)
            .map(|record| record.kind.clone())
            .collect();
        for kind in filtered {
            self.push_rewrite(REWRITE_NATIVE_TOOL_FILTERED, Some(kind));
        }
    }
}

fn truncate(value: String, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value;
    }
    let mut truncated: String = value.chars().take(max_chars).collect();
    truncated.push('…');
    truncated
}

/// Tools **declared** in a request body, in canonical kinds.
///
/// The same table serves the forwarded check: the final upstream body is in the
/// target protocol, so calling this again with `target_protocol` answers "did
/// the declaration survive?".
pub(super) fn declared_tools(protocol: AiProtocol, body: &Value) -> Vec<GatewayNativeToolRecord> {
    let mut kinds: Vec<&'static str> = Vec::new();
    match protocol {
        AiProtocol::OpenAiResponses => {
            for tool in body
                .get("tools")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(tool_type) = tool.get("type").and_then(Value::as_str) else {
                    continue;
                };
                if let Some(kind) = responses_declared_kind(tool_type) {
                    kinds.push(kind);
                }
            }
        }
        AiProtocol::AnthropicMessages => {
            for tool in body
                .get("tools")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(tool_type) = tool.get("type").and_then(Value::as_str) else {
                    continue;
                };
                if let Some(kind) = anthropic_declared_kind(tool_type) {
                    kinds.push(kind);
                }
            }
        }
        AiProtocol::GeminiNative => {
            for tool in body
                .get("tools")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                for (key, kind) in [
                    ("googleSearch", TOOL_WEB_SEARCH),
                    ("google_search", TOOL_WEB_SEARCH),
                    ("codeExecution", TOOL_CODE_INTERPRETER),
                    ("code_execution", TOOL_CODE_INTERPRETER),
                    ("urlContext", TOOL_WEB_SEARCH),
                ] {
                    if tool.get(key).is_some() {
                        kinds.push(kind);
                    }
                }
            }
        }
        AiProtocol::OpenAiChat => {}
    }

    dedupe_records(kinds, |kind| GatewayNativeToolRecord {
        kind: kind.to_string(),
        declared: true,
        forwarded: false,
        used: false,
    })
}

fn responses_declared_kind(tool_type: &str) -> Option<&'static str> {
    match tool_type {
        "web_search" | "web_search_preview" => Some(TOOL_WEB_SEARCH),
        "image_generation" => Some(TOOL_IMAGE_GENERATION),
        "code_interpreter" => Some(TOOL_CODE_INTERPRETER),
        "computer_use_preview" | "computer" => Some(TOOL_COMPUTER_USE),
        "file_search" => Some(TOOL_FILE_SEARCH),
        "mcp" => Some(TOOL_MCP),
        "local_shell" => Some(TOOL_LOCAL_SHELL),
        _ => None,
    }
}

fn anthropic_declared_kind(tool_type: &str) -> Option<&'static str> {
    match tool_type {
        "web_search_20250305" | "web_search_2025_03_05" | "web_search" => Some(TOOL_WEB_SEARCH),
        "mcp" => Some(TOOL_MCP),
        _ => None,
    }
}

/// Tools the upstream **invoked**, read from a reply body in `protocol`.
///
/// Scanned on the upstream (target) body rather than the converted client body:
/// cross-protocol conversion drops hosted-tool markers by design, and the point
/// is what the upstream actually did.
pub(super) fn used_tools(protocol: AiProtocol, body: &Value) -> Vec<&'static str> {
    let mut kinds = Vec::new();
    collect_used_markers(protocol, body, &mut kinds);
    kinds.sort_unstable();
    kinds.dedup();
    kinds
}

fn collect_used_markers(protocol: AiProtocol, value: &Value, kinds: &mut Vec<&'static str>) {
    match value {
        Value::Object(object) => {
            if let Some(item_type) = object.get("type").and_then(Value::as_str) {
                match protocol {
                    AiProtocol::OpenAiResponses => {
                        if let Some(kind) = responses_used_kind(item_type) {
                            kinds.push(kind);
                        }
                    }
                    AiProtocol::AnthropicMessages => {
                        if let Some(kind) = anthropic_used_kind(object, item_type) {
                            kinds.push(kind);
                        }
                    }
                    AiProtocol::GeminiNative | AiProtocol::OpenAiChat => {}
                }
            }
            // Gemini marks search in the candidate, code execution in parts.
            if protocol == AiProtocol::GeminiNative {
                if object.contains_key("groundingMetadata")
                    || object.contains_key("grounding_metadata")
                {
                    kinds.push(TOOL_WEB_SEARCH);
                }
                if object.contains_key("executableCode") || object.contains_key("executable_code") {
                    kinds.push(TOOL_CODE_INTERPRETER);
                }
                if object.contains_key("codeExecutionResult")
                    || object.contains_key("code_execution_result")
                {
                    kinds.push(TOOL_CODE_INTERPRETER);
                }
            }
            for child in object.values() {
                collect_used_markers(protocol, child, kinds);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_used_markers(protocol, item, kinds);
            }
        }
        _ => {}
    }
}

fn responses_used_kind(item_type: &str) -> Option<&'static str> {
    match item_type {
        "web_search_call" => Some(TOOL_WEB_SEARCH),
        "image_generation_call" => Some(TOOL_IMAGE_GENERATION),
        "code_interpreter_call" => Some(TOOL_CODE_INTERPRETER),
        "computer_call" => Some(TOOL_COMPUTER_USE),
        "file_search_call" => Some(TOOL_FILE_SEARCH),
        "mcp_call" | "mcp_list_tools" | "mcp_approval_request" => Some(TOOL_MCP),
        "local_shell_call" => Some(TOOL_LOCAL_SHELL),
        _ => None,
    }
}

fn anthropic_used_kind(
    object: &serde_json::Map<String, Value>,
    item_type: &str,
) -> Option<&'static str> {
    match item_type {
        "web_search_tool_result" => Some(TOOL_WEB_SEARCH),
        "mcp_tool_use" | "mcp_tool_result" => Some(TOOL_MCP),
        // `server_tool_use` is generic; the embedded name says which tool.
        "server_tool_use" => {
            object
                .get("name")
                .and_then(Value::as_str)
                .and_then(|name| match name {
                    "web_search" => Some(TOOL_WEB_SEARCH),
                    _ => None,
                })
        }
        _ => None,
    }
}

fn dedupe_records(
    kinds: Vec<&'static str>,
    build: impl Fn(&'static str) -> GatewayNativeToolRecord,
) -> Vec<GatewayNativeToolRecord> {
    let mut records = Vec::new();
    for kind in kinds {
        if records
            .iter()
            .any(|record: &GatewayNativeToolRecord| record.kind == kind)
        {
            continue;
        }
        if records.len() >= MAX_TOOL_RECORDS {
            break;
        }
        records.push(build(kind));
    }
    records
}

/// Bounded, protocol-aware SSE observer. Reuses the usage collector's physical
/// delimiter and multi-line data parser, but never affects stream delivery.
pub(super) struct UsedToolScanner {
    protocol: AiProtocol,
    buffer: Vec<u8>,
    discarding_oversized_event: bool,
    kinds: Vec<&'static str>,
    shared: Option<Arc<Mutex<Vec<&'static str>>>>,
}

impl UsedToolScanner {
    #[cfg(test)]
    pub(super) fn new() -> Self {
        Self::with_protocol(AiProtocol::OpenAiResponses)
    }

    fn with_protocol(protocol: AiProtocol) -> Self {
        Self {
            protocol,
            buffer: Vec::new(),
            discarding_oversized_event: false,
            kinds: Vec::new(),
            shared: None,
        }
    }

    pub(super) fn push_chunk(&mut self, mut chunk: &[u8]) {
        while !chunk.is_empty() {
            let append_len = chunk.len().min(MAX_TOOL_EVENT_BYTES - self.buffer.len());
            let mut search_from = self.buffer.len().saturating_sub(3);
            self.buffer.extend_from_slice(&chunk[..append_len]);
            chunk = &chunk[append_len..];
            let mut consumed = 0;
            while let Some((position, delimiter_len)) =
                find_sse_block_delimiter(&self.buffer[search_from..])
            {
                let block_end = search_from + position;
                if !self.discarding_oversized_event {
                    Self::observe_block(
                        self.protocol,
                        &self.buffer[consumed..block_end],
                        &mut self.kinds,
                    );
                }
                self.discarding_oversized_event = false;
                consumed = block_end + delimiter_len;
                search_from = consumed;
            }
            self.buffer.drain(..consumed);
            if !self.discarding_oversized_event && self.buffer.len() == MAX_TOOL_EVENT_BYTES {
                // Some supported relays flatten SSE fields onto one line.
                // Flush only complete JSON spans before applying the hard cap.
                // A network/cap boundary may split UTF-8. Complete earlier
                // events are still safe to flush; retain the unfinished tail.
                let utf8_end = std::str::from_utf8(&self.buffer)
                    .map_or_else(|error| error.valid_up_to(), |text| text.len());
                let prefix = &self.buffer[..utf8_end];
                let boundary = if is_flattened_field_line(
                    std::str::from_utf8(prefix).expect("valid UTF-8 prefix"),
                ) {
                    flattened_flush_boundary(prefix)
                } else {
                    0
                };
                Self::observe_block(self.protocol, &self.buffer[..boundary], &mut self.kinds);
                self.buffer.drain(..boundary);
            }
            if self.discarding_oversized_event || self.buffer.len() == MAX_TOOL_EVENT_BYTES {
                // Retain only enough bytes to find a split delimiter. Never
                // treat the tail of an oversized event as a fresh JSON event.
                let keep_from = self.buffer.len().saturating_sub(3);
                self.buffer.drain(..keep_from);
                self.discarding_oversized_event = true;
            }
        }
    }

    fn finish(&mut self) {
        if !self.discarding_oversized_event {
            Self::observe_block(self.protocol, &self.buffer, &mut self.kinds);
        }
        self.buffer.clear();
    }

    fn observe_block(protocol: AiProtocol, block: &[u8], kinds: &mut Vec<&'static str>) {
        let mut observe = |value: &Value| {
            for kind in used_tools(protocol, value) {
                if kinds.len() < MAX_TOOL_RECORDS && !kinds.contains(&kind) {
                    kinds.push(kind);
                }
            }
        };
        if let Some(value) = parse_sse_data_block(block) {
            observe(&value);
        } else {
            let text = String::from_utf8_lossy(block);
            let text = text.trim();
            // Only a flattened field line is eligible for this fallback. A
            // comment, id/retry field or malformed multi-line data block must
            // not turn a quoted "data:" example into a real tool invocation.
            if !is_flattened_field_line(text) {
                return;
            }
            for_each_flattened_sse_field(text, |_, data| {
                if let Ok(value) = serde_json::from_str::<Value>(data) {
                    observe(&value);
                }
                false
            });
        }
    }

    pub(super) fn kinds(&self) -> &[&'static str] {
        &self.kinds
    }

    fn publish(&self) {
        if let Some(shared) = &self.shared {
            *shared.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) =
                self.kinds().to_vec();
        }
    }
}

impl Drop for UsedToolScanner {
    fn drop(&mut self) {
        // Cancellation/first-chunk rejection can drop the wrapper before EOF.
        // Flush only already-received, complete JSON; never poll the upstream.
        self.finish();
        self.publish();
    }
}

fn is_flattened_field_line(text: &str) -> bool {
    let text = text.trim();
    (text.starts_with("event:") || text.starts_with("data:"))
        && !text.contains('\r')
        && !text.contains('\n')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn declared_and_forwarded_track_a_filtered_tool() {
        let original = json!({
            "tools": [
                {"type": "web_search"},
                {"type": "function", "name": "shell"}
            ]
        });
        let forwarded = json!({
            "tools": [
                {"type": "function", "name": "shell"}
            ]
        });
        let mut observations = RequestObservations::default();
        observations.observe_declared(AiProtocol::OpenAiResponses, &original);
        observations.observe_forwarded(AiProtocol::OpenAiResponses, &forwarded);
        observations.push_filtered_tools();

        assert_eq!(observations.native_tools.len(), 1);
        assert_eq!(observations.native_tools[0].kind, TOOL_WEB_SEARCH);
        assert!(observations.native_tools[0].declared);
        assert!(!observations.native_tools[0].forwarded);
        assert_eq!(observations.rewrites.len(), 1);
        assert_eq!(observations.rewrites[0].kind, REWRITE_NATIVE_TOOL_FILTERED);
        assert_eq!(
            observations.rewrites[0].detail.as_deref(),
            Some(TOOL_WEB_SEARCH)
        );
    }

    #[test]
    fn used_markers_are_read_for_each_protocol() {
        let responses = json!({
            "output": [{"type": "web_search_call"}, {"type": "message"}]
        });
        assert_eq!(
            used_tools(AiProtocol::OpenAiResponses, &responses),
            vec![TOOL_WEB_SEARCH]
        );

        let anthropic = json!({
            "content": [
                {"type": "server_tool_use", "name": "web_search"},
                {"type": "web_search_tool_result"}
            ]
        });
        assert_eq!(
            used_tools(AiProtocol::AnthropicMessages, &anthropic),
            vec![TOOL_WEB_SEARCH]
        );

        let gemini = json!({
            "candidates": [{
                "groundingMetadata": {"webSearchQueries": ["rust"]},
                "content": {"parts": [{"executableCode": {"language": "python"}}]}
            }]
        });
        let mut kinds = used_tools(AiProtocol::GeminiNative, &gemini);
        kinds.sort_unstable();
        assert_eq!(kinds, vec![TOOL_CODE_INTERPRETER, TOOL_WEB_SEARCH]);
    }

    #[test]
    fn an_undeclared_call_still_gets_a_record() {
        let reply = json!({"output": [{"type": "web_search_call"}]});
        let mut observations = RequestObservations::default();
        observations.observe_used(AiProtocol::OpenAiResponses, &reply);

        assert_eq!(observations.native_tools.len(), 1);
        assert!(!observations.native_tools[0].declared);
        assert!(observations.native_tools[0].used);
    }

    #[test]
    fn stream_scanner_sees_a_marker_split_across_chunks() {
        let mut scanner = UsedToolScanner::new();
        scanner.push_chunk(b"data: {\"type\":\"web_sea");
        scanner.push_chunk(b"rch_call\"}\n\n");

        assert_eq!(scanner.kinds(), &[TOOL_WEB_SEARCH]);
    }

    #[test]
    fn privacy_lossy_warnings_never_copy_request_values_into_observations() {
        use crate::coding::proxy_gateway::transformer::{check_lossy_conversion, ConversionRoute};

        const SECRET: &str = "synthetic_plain_private_value";
        for (source, target, body) in [
            (
                AiProtocol::OpenAiChat,
                AiProtocol::OpenAiResponses,
                json!({"modalities": ["text", SECRET]}),
            ),
            (
                AiProtocol::OpenAiChat,
                AiProtocol::OpenAiResponses,
                json!({"messages": [{"content": [{"type": SECRET}]}]}),
            ),
            (
                AiProtocol::GeminiNative,
                AiProtocol::OpenAiChat,
                json!({"contents": [{"parts": [{"inlineData": {"mimeType": SECRET}}]}]}),
            ),
        ] {
            let issues = check_lossy_conversion(ConversionRoute::new(source, target), &body);
            assert_eq!(issues.len(), 1);
            let warnings: Vec<String> = issues
                .iter()
                .map(|issue| format!("{}: {}", issue.path, issue.message))
                .collect();
            assert!(warnings[0].contains(SECRET));
            let original_warnings = warnings.clone();
            let mut observations = RequestObservations::default();
            observations.push_lossy_warnings(&warnings);
            assert_eq!(warnings, original_warnings, "wire warnings stay unchanged");
            assert_eq!(observations.rewrites.len(), 1);
            assert_eq!(observations.rewrites[0].kind, REWRITE_LOSSY_CONVERSION);
            assert!(!observations.rewrites[0]
                .detail
                .as_ref()
                .unwrap()
                .contains(SECRET));
        }
    }

    #[test]
    fn stream_scanner_handles_split_utf8_multiline_and_mixed_delimiters() {
        let wire = concat!(
            ": keepalive\r\n\r\n",
            "event: response.output_item.added\n",
            "data: {\"item\":\n",
            "data: {\"type\":\"web_search_call\",\"id\":\"搜索\"}}\n\n",
            "data: {\"item\":{\"type\":\"image_generation_call\"}}\r\n\r\n",
            "data: [DONE]\n\n",
        )
        .as_bytes();
        for chunk_size in [1, 2, 3, 7, 64, wire.len()] {
            let mut scanner = UsedToolScanner::new();
            for chunk in wire.chunks(chunk_size) {
                scanner.push_chunk(chunk);
            }
            assert_eq!(scanner.kinds(), &[TOOL_WEB_SEARCH, TOOL_IMAGE_GENERATION]);
            assert!(scanner.buffer.is_empty());
        }
    }

    #[test]
    fn stream_scanner_ignores_text_and_other_protocol_markers() {
        let text = "web_search_call groundingMetadata event: x data: {\"type\":\"mcp_call\"}";
        for protocol in [
            AiProtocol::OpenAiResponses,
            AiProtocol::AnthropicMessages,
            AiProtocol::GeminiNative,
            AiProtocol::OpenAiChat,
        ] {
            let value = json!({"type": "text", "text": text, "content": text});
            let mut scanner = UsedToolScanner::with_protocol(protocol);
            scanner.push_chunk(format!("data: {value}\n\n").as_bytes());
            scanner.finish();
            assert!(scanner.kinds().is_empty(), "{protocol:?}");
        }
        let mut scanner = UsedToolScanner::with_protocol(AiProtocol::AnthropicMessages);
        scanner.push_chunk(b"data: {\"item\":{\"type\":\"web_search_call\"}}\n\n");
        assert!(scanner.kinds().is_empty());

        let mut scanner = UsedToolScanner::new();
        scanner.push_chunk(
            concat!(
                ": example data: {\"type\":\"web_search_call\"}\n\n",
                "event: comment\n: data: {\"type\":\"mcp_call\"}\n\n",
            )
            .as_bytes(),
        );
        assert!(scanner.kinds().is_empty());
    }

    #[test]
    fn stream_scanner_recognizes_anthropic_and_gemini_native_events() {
        for (protocol, value, expected) in [
            (
                AiProtocol::AnthropicMessages,
                json!({"type": "content_block_start", "content_block": {
                    "type": "server_tool_use", "name": "web_search"}}),
                vec![TOOL_WEB_SEARCH],
            ),
            (
                AiProtocol::GeminiNative,
                json!({"candidates": [{"groundingMetadata": {}, "content": {
                    "parts": [{"executableCode": {"language": "PYTHON"}}]}}]}),
                vec![TOOL_CODE_INTERPRETER, TOOL_WEB_SEARCH],
            ),
        ] {
            let mut scanner = UsedToolScanner::with_protocol(protocol);
            scanner.push_chunk(format!("data: {value}\n\n").as_bytes());
            assert_eq!(scanner.kinds(), expected.as_slice());
        }
    }

    #[test]
    fn stream_scanner_bounds_oversized_events_and_recovers_after_the_delimiter() {
        let oversized = format!(
            "data: {{\"type\":\"web_search_call\",\"payload\":\"{}\"}}",
            "x".repeat(MAX_TOOL_EVENT_BYTES * 2),
        );
        for chunk_size in [7, 4096, oversized.len()] {
            let mut scanner = UsedToolScanner::new();
            for chunk in oversized.as_bytes().chunks(chunk_size) {
                scanner.push_chunk(chunk);
                assert!(scanner.buffer.len() <= MAX_TOOL_EVENT_BYTES);
            }
            assert!(scanner.kinds().is_empty());
            assert!(scanner.discarding_oversized_event);
            scanner.push_chunk(b"\r\n\r");
            scanner.push_chunk(b"\ndata: {\"item\":{\"type\":\"file_search_call\"}}\n\n");
            assert_eq!(scanner.kinds(), &[TOOL_FILE_SEARCH]);
            assert!(!scanner.discarding_oversized_event);
        }

        // A malformed multi-line data value is not a flattened event sequence,
        // even when its first JSON value happens to fit before the hard cap.
        let malformed = format!(
            "event: x\ndata: {{\"type\":\"web_search_call\"}}\ndata: {{\"text\":\"{}\"}}\n\n",
            "x".repeat(MAX_TOOL_EVENT_BYTES),
        );
        let mut scanner = UsedToolScanner::new();
        scanner.push_chunk(malformed.as_bytes());
        scanner.finish();
        assert!(scanner.kinds().is_empty());
    }

    #[test]
    fn stream_scanner_handles_complete_eof_and_flattened_events_without_guessing() {
        let mut scanner = UsedToolScanner::new();
        scanner.push_chunk(b"data: {\"type\":\"web_search_call\"");
        scanner.finish();
        assert!(scanner.kinds().is_empty());

        scanner.push_chunk(b"data: {\"type\":\"file_search_call\"}");
        scanner.finish();
        assert_eq!(scanner.kinds(), &[TOOL_FILE_SEARCH]);

        let event = "event: item data: {\"item\":{\"type\":\"web_search_call\"}} ";
        let flattened = event.repeat(MAX_TOOL_EVENT_BYTES / event.len() + 2);
        scanner.push_chunk(flattened.as_bytes());
        assert!(scanner.buffer.len() < MAX_TOOL_EVENT_BYTES);
        assert!(!scanner.discarding_oversized_event);
        scanner.push_chunk(b"event: item data: {\"item\":{\"type\":\"mcp_call\"}}");
        scanner.finish();
        assert_eq!(scanner.kinds(), &[TOOL_FILE_SEARCH, TOOL_WEB_SEARCH, TOOL_MCP]);
    }

    #[test]
    fn stream_scanner_flushes_flattened_events_when_the_cap_splits_utf8() {
        let first = "event: item data: {\"item\":{\"type\":\"web_search_call\"}} ";
        let text_start = "event: text data: {\"type\":\"text\",\"text\":\"";
        let padding = "x".repeat(MAX_TOOL_EVENT_BYTES - 1 - first.len() - text_start.len());
        let wire = format!(
            "{first}{text_start}{padding}搜\"}} event: item data: {{\"item\":{{\"type\":\"mcp_call\"}}}}"
        );
        let mut scanner = UsedToolScanner::new();
        scanner.push_chunk(&wire.as_bytes()[..MAX_TOOL_EVENT_BYTES]);
        assert_eq!(scanner.kinds(), &[TOOL_WEB_SEARCH]);
        assert!(!scanner.discarding_oversized_event);
        scanner.push_chunk(&wire.as_bytes()[MAX_TOOL_EVENT_BYTES..]);
        scanner.finish();
        assert_eq!(scanner.kinds(), &[TOOL_WEB_SEARCH, TOOL_MCP]);
    }

    #[tokio::test]
    async fn stream_observer_is_lazy_and_preserves_chunks_errors_and_request_markers() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let polls = Arc::new(AtomicUsize::new(0));
        let stream_polls = polls.clone();
        let chunks = vec![
            Ok(b"data: {\"item\":{\"type\":\"web_".to_vec()),
            Ok(b"search_call\"}}\n\n".to_vec()),
            Err("upstream body failed".to_string()),
        ];
        let stream = futures_util::stream::iter(chunks.clone()).inspect(move |_| {
            stream_polls.fetch_add(1, Ordering::SeqCst);
        });
        let mut observations = RequestObservations::default();
        let request = json!({"tools": [{"type": "web_search"}]});
        observations.observe_declared(AiProtocol::OpenAiResponses, &request);
        observations.observe_forwarded(AiProtocol::OpenAiResponses, &request);
        observations.push_rewrite(REWRITE_PROMPT_CACHE_KEY, None);
        let mut stream =
            observations.observe_sse_stream(Box::pin(stream), AiProtocol::OpenAiResponses);
        assert_eq!(polls.load(Ordering::SeqCst), 0);
        assert!(!observations.native_tools_snapshot()[0].used);
        for (index, expected) in chunks.into_iter().enumerate() {
            assert_eq!(stream.next().await, Some(expected));
            assert_eq!(polls.load(Ordering::SeqCst), index + 1);
            assert_eq!(observations.native_tools_snapshot()[0].used, index > 0);
        }
        assert!(stream.next().await.is_none());
        let tools = observations.native_tools_snapshot();
        assert_eq!(tools.len(), 1);
        assert!(tools[0].declared && tools[0].forwarded && tools[0].used);
        assert_eq!(observations.rewrites[0].kind, REWRITE_PROMPT_CACHE_KEY);
    }

    #[tokio::test]
    async fn stream_observer_drop_keeps_received_markers_and_stops_reading() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::task::Poll;

        struct Dropped(Arc<AtomicBool>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        for wire in [
            "data: {\"item\":{\"type\":\"web_search_call\"}}\n\n",
            "event: item data: {\"item\":{\"type\":\"web_search_call\"}}",
        ] {
            let dropped = Arc::new(AtomicBool::new(false));
            let guard = Dropped(dropped.clone());
            let mut sent = false;
            let upstream = futures_util::stream::poll_fn(move |_| {
                let _guard = &guard;
                assert!(!sent, "the observer must not read ahead");
                sent = true;
                Poll::Ready(Some(Ok(wire.as_bytes().to_vec())))
            });
            let mut observations = RequestObservations::default();
            let mut stream =
                observations.observe_sse_stream(Box::pin(upstream), AiProtocol::OpenAiResponses);
            assert!(stream.next().await.unwrap().is_ok());
            drop(stream);
            assert!(dropped.load(Ordering::SeqCst));
            assert!(observations.native_tools_snapshot()[0].used);
        }
    }

    #[test]
    fn rewrite_detail_is_truncated() {
        let mut observations = RequestObservations::default();
        observations.push_rewrite(REWRITE_PROMPT_CACHE_KEY, Some("x".repeat(400)));

        let detail = observations.rewrites[0].detail.as_deref().unwrap();
        assert_eq!(detail.chars().count(), MAX_DETAIL_CHARS + 1);
    }

    #[test]
    fn rewrite_list_is_capped() {
        let mut observations = RequestObservations::default();
        for index in 0..(MAX_REWRITE_RECORDS + 5) {
            observations.push_rewrite(REWRITE_PROMPT_CACHE_KEY, Some(index.to_string()));
        }

        assert_eq!(observations.rewrites.len(), MAX_REWRITE_RECORDS);
    }
}
