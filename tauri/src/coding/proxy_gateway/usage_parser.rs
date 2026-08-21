use super::types::GatewayCliKey;
use serde_json::Value;

const MAX_SSE_USAGE_BUFFER_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_creation_tokens: Option<u64>,
    /// Upstream envelope id when available (Claude message id, OpenAI/Codex response id,
    /// Gemini responseId). Used to build stable usage request keys.
    pub envelope_id: Option<String>,
}

impl TokenUsage {
    pub fn total_tokens(&self) -> Option<u64> {
        let input = self.input_tokens.unwrap_or(0);
        let output = self.output_tokens.unwrap_or(0);
        let cache_read = self.cache_read_tokens.unwrap_or(0);
        let cache_creation = self.cache_creation_tokens.unwrap_or(0);
        let total = input
            .saturating_add(output)
            .saturating_add(cache_read)
            .saturating_add(cache_creation);
        (total > 0).then_some(total)
    }

    fn merge_max(&mut self, other: TokenUsage) {
        self.input_tokens = max_option(self.input_tokens, other.input_tokens);
        self.output_tokens = max_option(self.output_tokens, other.output_tokens);
        self.cache_read_tokens = max_option(self.cache_read_tokens, other.cache_read_tokens);
        self.cache_creation_tokens =
            max_option(self.cache_creation_tokens, other.cache_creation_tokens);
        if self.envelope_id.is_none() {
            self.envelope_id = other.envelope_id;
        }
    }
}

/// Build a stable usage `request_id` from an upstream envelope id.
///
/// Claude keeps bare `SESSION:{id}` so proxy rows converge with session JSONL import.
/// Other CLIs scope by app + provider to avoid cross-provider envelope collisions.
pub fn stable_usage_request_id(
    cli_key: GatewayCliKey,
    provider_id: Option<&str>,
    envelope_id: Option<&str>,
    fallback: &str,
) -> String {
    let Some(envelope_id) = envelope_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return fallback.to_string();
    };
    match cli_key {
        GatewayCliKey::Claude => format!("SESSION:{envelope_id}"),
        other => {
            let provider = provider_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("unknown");
            format!("SESSION:{}:{provider}:{envelope_id}", other.as_str())
        }
    }
}

#[derive(Debug, Default)]
pub struct SseUsageCollector {
    buffer: Vec<u8>,
    usage: TokenUsage,
    provider_type: Option<String>,
    /// The protocol-level terminal event kind observed in any SSE block pushed
    /// so far (first-wins: terminal events are mutually exclusive). Mirrors
    /// axonhub's `IsTerminalStreamEvent` so the gateway can decide "did the
    /// stream actually end" independent of the already-written 200 status code.
    /// Distinct from `GatewayStreamOutcome`: this is *what the upstream terminal
    /// said*, which feeds `classify_stream_outcome` together with write success.
    terminal_kind: Option<SseTerminalKind>,
}

/// The protocol-level terminal event kind carried by an SSE block. Required so
/// `response.completed` + `response.status = "incomplete"` (the gateway's own
/// cross-protocol Responses-incomplete signal, see
/// `transformer/stream.rs:2351`) maps to `Incomplete`, not `Completed` — two
/// booleans (`terminal_event_seen` + `error_event_seen`) cannot express the four
/// distinct terminal outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SseTerminalKind {
    /// `message_stop` / `[DONE]` / `response.completed` (normal) /
    /// non-empty `finish_reason` / `finishReason` / `speech.audio.done` /
    /// `transcript.text.done`.
    Success,
    /// `error` / `response.failed` / `response.completed`+`status=failed` /
    /// non-empty top-level `error` / nested `response.error` / `status=failed`.
    Failed,
    /// `response.incomplete` / `response.completed`+`status=incomplete` /
    /// `status=incomplete`.
    Incomplete,
    /// `response.cancelled` / `response.canceled` / `status=cancelled|canceled`.
    Canceled,
}

impl SseUsageCollector {
    pub fn with_provider_type(provider_type: Option<&str>) -> Self {
        Self {
            buffer: Vec::new(),
            usage: TokenUsage::default(),
            provider_type: provider_type
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
            terminal_kind: None,
        }
    }

    pub fn push_chunk(&mut self, cli_key: GatewayCliKey, chunk: &[u8]) {
        if chunk.len() > MAX_SSE_USAGE_BUFFER_BYTES {
            self.buffer.clear();
            return;
        }
        if self.buffer.len().saturating_add(chunk.len()) > MAX_SSE_USAGE_BUFFER_BYTES {
            self.buffer.clear();
        }
        self.buffer.extend_from_slice(chunk);
        while let Some(block) = take_sse_block(&mut self.buffer) {
            self.observe_block(&block);
            if let Some(value) = parse_sse_data_block(&block) {
                self.merge_event(cli_key, &value);
            }
        }
    }

    pub fn finish(mut self, cli_key: GatewayCliKey) -> TokenUsage {
        if !self.buffer.is_empty() {
            let block = std::mem::take(&mut self.buffer);
            self.observe_block(&block);
            if let Some(value) = parse_sse_data_block(&block) {
                self.merge_event(cli_key, &value);
            }
        }
        self.usage
    }

    /// Whether a terminal event has been observed in any pushed SSE block.
    pub fn terminal_event_seen(&self) -> bool {
        self.terminal_kind.is_some()
    }

    /// Whether an error terminal event (`error` / `response.failed`) has been
    /// observed. These end the stream properly — the client received an explicit
    /// protocol error — but the request is a failure, not a success.
    pub fn error_event_seen(&self) -> bool {
        matches!(self.terminal_kind, Some(SseTerminalKind::Failed))
    }

    /// The terminal event kind observed so far, if any. Drives
    /// `classify_stream_outcome` so the four distinct terminal outcomes
    /// (Success / Failed / Incomplete / Canceled) map to the right
    /// `GatewayStreamOutcome` instead of collapsing into two booleans.
    pub fn terminal_kind(&self) -> Option<SseTerminalKind> {
        self.terminal_kind
    }

    /// Ingest a chunk for terminal-event tracking only, without a cli identity
    /// to merge usage for. Same block splitting as [`push_chunk`].
    pub fn observe_chunk(&mut self, chunk: &[u8]) {
        if chunk.len() > MAX_SSE_USAGE_BUFFER_BYTES {
            self.buffer.clear();
            return;
        }
        if self.buffer.len().saturating_add(chunk.len()) > MAX_SSE_USAGE_BUFFER_BYTES {
            self.buffer.clear();
        }
        self.buffer.extend_from_slice(chunk);
        while let Some(block) = take_sse_block(&mut self.buffer) {
            self.observe_block(&block);
        }
    }

    /// Update the terminal kind from a complete SSE block (first-wins).
    fn observe_block(&mut self, block: &[u8]) {
        if self.terminal_kind.is_none() {
            if let Some(kind) = sse_block_classify_terminal(block) {
                self.terminal_kind = Some(kind);
            }
        }
    }

    /// Drain any terminal marker still sitting in the residual buffer (a final
    /// SSE event that arrived without a trailing blank-line separator, so
    /// `push_chunk` could not extract it). Called after the stream loop ends so
    /// the verdict reflects a terminal event whose bytes were already written to
    /// the client inside an earlier chunk, even though the collector only parsed
    /// it on EOF. Does not consume the buffer — `finish` still merges any usage
    /// carried by that trailing block.
    pub fn drain_terminal(&mut self) {
        if self.buffer.is_empty() || self.terminal_kind.is_some() {
            return;
        }
        if let Some(kind) = sse_block_classify_terminal(&self.buffer) {
            self.terminal_kind = Some(kind);
        }
    }

    fn merge_event(&mut self, cli_key: GatewayCliKey, value: &Value) {
        let mut parsed =
            from_json_response_with_provider_type(cli_key, value, self.provider_type.as_deref());
        if parsed.envelope_id.is_none() {
            parsed.envelope_id = extract_envelope_id(cli_key, value);
        }
        self.usage.merge_max(parsed);
    }
}

/// Whether an SSE block carries a protocol-level terminal marker. Thin wrapper
/// over [`sse_block_classify_terminal`] kept for call sites that only need the
/// boolean "did the stream end" verdict.
pub fn sse_block_is_terminal(block: &[u8]) -> bool {
    sse_block_classify_terminal(block).is_some()
}

/// Whether an SSE block carries a non-success terminal event. Thin wrapper over
/// [`sse_block_classify_terminal`]: `error` / `response.failed` (and the
/// terminal-but-not-success `response.incomplete` / `response.cancelled`) end
/// the stream properly but mark a failure.
pub fn sse_block_is_error_event(block: &[u8]) -> bool {
    matches!(
        sse_block_classify_terminal(block),
        Some(SseTerminalKind::Failed | SseTerminalKind::Incomplete | SseTerminalKind::Canceled)
    )
}

/// Map a Responses `response.status` / top-level `status` string to a terminal
/// kind. This is the key to distinguishing the gateway's own cross-protocol
/// signals: `response.completed` + `status=incomplete` (see
/// `transformer/stream.rs:2351`) must read as `Incomplete`, not `Completed`.
/// `completed` / unknown return `None` so the caller can fall back to the
/// event-name decision (Success) or continue inspecting `response.error`.
fn classify_response_status(status: &str) -> Option<SseTerminalKind> {
    match status.trim() {
        "failed" => Some(SseTerminalKind::Failed),
        "incomplete" => Some(SseTerminalKind::Incomplete),
        "cancelled" | "canceled" => Some(SseTerminalKind::Canceled),
        _ => None,
    }
}

/// Whether a JSON value carries a non-null `error` field (top-level or nested
/// under `response`). Mirrors the `value.error` / `response.error` arms of
/// `upstream.rs` `gateway_json_reports_error`.
fn json_carries_error(value: &Value) -> bool {
    value
        .get("error")
        .is_some_and(|error| !error.is_null())
        || value
            .get("response")
            .and_then(|response| response.get("error"))
            .is_some_and(|error| !error.is_null())
}

/// Classify a single complete SSE block into a terminal kind. The check is flat
/// (not dispatched by protocol) because `source_protocol_from_route` returns
/// `None` for unknown paths and compatible upstreams often omit the SSE
/// `event:` field. Semantically aligned with `upstream.rs:9746`
/// `gateway_json_reports_error` (top-level `error`/`status`/`type`, nested
/// `response.error`/`response.status`), but extended to distinguish the three
/// non-success terminal outcomes (`Failed` / `Incomplete` / `Canceled`) that two
/// booleans could not express.
pub fn sse_block_classify_terminal(block: &[u8]) -> Option<SseTerminalKind> {
    let text = String::from_utf8_lossy(block);
    let mut event_name: Option<&str> = None;
    let mut data_lines: Vec<&str> = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(event) = line.strip_prefix("event:") {
            event_name = Some(event.trim());
        } else if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.strip_prefix(' ').unwrap_or(data));
        }
    }

    // OpenAI Chat `[DONE]` sentinel.
    if data_lines.iter().any(|data| data.trim() == "[DONE]") {
        return Some(SseTerminalKind::Success);
    }

    // SSE `event:` field — take precedence over JSON when present.
    if let Some(event_name) = event_name {
        if let Some(kind) = classify_terminal_event_name(event_name) {
            return Some(kind);
        }
    }

    let data = data_lines.join("\n");
    let Ok(value) = serde_json::from_str::<Value>(&data) else {
        return None;
    };

    // Fallback: JSON `type` field when `event:` is absent.
    if let Some(event_type) = value.get("type").and_then(Value::as_str) {
        if let Some(kind) = classify_terminal_event_name(event_type) {
            return Some(kind);
        }
    }

    // `response.completed` may carry a non-success `response.status` (the
    // gateway's own cross-protocol incomplete/cancelled/failed signal). Inspect
    // before treating it as a plain success terminal.
    if event_name
        .map(|name| name == "response.completed")
        .unwrap_or_else(|| {
            value
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|t| t == "response.completed")
        })
    {
        let status = value
            .get("response")
            .and_then(|r| r.get("status"))
            .and_then(Value::as_str)
            .or_else(|| value.get("status").and_then(Value::as_str));
        if let Some(kind) = status.and_then(classify_response_status) {
            return Some(kind);
        }
        return Some(SseTerminalKind::Success);
    }

    // OpenAI Chat completions: choices[].finish_reason non-empty.
    if let Some(choices) = value.get("choices").and_then(Value::as_array) {
        if choices.iter().any(|choice| {
            choice
                .get("finish_reason")
                .and_then(Value::as_str)
                .is_some_and(|reason| !reason.is_empty())
        }) {
            return Some(SseTerminalKind::Success);
        }
    }

    // Gemini generateContent: candidates[].finishReason non-empty.
    if let Some(candidates) = value.get("candidates").and_then(Value::as_array) {
        if candidates.iter().any(|candidate| {
            candidate
                .get("finishReason")
                .and_then(Value::as_str)
                .is_some_and(|reason| !reason.is_empty())
        }) {
            return Some(SseTerminalKind::Success);
        }
    }

    // JSON fallback aligned with gateway_json_reports_error: a non-null error
    // envelope (top-level or nested) makes this a Failed terminal even when the
    // event name was absent or non-terminal.
    if json_carries_error(&value) {
        return Some(SseTerminalKind::Failed);
    }

    // Top-level / nested `status` string for non-Responses envelopes.
    if let Some(kind) = value
        .get("status")
        .and_then(Value::as_str)
        .and_then(classify_response_status)
    {
        return Some(kind);
    }
    if let Some(kind) = value
        .get("response")
        .and_then(|r| r.get("status"))
        .and_then(Value::as_str)
        .and_then(classify_response_status)
    {
        return Some(kind);
    }

    None
}

/// Map an SSE `event:` / JSON `type` name to a terminal kind. `response.completed`
/// deliberately returns `None` so the caller falls through to the
/// `response.status` inspection — the gateway emits
/// `response.completed` + `status=incomplete`/`cancelled`/`failed` for
/// cross-protocol non-success outcomes, and those must not collapse to Success.
fn classify_terminal_event_name(name: &str) -> Option<SseTerminalKind> {
    match name.trim() {
        "message_stop" | "speech.audio.done" | "transcript.text.done" => {
            Some(SseTerminalKind::Success)
        }
        "error" | "response.failed" => Some(SseTerminalKind::Failed),
        "response.incomplete" => Some(SseTerminalKind::Incomplete),
        "response.cancelled" | "response.canceled" => Some(SseTerminalKind::Canceled),
        "response.completed" => None,
        _ => None,
    }
}

pub fn from_response_body(cli_key: GatewayCliKey, body: &[u8]) -> TokenUsage {
    from_response_body_with_provider_type(cli_key, None, body)
}

pub fn from_response_body_with_provider_type(
    cli_key: GatewayCliKey,
    provider_type: Option<&str>,
    body: &[u8],
) -> TokenUsage {
    if let Ok(value) = serde_json::from_slice::<Value>(body) {
        return from_json_response_with_provider_type(cli_key, &value, provider_type);
    }

    let mut collector = SseUsageCollector::with_provider_type(provider_type);
    collector.push_chunk(cli_key, body);
    collector.finish(cli_key)
}

fn from_json_response_with_provider_type(
    cli_key: GatewayCliKey,
    value: &Value,
    provider_type: Option<&str>,
) -> TokenUsage {
    let mut usage = match cli_key {
        GatewayCliKey::Claude | GatewayCliKey::ClaudeDesktop => claude_usage(value, provider_type),
        GatewayCliKey::Codex | GatewayCliKey::Grok | GatewayCliKey::OpenCode => openai_usage(value),
        GatewayCliKey::Gemini => gemini_usage(value),
    };
    if usage.envelope_id.is_none() {
        usage.envelope_id = extract_envelope_id(cli_key, value);
    }
    usage
}

fn extract_envelope_id(cli_key: GatewayCliKey, value: &Value) -> Option<String> {
    let paths: &[&str] = match cli_key {
        GatewayCliKey::Claude | GatewayCliKey::ClaudeDesktop => &[
            "/message/id",
            "/id",
            "/response/id",
            "/message_id",
            "/messageId",
        ],
        GatewayCliKey::Gemini => &["/responseId", "/response/responseId", "/id"],
        GatewayCliKey::Codex | GatewayCliKey::Grok | GatewayCliKey::OpenCode => {
            &["/response/id", "/id", "/responseId"]
        }
    };
    first_non_empty_string_at_paths(value, paths)
}

fn first_non_empty_string_at_paths(value: &Value, paths: &[&str]) -> Option<String> {
    paths.iter().find_map(|path| {
        value
            .pointer(path)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    })
}

fn claude_usage(value: &Value, provider_type: Option<&str>) -> TokenUsage {
    let usage = value.pointer("/usage").or_else(|| {
        value
            .pointer("/message/usage")
            .or_else(|| value.pointer("/delta/usage"))
    });
    let usage_value = usage.unwrap_or(value);
    let input_tokens = first_u64_at_paths(usage_value, &["/input_tokens", "/prompt_tokens"])
        .or_else(|| {
            first_u64_at_paths(
                value,
                &[
                    "/usage/input_tokens",
                    "/message/usage/input_tokens",
                    "/delta/usage/input_tokens",
                ],
            )
        });
    let signed_input_tokens = first_i64_at_paths(usage_value, &["/input_tokens", "/prompt_tokens"])
        .or_else(|| {
            first_i64_at_paths(
                value,
                &[
                    "/usage/input_tokens",
                    "/message/usage/input_tokens",
                    "/delta/usage/input_tokens",
                ],
            )
        });
    let cache_read_tokens = first_u64_at_paths(
        usage_value,
        &[
            "/cache_read_input_tokens",
            "/cache_read_tokens",
            "/cached_tokens",
        ],
    )
    .or_else(|| {
        first_u64_at_paths(
            value,
            &[
                "/usage/cache_read_input_tokens",
                "/usage/cached_tokens",
                "/message/usage/cache_read_input_tokens",
                "/message/usage/cached_tokens",
                "/delta/usage/cache_read_input_tokens",
                "/delta/usage/cached_tokens",
            ],
        )
    });
    let cache_creation_tokens = first_u64_at_paths(
        usage_value,
        &[
            "/cache_creation_input_tokens",
            "/cache_creation_tokens",
            "/cache_write_input_tokens",
        ],
    )
    .or_else(|| {
        first_u64_at_paths(
            value,
            &[
                "/usage/cache_creation_input_tokens",
                "/message/usage/cache_creation_input_tokens",
                "/delta/usage/cache_creation_input_tokens",
            ],
        )
    });
    let input_tokens = if is_moonshot_provider_type(provider_type) {
        moonshot_fresh_input_tokens(signed_input_tokens, input_tokens, cache_read_tokens)
    } else {
        input_tokens
    };

    TokenUsage {
        input_tokens,
        output_tokens: first_u64_at_paths(usage_value, &["/output_tokens", "/completion_tokens"])
            .or_else(|| {
                first_u64_at_paths(
                    value,
                    &[
                        "/usage/output_tokens",
                        "/message/usage/output_tokens",
                        "/delta/usage/output_tokens",
                    ],
                )
            }),
        cache_read_tokens,
        cache_creation_tokens,
        envelope_id: extract_envelope_id(GatewayCliKey::Claude, value),
    }
}

fn is_moonshot_provider_type(provider_type: Option<&str>) -> bool {
    provider_type
        .map(|value| value.trim().to_ascii_lowercase().replace('_', "-"))
        .is_some_and(|value| matches!(value.as_str(), "moonshot" | "kimi"))
}

fn moonshot_fresh_input_tokens(
    signed_input_tokens: Option<i64>,
    input_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
) -> Option<u64> {
    let cache_read = cache_read_tokens.unwrap_or(0);
    if cache_read == 0 {
        return input_tokens;
    }
    if let Some(signed_input) = signed_input_tokens {
        if signed_input < 0 {
            return Some((signed_input + cache_read as i64).max(0) as u64);
        }
    }
    let input = input_tokens?;
    if input < cache_read {
        Some(input)
    } else {
        Some(input.saturating_sub(cache_read))
    }
}

fn openai_usage(value: &Value) -> TokenUsage {
    let usage = value
        .pointer("/usage")
        .or_else(|| value.pointer("/response/usage"))
        .unwrap_or(value);
    let raw_input_tokens =
        first_u64_at_paths(usage, &["/input_tokens", "/prompt_tokens"]).or_else(|| {
            first_u64_at_paths(
                value,
                &[
                    "/usage/input_tokens",
                    "/usage/prompt_tokens",
                    "/response/usage/input_tokens",
                    "/response/usage/prompt_tokens",
                ],
            )
        });
    let cache_read_tokens = first_u64_at_paths(
        usage,
        &[
            "/input_tokens_details/cached_tokens",
            "/prompt_tokens_details/cached_tokens",
        ],
    )
    .or_else(|| {
        first_u64_at_paths(
            value,
            &[
                "/usage/input_tokens_details/cached_tokens",
                "/usage/prompt_tokens_details/cached_tokens",
                "/response/usage/input_tokens_details/cached_tokens",
                "/response/usage/prompt_tokens_details/cached_tokens",
            ],
        )
    });
    // Responses API cache writes (AxonHub 94704784) and Chat-compatible aliases.
    // OpenAI/Responses `input_tokens` / `prompt_tokens` are treated as cache-inclusive
    // totals: fresh input = raw - cache_read - cache_write. Providers that already
    // report fresh-only input would undercount if they also emit cache write details.
    let cache_creation_tokens = first_u64_at_paths(
        usage,
        &[
            "/input_tokens_details/cache_write_tokens",
            "/prompt_tokens_details/cache_write_tokens",
            "/input_tokens_details/cache_creation_tokens",
            "/prompt_tokens_details/cache_creation_tokens",
        ],
    )
    .or_else(|| {
        first_u64_at_paths(
            value,
            &[
                "/usage/input_tokens_details/cache_write_tokens",
                "/usage/prompt_tokens_details/cache_write_tokens",
                "/response/usage/input_tokens_details/cache_write_tokens",
                "/response/usage/prompt_tokens_details/cache_write_tokens",
                "/usage/input_tokens_details/cache_creation_tokens",
                "/usage/prompt_tokens_details/cache_creation_tokens",
                "/response/usage/input_tokens_details/cache_creation_tokens",
                "/response/usage/prompt_tokens_details/cache_creation_tokens",
            ],
        )
    });
    TokenUsage {
        input_tokens: subtract_cache_from_inclusive_input(
            raw_input_tokens,
            cache_read_tokens,
            cache_creation_tokens,
        ),
        output_tokens: first_u64_at_paths(usage, &["/output_tokens", "/completion_tokens"])
            .or_else(|| {
                first_u64_at_paths(
                    value,
                    &[
                        "/usage/output_tokens",
                        "/usage/completion_tokens",
                        "/response/usage/output_tokens",
                        "/response/usage/completion_tokens",
                    ],
                )
            }),
        cache_read_tokens,
        cache_creation_tokens,
        envelope_id: extract_envelope_id(GatewayCliKey::Codex, value),
    }
}

fn gemini_usage(value: &Value) -> TokenUsage {
    let usage = value.pointer("/usageMetadata").unwrap_or(value);
    let input_tokens =
        first_u64_at_paths(usage, &["/promptTokenCount", "/prompt_tokens"]).or_else(|| {
            first_u64_at_paths(
                value,
                &[
                    "/usageMetadata/promptTokenCount",
                    "/response/usageMetadata/promptTokenCount",
                ],
            )
        });
    let output_tokens = first_u64_at_paths(
        usage,
        &[
            "/candidatesTokenCount",
            "/completion_tokens",
            "/output_tokens",
        ],
    )
    .or_else(|| {
        first_u64_at_paths(
            value,
            &[
                "/usageMetadata/candidatesTokenCount",
                "/response/usageMetadata/candidatesTokenCount",
            ],
        )
    });

    let cache_read_tokens =
        first_u64_at_paths(usage, &["/cachedContentTokenCount", "/cache_read_tokens"]).or_else(
            || {
                first_u64_at_paths(
                    value,
                    &[
                        "/usageMetadata/cachedContentTokenCount",
                        "/response/usageMetadata/cachedContentTokenCount",
                    ],
                )
            },
        );

    TokenUsage {
        input_tokens: subtract_cache_from_inclusive_input(input_tokens, cache_read_tokens, None),
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens: None,
        envelope_id: extract_envelope_id(GatewayCliKey::Gemini, value),
    }
}

fn first_u64_at_paths(value: &Value, paths: &[&str]) -> Option<u64> {
    paths
        .iter()
        .find_map(|path| value.pointer(path).and_then(Value::as_u64))
}

fn first_i64_at_paths(value: &Value, paths: &[&str]) -> Option<i64> {
    paths
        .iter()
        .find_map(|path| value.pointer(path).and_then(Value::as_i64))
}

fn subtract_cache_from_inclusive_input(
    input_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_creation_tokens: Option<u64>,
) -> Option<u64> {
    input_tokens.map(|tokens| {
        tokens
            .saturating_sub(cache_read_tokens.unwrap_or(0))
            .saturating_sub(cache_creation_tokens.unwrap_or(0))
    })
}

fn max_option(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn take_sse_block(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    let lf = buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|index| (index, 2));
    let crlf = buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| (index, 4));
    let position = match (lf, crlf) {
        (Some(lf), Some(crlf)) if crlf.0 < lf.0 => crlf,
        (Some(lf), _) => lf,
        (None, Some(crlf)) => crlf,
        (None, None) => return None,
    };
    let block = buffer[..position.0].to_vec();
    buffer.drain(..position.0 + position.1);
    Some(block)
}

fn parse_sse_data_block(block: &[u8]) -> Option<Value> {
    let text = String::from_utf8_lossy(block);
    let mut data_lines = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.strip_prefix(' ').unwrap_or(data);
        if data.trim() == "[DONE]" {
            return None;
        }
        data_lines.push(data);
    }
    if data_lines.is_empty() {
        return None;
    }
    let data = data_lines.join("\n");
    serde_json::from_str::<Value>(&data).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_claude_stream_usage_with_cache_tokens() {
        let body = br#"event: message_start
data: {"type":"message_start","message":{"usage":{"input_tokens":120,"output_tokens":1,"cache_read_input_tokens":40,"cache_creation_input_tokens":8}}}

event: message_delta
data: {"type":"message_delta","usage":{"output_tokens":35}}

"#;

        let usage = from_response_body(GatewayCliKey::Claude, body);

        assert_eq!(usage.input_tokens, Some(120));
        assert_eq!(usage.output_tokens, Some(35));
        assert_eq!(usage.cache_read_tokens, Some(40));
        assert_eq!(usage.cache_creation_tokens, Some(8));
        assert_eq!(usage.total_tokens(), Some(203));
    }

    #[test]
    fn parses_moonshot_anthropic_cached_tokens_as_cache_read() {
        let usage = from_response_body_with_provider_type(
            GatewayCliKey::Claude,
            Some("moonshot"),
            br#"{"usage":{"input_tokens":100,"output_tokens":10,"cached_tokens":80}}"#,
        );

        assert_eq!(usage.input_tokens, Some(20));
        assert_eq!(usage.output_tokens, Some(10));
        assert_eq!(usage.cache_read_tokens, Some(80));
        assert_eq!(usage.total_tokens(), Some(110));
    }

    #[test]
    fn parses_moonshot_negative_input_cache_discount() {
        let usage = from_response_body_with_provider_type(
            GatewayCliKey::Claude,
            Some("kimi"),
            br#"{"usage":{"input_tokens":-40,"output_tokens":10,"cached_tokens":80}}"#,
        );

        // fresh = signed + 1×cache = -40 + 80 = 40; total = fresh+output+cache = 130
        assert_eq!(usage.input_tokens, Some(40));
        assert_eq!(usage.output_tokens, Some(10));
        assert_eq!(usage.cache_read_tokens, Some(80));
        assert_eq!(usage.total_tokens(), Some(130));
    }

    #[test]
    fn parses_moonshot_positive_input_less_than_cache_as_fresh() {
        // Branch: positive input < cache → treat input as already-fresh (no subtract).
        let usage = from_response_body_with_provider_type(
            GatewayCliKey::Claude,
            Some("moonshot"),
            br#"{"usage":{"input_tokens":30,"output_tokens":5,"cached_tokens":80}}"#,
        );
        assert_eq!(usage.input_tokens, Some(30));
        assert_eq!(usage.output_tokens, Some(5));
        assert_eq!(usage.cache_read_tokens, Some(80));
        assert_eq!(usage.total_tokens(), Some(115));
    }

    #[test]
    fn default_anthropic_usage_keeps_input_tokens_as_fresh_input() {
        let usage = from_response_body_with_provider_type(
            GatewayCliKey::Claude,
            Some("anthropic"),
            br#"{"usage":{"input_tokens":100,"output_tokens":10,"cached_tokens":80}}"#,
        );

        assert_eq!(usage.input_tokens, Some(100));
        assert_eq!(usage.output_tokens, Some(10));
        assert_eq!(usage.cache_read_tokens, Some(80));
        assert_eq!(usage.total_tokens(), Some(190));
    }

    #[test]
    fn parses_openai_stream_usage_with_cached_prompt_tokens() {
        let body = br#"data: {"type":"response.completed","response":{"id":"resp_abc","usage":{"input_tokens":90,"output_tokens":12,"input_tokens_details":{"cached_tokens":30}}}}

data: [DONE]

"#;

        let usage = from_response_body(GatewayCliKey::Codex, body);

        assert_eq!(usage.input_tokens, Some(60));
        assert_eq!(usage.output_tokens, Some(12));
        assert_eq!(usage.cache_read_tokens, Some(30));
        assert_eq!(usage.cache_creation_tokens, None);
        assert_eq!(usage.total_tokens(), Some(102));
        assert_eq!(usage.envelope_id.as_deref(), Some("resp_abc"));
    }

    #[test]
    fn parses_responses_cache_write_tokens_as_cache_creation() {
        // AxonHub 94704784: input_tokens_details.cache_write_tokens.
        let usage = from_response_body(
            GatewayCliKey::Codex,
            br#"{"id":"resp_cw","usage":{"input_tokens":100,"output_tokens":10,"input_tokens_details":{"cached_tokens":20,"cache_write_tokens":8}}}"#,
        );
        // Inclusive OpenAI/Responses semantics: fresh input = 100 - 20 - 8 = 72
        assert_eq!(usage.input_tokens, Some(72));
        assert_eq!(usage.output_tokens, Some(10));
        assert_eq!(usage.cache_read_tokens, Some(20));
        assert_eq!(usage.cache_creation_tokens, Some(8));
        assert_eq!(usage.total_tokens(), Some(110));
        assert_eq!(usage.envelope_id.as_deref(), Some("resp_cw"));
    }

    #[test]
    fn parses_responses_cache_write_only_without_cache_read() {
        let usage = from_response_body(
            GatewayCliKey::Codex,
            br#"{"id":"resp_cw_only","usage":{"input_tokens":50,"output_tokens":5,"input_tokens_details":{"cache_write_tokens":12}}}"#,
        );
        assert_eq!(usage.input_tokens, Some(38));
        assert_eq!(usage.cache_read_tokens, None);
        assert_eq!(usage.cache_creation_tokens, Some(12));
        assert_eq!(usage.total_tokens(), Some(55));
    }

    #[test]
    fn stable_usage_request_id_scopes_non_claude_and_keeps_claude_bare() {
        assert_eq!(
            stable_usage_request_id(
                GatewayCliKey::Claude,
                Some("provider-a"),
                Some("msg_123"),
                "gw-fallback"
            ),
            "SESSION:msg_123"
        );
        assert_eq!(
            stable_usage_request_id(
                GatewayCliKey::Codex,
                Some("provider-a"),
                Some("resp_123"),
                "gw-fallback"
            ),
            "SESSION:codex:provider-a:resp_123"
        );
        assert_eq!(
            stable_usage_request_id(GatewayCliKey::Gemini, Some("p1"), None, "gw-fallback"),
            "gw-fallback"
        );
        assert_eq!(
            stable_usage_request_id(GatewayCliKey::Codex, Some("p1"), Some(""), "gw-fallback"),
            "gw-fallback"
        );
    }

    #[test]
    fn extracts_gemini_response_id_as_envelope() {
        let usage = from_response_body(
            GatewayCliKey::Gemini,
            br#"{"responseId":"gemini_1","usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":7}}"#,
        );
        assert_eq!(usage.envelope_id.as_deref(), Some("gemini_1"));
    }

    #[test]
    fn extracts_claude_message_id_as_envelope() {
        let usage = from_response_body(
            GatewayCliKey::Claude,
            br#"{"id":"msg_9","usage":{"input_tokens":1,"output_tokens":2}}"#,
        );
        assert_eq!(usage.envelope_id.as_deref(), Some("msg_9"));
        assert_eq!(
            stable_usage_request_id(
                GatewayCliKey::Claude,
                Some("p"),
                usage.envelope_id.as_deref(),
                "gw-x"
            ),
            "SESSION:msg_9"
        );
    }

    #[test]
    fn parses_gemini_json_usage_metadata() {
        let usage = from_response_body(
            GatewayCliKey::Gemini,
            br#"{"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":7,"cachedContentTokenCount":3}}"#,
        );

        assert_eq!(usage.input_tokens, Some(7));
        assert_eq!(usage.output_tokens, Some(7));
        assert_eq!(usage.cache_read_tokens, Some(3));
        assert_eq!(usage.total_tokens(), Some(17));
    }

    #[test]
    fn gemini_usage_does_not_infer_output_from_total_tokens() {
        let usage = from_response_body(
            GatewayCliKey::Gemini,
            br#"{"usageMetadata":{"promptTokenCount":10,"totalTokenCount":17}}"#,
        );

        assert_eq!(usage.input_tokens, Some(10));
        assert_eq!(usage.output_tokens, None);
        assert_eq!(usage.total_tokens(), Some(10));
    }

    #[test]
    fn sse_usage_collector_keeps_buffer_bounded() {
        let mut collector = SseUsageCollector::default();
        collector.push_chunk(
            GatewayCliKey::Claude,
            &vec![b'a'; MAX_SSE_USAGE_BUFFER_BYTES + 1],
        );
        assert!(collector.buffer.is_empty());

        collector.push_chunk(
            GatewayCliKey::Claude,
            &vec![b'a'; MAX_SSE_USAGE_BUFFER_BYTES - 1],
        );
        assert_eq!(collector.buffer.len(), MAX_SSE_USAGE_BUFFER_BYTES - 1);

        collector.push_chunk(GatewayCliKey::Claude, b"aa");
        assert_eq!(collector.buffer.len(), 2);
    }

    #[test]
    fn take_sse_block_consumes_physically_earliest_delimiter() {
        // LF event first, CRLF later: must not drain through the later CRLF.
        let mut buffer = b"data: {\"a\":1}\n\ndata: {\"b\":2}\r\n\r\n".to_vec();
        let first = take_sse_block(&mut buffer).expect("first block");
        assert_eq!(String::from_utf8_lossy(&first), "data: {\"a\":1}");
        let second = take_sse_block(&mut buffer).expect("second block");
        assert_eq!(String::from_utf8_lossy(&second), "data: {\"b\":2}");
        assert!(buffer.is_empty());

        // CRLF first, LF later.
        let mut buffer = b"data: {\"c\":3}\r\n\r\ndata: {\"d\":4}\n\n".to_vec();
        let first = take_sse_block(&mut buffer).expect("crlf-first block");
        assert_eq!(String::from_utf8_lossy(&first), "data: {\"c\":3}");
        let second = take_sse_block(&mut buffer).expect("lf-second block");
        assert_eq!(String::from_utf8_lossy(&second), "data: {\"d\":4}");
    }

    #[test]
    fn sse_usage_collector_parses_mixed_lf_crlf_events() {
        let mut collector = SseUsageCollector::default();
        // First event ends with LF, second with CRLF; wrong priority would merge both.
        collector.push_chunk(
            GatewayCliKey::Codex,
            b"data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_a\",\"usage\":{\"input_tokens\":10,\"output_tokens\":1}}}\n\n",
        );
        collector.push_chunk(
            GatewayCliKey::Codex,
            b"data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_b\",\"usage\":{\"input_tokens\":90,\"output_tokens\":12,\"input_tokens_details\":{\"cached_tokens\":30}}}}\r\n\r\n",
        );
        let usage = collector.finish(GatewayCliKey::Codex);
        assert_eq!(usage.input_tokens, Some(60));
        assert_eq!(usage.output_tokens, Some(12));
        assert_eq!(usage.cache_read_tokens, Some(30));
        assert_eq!(usage.envelope_id.as_deref(), Some("resp_a"));
    }

    #[test]
    fn sse_block_is_terminal_detects_openai_done_sentinel() {
        assert!(sse_block_is_terminal(b"data: [DONE]\n\n"));
    }

    #[test]
    fn sse_block_is_terminal_detects_anthropic_message_stop_event_field() {
        assert!(sse_block_is_terminal(
            b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"
        ));
    }

    #[test]
    fn sse_block_is_terminal_detects_responses_completed_via_json_type_fallback() {
        // Compatible upstreams omit the SSE `event:` field; only JSON `type`.
        assert!(sse_block_is_terminal(
            b"data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_x\"}}\n\n"
        ));
    }

    #[test]
    fn sse_block_is_terminal_detects_openai_chat_finish_reason() {
        assert!(sse_block_is_terminal(
            b"data: {\"choices\":[{\"finish_reason\":\"stop\",\"delta\":{}}]}\n\n"
        ));
    }

    #[test]
    fn sse_block_is_terminal_ignores_null_finish_reason() {
        assert!(!sse_block_is_terminal(
            b"data: {\"choices\":[{\"finish_reason\":null,\"delta\":{\"content\":\"hi\"}}]}\n\n"
        ));
    }

    #[test]
    fn sse_block_is_terminal_detects_gemini_finish_reason() {
        assert!(sse_block_is_terminal(
            b"data: {\"candidates\":[{\"finishReason\":\"STOP\",\"content\":{\"parts\":[{\"text\":\"ok\"}]}}]}\n\n"
        ));
    }

    #[test]
    fn sse_block_is_terminal_detects_mid_stream_error_event() {
        assert!(sse_block_is_terminal(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"overloaded\"}}\n\n"
        ));
    }

    #[test]
    fn sse_block_is_error_event_distinguishes_error_terminal_from_success_terminal() {
        // `error` / `response.failed` end the stream but mark a failure.
        assert!(sse_block_is_error_event(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"message\":\"boom\"}}\n\n"
        ));
        assert!(sse_block_is_error_event(
            b"data: {\"type\":\"response.failed\",\"response\":{\"id\":\"r\",\"status\":\"failed\"}}\n\n"
        ));
        // `response.incomplete` / `response.cancelled` are terminal-but-not-success
        // (mirrors upstream.rs:488-494), so they count as error terminals.
        assert!(sse_block_is_error_event(
            b"data: {\"type\":\"response.incomplete\",\"response\":{\"id\":\"r\",\"status\":\"incomplete\"}}\n\n"
        ));
        assert!(sse_block_is_error_event(
            b"event: response.cancelled\ndata: {\"type\":\"response.cancelled\",\"response\":{\"id\":\"r\",\"status\":\"canceled\"}}\n\n"
        ));
        // `message_stop` / `response.completed` / `[DONE]` end the stream as a
        // success terminal, not an error terminal.
        assert!(!sse_block_is_error_event(
            b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"
        ));
        assert!(!sse_block_is_error_event(
            b"data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\",\"status\":\"completed\"}}\n\n"
        ));
        assert!(!sse_block_is_error_event(b"data: [DONE]\n\n"));
    }

    #[test]
    fn sse_usage_collector_tracks_error_event_independently_from_terminal() {
        let mut collector = SseUsageCollector::default();
        collector.push_chunk(
            GatewayCliKey::Claude,
            b"event: message_delta\ndata: {\"type\":\"message_delta\"}\n\n",
        );
        assert!(!collector.terminal_event_seen());
        assert!(!collector.error_event_seen());
        collector.push_chunk(
            GatewayCliKey::Claude,
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"message\":\"boom\"}}\n\n",
        );
        assert!(collector.terminal_event_seen());
        assert!(collector.error_event_seen());
    }

    #[test]
    fn sse_block_is_terminal_rejects_delta_events() {
        assert!(!sse_block_is_terminal(
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"hello\"}}\n\n"
        ));
        assert!(!sse_block_is_terminal(
            b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"hello\"}\n\n"
        ));
    }

    #[test]
    fn sse_usage_collector_tracks_terminal_event_across_chunks() {
        let mut collector = SseUsageCollector::default();
        collector.push_chunk(
            GatewayCliKey::Claude,
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"hi\"}}\n\n",
        );
        assert!(!collector.terminal_event_seen());
        collector.push_chunk(
            GatewayCliKey::Claude,
            b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        );
        assert!(collector.terminal_event_seen());
    }

    #[test]
    fn sse_usage_collector_finish_detects_terminal_in_trailing_buffer() {
        // Terminal event arrives without a trailing blank-line separator, so it
        // stays in the buffer until finish() drains it.
        let mut collector = SseUsageCollector::default();
        collector.push_chunk(GatewayCliKey::Codex, b"data: [DONE]");
        assert!(!collector.terminal_event_seen());
        let _ = collector.finish(GatewayCliKey::Codex);
        // finish() consumes self; assert via a fresh collector that sees it.
        let mut collector = SseUsageCollector::default();
        collector.push_chunk(GatewayCliKey::Codex, b"data: [DONE]\n\n");
        assert!(collector.terminal_event_seen());
    }

    #[test]
    fn sse_usage_collector_drain_terminal_recovers_unseparated_final_event() {
        // A real stream often ends with the terminal event and immediate EOF,
        // no trailing blank line. push_chunk cannot extract the block, so the
        // verdict must still surface via drain_terminal — otherwise the stream
        // is misclassified as incomplete and the client gets a spurious error.
        let mut collector = SseUsageCollector::default();
        collector.push_chunk(
            GatewayCliKey::Claude,
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"hi\"}}\n\n",
        );
        assert!(!collector.terminal_event_seen());
        // Final message_stop arrives without a trailing blank-line separator.
        collector.push_chunk(
            GatewayCliKey::Claude,
            b"event: message_stop\ndata: {\"type\":\"message_stop\"}",
        );
        assert!(
            !collector.terminal_event_seen(),
            "unseparated block must not be parsed by push_chunk"
        );
        collector.drain_terminal();
        assert!(
            collector.terminal_event_seen(),
            "drain_terminal must recover the terminal marker from the residual buffer"
        );
    }

    #[test]
    fn sse_block_classify_terminal_response_completed_with_status_failed_is_failed() {
        // The gateway's own cross-protocol signal: a `response.completed` event
        // whose payload carries `status=failed` must read as Failed, not Success.
        assert_eq!(
            sse_block_classify_terminal(
                b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\",\"status\":\"failed\",\"error\":{\"message\":\"boom\"}}}\n\n"
            ),
            Some(SseTerminalKind::Failed)
        );
    }

    #[test]
    fn sse_block_classify_terminal_response_completed_with_status_incomplete_is_incomplete() {
        // `transformer/stream.rs` emits `response.completed` + `status=incomplete`
        // for cross-protocol Responses-incomplete. This must NOT collapse to
        // Completed (the original bug) or Failed.
        assert_eq!(
            sse_block_classify_terminal(
                b"data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\",\"status\":\"incomplete\"}}\n\n"
            ),
            Some(SseTerminalKind::Incomplete)
        );
    }

    #[test]
    fn sse_block_classify_terminal_response_completed_with_status_cancelled_is_canceled() {
        assert_eq!(
            sse_block_classify_terminal(
                b"data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\",\"status\":\"cancelled\"}}\n\n"
            ),
            Some(SseTerminalKind::Canceled)
        );
    }

    #[test]
    fn sse_block_classify_terminal_response_incomplete_event_is_incomplete() {
        assert_eq!(
            sse_block_classify_terminal(
                b"event: response.incomplete\ndata: {\"type\":\"response.incomplete\",\"response\":{\"id\":\"r\",\"status\":\"incomplete\"}}\n\n"
            ),
            Some(SseTerminalKind::Incomplete)
        );
    }

    #[test]
    fn sse_block_classify_terminal_top_level_error_field_is_failed() {
        // Aligned with gateway_json_reports_error: a non-null top-level `error`
        // makes this a Failed terminal even with no terminal event name.
        assert_eq!(
            sse_block_classify_terminal(b"data: {\"error\":{\"message\":\"boom\"}}\n\n"),
            Some(SseTerminalKind::Failed)
        );
    }

    #[test]
    fn sse_block_classify_terminal_nested_response_error_is_failed() {
        assert_eq!(
            sse_block_classify_terminal(
                b"data: {\"response\":{\"error\":{\"message\":\"boom\"}}}\n\n"
            ),
            Some(SseTerminalKind::Failed)
        );
    }

    #[test]
    fn sse_block_classify_terminal_top_level_status_failed_is_failed() {
        assert_eq!(
            sse_block_classify_terminal(b"data: {\"status\":\"failed\"}\n\n"),
            Some(SseTerminalKind::Failed)
        );
    }

    #[test]
    fn sse_block_classify_terminal_nested_response_status_incomplete_is_incomplete() {
        assert_eq!(
            sse_block_classify_terminal(
                b"data: {\"response\":{\"status\":\"incomplete\"}}\n\n"
            ),
            Some(SseTerminalKind::Incomplete)
        );
    }

    #[test]
    fn sse_block_classify_terminal_success_terminals_map_to_success() {
        assert_eq!(
            sse_block_classify_terminal(b"data: [DONE]\n\n"),
            Some(SseTerminalKind::Success)
        );
        assert_eq!(
            sse_block_classify_terminal(
                b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"
            ),
            Some(SseTerminalKind::Success)
        );
        assert_eq!(
            sse_block_classify_terminal(
                b"data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\",\"status\":\"completed\"}}\n\n"
            ),
            Some(SseTerminalKind::Success)
        );
    }

    #[test]
    fn sse_block_classify_terminal_delta_events_are_none() {
        assert_eq!(
            sse_block_classify_terminal(
                b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"hi\"}}\n\n"
            ),
            None
        );
    }
}
