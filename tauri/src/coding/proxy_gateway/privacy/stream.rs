//! Per-logical-channel restoration shared by SSE and Responses WebSocket.
use super::{PrivacyRequest, TOKEN_PREFIX};
use futures_util::{Stream, StreamExt};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::pin::Pin;

const MAX_BUFFER_BYTES: usize = 2 * 1024 * 1024;
type BodyStream = Pin<Box<dyn Stream<Item = Result<Vec<u8>, String>> + Send + 'static>>;

struct Fragment {
    channel: String,
    pointer: String,
    json: bool,
}
struct Tail {
    text: String,
    frame: u64,
    pointer: String,
    json: bool,
    string_scan: Option<JsonStringScan>,
}

struct JsonStringScan {
    offset: usize,
    escaped: bool,
}

struct Frame {
    sequence: u64,
    raw: Vec<u8>,
    value: Option<Value>,
    original: Option<Value>,
    sse: bool,
    data_span: Option<(usize, usize)>,
}

impl Frame {
    fn render(self) -> Result<Vec<u8>, String> {
        if self.value == self.original {
            return Ok(self.raw);
        }
        let value = self.value.ok_or("privacy_event_invalid")?;
        if !self.sse {
            return Ok(value.to_string().into_bytes());
        }
        if let Some((start, end)) = self.data_span {
            let mut output = self.raw[..start].to_vec();
            output.extend_from_slice(value.to_string().as_bytes());
            output.extend_from_slice(&self.raw[end..]);
            return Ok(output);
        }
        let raw = std::str::from_utf8(&self.raw).map_err(|_| "privacy_event_invalid")?;
        let mut output = String::new();
        let mut wrote_data = false;
        for line in raw.split_inclusive('\n') {
            if line.starts_with("data:") {
                if !wrote_data {
                    output.push_str("data: ");
                    output.push_str(&value.to_string());
                    output.push_str(if line.ends_with("\r\n") {
                        "\r\n"
                    } else if line.ends_with('\n') {
                        "\n"
                    } else {
                        ""
                    });
                    wrote_data = true;
                }
            } else {
                output.push_str(line);
            }
        }
        Ok(output.into_bytes())
    }
}

pub(crate) struct EventRestorer {
    request: PrivacyRequest,
    tails: HashMap<String, Tail>,
    frames: VecDeque<Frame>,
    bytes: usize,
    sequence: u64,
    argument_channels: HashSet<String>,
    signed_channels: HashSet<String>,
    changed_channels: HashSet<String>,
}

impl EventRestorer {
    pub(crate) fn new(request: PrivacyRequest) -> Self {
        Self {
            request,
            tails: HashMap::new(),
            frames: VecDeque::new(),
            bytes: 0,
            sequence: 0,
            argument_channels: HashSet::new(),
            signed_channels: HashSet::new(),
            changed_channels: HashSet::new(),
        }
    }

    pub(crate) fn push_json(&mut self, raw: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        self.push(raw.to_vec(), false)
    }

    fn push(&mut self, raw: Vec<u8>, sse: bool) -> Result<Vec<Vec<u8>>, String> {
        let result = self.push_inner(raw, sse);
        if result.is_err() {
            self.request.fail();
        }
        result
    }

    fn push_inner(&mut self, raw: Vec<u8>, sse: bool) -> Result<Vec<Vec<u8>>, String> {
        self.bytes = self.bytes.saturating_add(raw.len());
        if self.bytes > MAX_BUFFER_BYTES {
            return Err("privacy_stream_buffer_limit".into());
        }
        let text = std::str::from_utf8(&raw).map_err(|_| "privacy_event_invalid_utf8")?;
        let mut data = if sse {
            text.lines()
                .filter_map(|line| {
                    line.strip_prefix("data:")
                        .map(|data| data.strip_prefix(' ').unwrap_or(data))
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            text.to_string()
        };
        let mut data_span = None;
        if sse
            && !data.trim().eq("[DONE]")
            && (data.is_empty() || serde_json::from_str::<Value>(&data).is_err())
        {
            if let Some((start, end)) = first_data_span(&raw) {
                if text[end..].trim().is_empty() {
                    data = text[start..end].to_string();
                    data_span = Some((start, end));
                }
            }
        }
        let done = data.trim() == "[DONE]";
        let mut value = if data.is_empty() || done {
            None
        } else {
            Some(serde_json::from_str::<Value>(&data).map_err(|_| "privacy_event_invalid_json")?)
        };
        let original = value.clone();
        if value.is_some() {
            self.request.observe_usage(data.as_bytes());
        }
        let terminal = done
            || value.as_ref().is_some_and(|value| {
                matches!(
                    value.get("type").and_then(Value::as_str),
                    Some(
                        "message_stop"
                            | "response.completed"
                            | "response.failed"
                            | "response.incomplete"
                            | "response.cancelled"
                            | "response.canceled"
                            | "error"
                    )
                ) || value.get("error").is_some_and(|error| match error {
                    Value::Null => false,
                    Value::String(message) => !message.trim().is_empty(),
                    Value::Object(fields) => !fields.is_empty(),
                    Value::Array(items) => !items.is_empty(),
                    _ => true,
                })
            });
        self.sequence += 1;
        if let Some(value) = &mut value {
            self.request.remember_response(value);
            for channel in signed_channels(value) {
                if self.changed_channels.contains(&channel) {
                    return Err(
                        "privacy_signed_payload: signature arrived after changed content".into(),
                    );
                }
                self.signed_channels.insert(channel);
            }
            if self.signed_channels.len() > 1024 || self.changed_channels.len() > 1024 {
                return Err("privacy_stream_channel_limit".into());
            }
            let fragments = fragments(value);
            let strings = fragments
                .iter()
                .map(|fragment| {
                    let field = value
                        .pointer_mut(&fragment.pointer)
                        .expect("discovered fragment");
                    std::mem::replace(field, Value::Null)
                        .as_str()
                        .unwrap_or_default()
                        .to_string()
                })
                .collect::<Vec<_>>();
            self.request.transform_value(value, true)?;
            for (channel, pointer) in gemini_part_paths(value) {
                let pointer = format!("{pointer}/functionCall");
                if value.pointer(&pointer)
                    != original.as_ref().and_then(|value| value.pointer(&pointer))
                {
                    self.changed_channels.insert(channel);
                }
            }
            for (fragment, string) in fragments.into_iter().zip(strings) {
                let (mut combined, string_scan) =
                    if let Some(tail) = self.tails.remove(&fragment.channel) {
                        (tail.text + &string, tail.string_scan)
                    } else {
                        (string, None)
                    };
                let (restored, split, string_scan) = if fragment.json {
                    if self.argument_channels.len() >= 1024
                        && !self.argument_channels.contains(&fragment.channel)
                    {
                        return Err("privacy_stream_channel_limit".into());
                    }
                    self.argument_channels.insert(fragment.channel.clone());
                    restore_argument_fragment(&combined, false, &self.request, string_scan)?
                } else {
                    let split = complete_prefix_len(&combined);
                    (
                        self.request.restore_text(&combined[..split], false)?.0,
                        split,
                        None,
                    )
                };
                if restored != combined[..split] {
                    if self.signed_channels.contains(&fragment.channel)
                        || (fragment.channel.starts_with("anthropic:")
                            && fragment.channel.ends_with(":thinking"))
                    {
                        return Err("privacy_signed_payload: cannot restore signed deltas".into());
                    }
                    self.changed_channels.insert(fragment.channel.clone());
                }
                *value
                    .pointer_mut(&fragment.pointer)
                    .expect("fragment retained") = Value::String(restored);
                if split < combined.len() {
                    combined.drain(..split);
                    self.tails.insert(
                        fragment.channel,
                        Tail {
                            text: combined,
                            frame: self.sequence,
                            pointer: fragment.pointer,
                            json: fragment.json,
                            string_scan,
                        },
                    );
                }
            }
        }
        self.frames.push_back(Frame {
            sequence: self.sequence,
            raw,
            value,
            original,
            sse,
            data_span,
        });
        if terminal {
            self.flush_tails(None)?;
        } else if let Some(value) = self
            .frames
            .back()
            .and_then(|frame| frame.value.as_ref())
            .cloned()
        {
            for (array, finish, prefix) in [
                ("choices", "finish_reason", "chat"),
                ("candidates", "finishReason", "gemini"),
            ] {
                for (position, item) in value
                    .get(array)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    if item
                        .get(finish)
                        .and_then(Value::as_str)
                        .is_some_and(|reason| !reason.is_empty())
                    {
                        let index = item
                            .get("index")
                            .and_then(Value::as_u64)
                            .unwrap_or(position as u64);
                        self.flush_tails(Some(&format!("{prefix}:{index}:")))?;
                    }
                }
            }
            if value.get("type").and_then(Value::as_str) == Some("content_block_stop") {
                self.flush_tails(Some(&format!(
                    "anthropic:{}:",
                    value
                        .get("index")
                        .and_then(Value::as_u64)
                        .unwrap_or_default()
                )))?;
            }
        }
        self.drain_ready()
    }

    fn flush_tails(&mut self, channel_prefix: Option<&str>) -> Result<(), String> {
        for (channel, tail) in &self.tails {
            if channel_prefix.is_some_and(|prefix| !channel.starts_with(prefix)) {
                continue;
            }
            let remainder = if tail.json {
                restore_argument_fragment(&tail.text, true, &self.request, None)?.0
            } else {
                if tail.text.contains(TOKEN_PREFIX) {
                    return Err("privacy_token_incomplete".into());
                }
                tail.text.clone()
            };
            let field = self
                .frames
                .iter_mut()
                .find(|frame| frame.sequence == tail.frame)
                .and_then(|frame| frame.value.as_mut())
                .and_then(|value| value.pointer_mut(&tail.pointer))
                .ok_or("privacy_event_channel_missing")?;
            let string = field
                .as_str()
                .ok_or("privacy_event_channel_invalid")?
                .to_string()
                + &remainder;
            *field = Value::String(string);
        }
        self.tails
            .retain(|channel, _| channel_prefix.is_some_and(|prefix| !channel.starts_with(prefix)));
        Ok(())
    }

    fn drain_ready(&mut self) -> Result<Vec<Vec<u8>>, String> {
        if !self.tails.is_empty() {
            return Ok(Vec::new());
        }
        self.bytes = 0;
        self.frames.drain(..).map(Frame::render).collect()
    }

    pub(crate) fn finish(&mut self) -> Result<Vec<Vec<u8>>, String> {
        let result = self.flush_tails(None).and_then(|()| self.drain_ready());
        if result.is_err() {
            self.request.fail();
        }
        result
    }
}

/// Buffer a JSON string until it can be decoded. Tool values may themselves contain JSON,
/// and a placeholder's underscores can be unicode-escaped across arbitrary event boundaries.
fn restore_argument_fragment(
    text: &str,
    final_fragment: bool,
    request: &PrivacyRequest,
    mut string_scan: Option<JsonStringScan>,
) -> Result<(String, usize, Option<JsonStringScan>), String> {
    let mut output = String::new();
    let mut offset = 0;
    while let Some(start) = text[offset..].find('"') {
        let start = offset + start;
        let prefix = &text[offset..start];
        if prefix.contains(TOKEN_PREFIX) {
            return Err(
                "privacy_tool_arguments_invalid: placeholder is outside a JSON string".into(),
            );
        }
        output.push_str(prefix);
        let scan = string_scan.take().unwrap_or(JsonStringScan {
            offset: 1,
            escaped: false,
        });
        let mut escaped = scan.escaped;
        let end = text[start + scan.offset..]
            .char_indices()
            .find_map(|(index, character)| {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    return Some(start + scan.offset + index + 1);
                }
                None
            });
        let Some(end) = end else {
            if final_fragment {
                return Err("privacy_tool_arguments_invalid: incomplete JSON string".into());
            }
            return Ok((
                output,
                start,
                Some(JsonStringScan {
                    offset: text.len() - start,
                    escaped,
                }),
            ));
        };
        let next = text[end..]
            .chars()
            .find(|character| !character.is_whitespace());
        if next.is_none() && !final_fragment {
            return Ok((
                output,
                start,
                Some(JsonStringScan {
                    offset: end - start - 1,
                    escaped: false,
                }),
            ));
        }
        let literal = &text[start..end];
        let decoded: String = serde_json::from_str(literal)
            .map_err(|_| "privacy_tool_arguments_invalid: invalid JSON string")?;
        let mut value = Value::String(decoded);
        let changed = next != Some(':')
            && super::payload::transform_business(
                &mut value,
                &mut |text, _| request.restore_text(text, false).map(|restored| restored.0),
                true,
            )?;
        if changed {
            output.push_str(&value.to_string());
        } else {
            output.push_str(literal);
        }
        offset = end;
    }
    let remainder = &text[offset..];
    if remainder.contains(TOKEN_PREFIX) {
        return Err("privacy_tool_arguments_invalid: placeholder is outside a JSON string".into());
    }
    let split = if final_fragment {
        remainder.len()
    } else {
        complete_prefix_len(remainder)
    };
    output.push_str(&remainder[..split]);
    Ok((output, offset + split, None))
}

/// Find only an unfinished token/prefix; ordinary braces and completed token suffixes are not held.
fn complete_prefix_len(text: &str) -> usize {
    let mut offset = 0;
    while let Some(start) = text[offset..].find(TOKEN_PREFIX) {
        let start = offset + start;
        let suffix = start + TOKEN_PREFIX.len();
        let Some(end) = text[suffix..].find("__") else {
            return start;
        };
        offset = suffix + end + 2;
    }
    let remainder = &text[offset..];
    for count in (1..TOKEN_PREFIX.len()).rev() {
        if remainder.ends_with(&TOKEN_PREFIX[..count]) {
            return text.len() - count;
        }
    }
    text.len()
}

fn fragments(value: &Value) -> Vec<Fragment> {
    let mut result = Vec::new();
    let mut add = |channel: String, pointer: String, json: bool| {
        if value.pointer(&pointer).is_some_and(Value::is_string) {
            result.push(Fragment {
                channel,
                pointer,
                json,
            });
        }
    };
    if let Some(choices) = value.get("choices").and_then(Value::as_array) {
        for (position, choice) in choices.iter().enumerate() {
            let index = choice
                .get("index")
                .and_then(Value::as_u64)
                .unwrap_or(position as u64);
            for field in ["content", "reasoning_content", "reasoning"] {
                add(
                    format!("chat:{index}:{field}"),
                    format!("/choices/{position}/delta/{field}"),
                    false,
                );
            }
            if let Some(calls) = choice
                .pointer("/delta/tool_calls")
                .and_then(Value::as_array)
            {
                for (call_position, call) in calls.iter().enumerate() {
                    let call_index = call
                        .get("index")
                        .and_then(Value::as_u64)
                        .unwrap_or(call_position as u64);
                    add(format!("chat:{index}:tool:{call_index}"), format!("/choices/{position}/delta/tool_calls/{call_position}/function/arguments"), true);
                }
            }
            add(
                format!("chat:{index}:function"),
                format!("/choices/{position}/delta/function_call/arguments"),
                true,
            );
        }
    }
    let event = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if event == "content_block_start" {
        let index = value
            .get("index")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        for field in ["text", "thinking"] {
            add(
                format!("anthropic:{index}:{field}"),
                format!("/content_block/{field}"),
                false,
            );
        }
    }
    if event == "content_block_delta" {
        let index = value
            .get("index")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        for (field, json) in [("text", false), ("thinking", false), ("partial_json", true)] {
            add(
                format!("anthropic:{index}:{field}"),
                format!("/delta/{field}"),
                json,
            );
        }
    }
    if event.starts_with("response.")
        && event.ends_with(".delta")
        && value.get("delta").is_some_and(Value::is_string)
    {
        let item = value
            .get("item_id")
            .cloned()
            .unwrap_or_else(|| value.get("output_index").cloned().unwrap_or(Value::Null));
        let content = value
            .get("content_index")
            .or_else(|| value.get("summary_index"))
            .cloned()
            .unwrap_or(Value::Null);
        add(
            format!("responses:{event}:{item}:{content}"),
            "/delta".into(),
            event.contains("function_call_arguments"),
        );
    }
    if let Some(candidates) = value.get("candidates").and_then(Value::as_array) {
        for (position, candidate) in candidates.iter().enumerate() {
            let index = candidate
                .get("index")
                .and_then(Value::as_u64)
                .unwrap_or(position as u64);
            if let Some(parts) = candidate
                .pointer("/content/parts")
                .and_then(Value::as_array)
            {
                for (part, _) in parts.iter().enumerate() {
                    add(
                        format!("gemini:{index}:{part}"),
                        format!("/candidates/{position}/content/parts/{part}/text"),
                        false,
                    );
                }
            }
        }
    }
    result
}

fn gemini_part_paths(value: &Value) -> Vec<(String, String)> {
    let mut paths = Vec::new();
    for (position, candidate) in value
        .get("candidates")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let index = candidate
            .get("index")
            .and_then(Value::as_u64)
            .unwrap_or(position as u64);
        for (part, _) in candidate
            .pointer("/content/parts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            paths.push((
                format!("gemini:{index}:{part}"),
                format!("/candidates/{position}/content/parts/{part}"),
            ));
        }
    }
    paths
}

fn signed_channels(value: &Value) -> Vec<String> {
    let mut channels = Vec::new();
    if value.pointer("/content_block/type").and_then(Value::as_str) == Some("thinking")
        || value.pointer("/delta/type").and_then(Value::as_str) == Some("signature_delta")
    {
        channels.push(format!(
            "anthropic:{}:thinking",
            value
                .get("index")
                .and_then(Value::as_u64)
                .unwrap_or_default()
        ));
    }
    for (channel, pointer) in gemini_part_paths(value) {
        if value.pointer(&pointer).is_some_and(|part| {
            part.get("thought").and_then(Value::as_bool) == Some(true)
                || part.get("thoughtSignature").is_some()
        }) {
            channels.push(channel);
        }
    }
    channels
}

fn frame_len(buffer: &[u8]) -> Option<usize> {
    (0..buffer.len()).find_map(|index| {
        if buffer[index..].starts_with(b"\n\n") {
            Some(index + 2)
        } else if buffer[index..].starts_with(b"\r\n\r\n") {
            Some(index + 4)
        } else {
            None
        }
    })
}

fn first_data_span(buffer: &[u8]) -> Option<(usize, usize)> {
    let start = buffer
        .windows(5)
        .enumerate()
        .find(|(index, field)| {
            *field == b"data:" && (*index == 0 || buffer[*index - 1].is_ascii_whitespace())
        })?
        .0
        + 5;
    let start = start + usize::from(buffer.get(start) == Some(&b' '));
    if buffer[start..].starts_with(b"[DONE]") {
        return Some((start, start + 6));
    }
    let mut values = serde_json::Deserializer::from_slice(&buffer[start..]).into_iter::<Value>();
    values.next()?.ok()?;
    Some((start, start + values.byte_offset()))
}

fn flattened_frame_len(buffer: &[u8]) -> Option<usize> {
    let (start, end) = first_data_span(buffer)?;
    let prefix = &buffer[..start];
    let inline_event =
        prefix.windows(6).any(|field| field == b"event:") && !prefix.contains(&b'\n');
    let remainder = buffer[end..]
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .map(|offset| &buffer[end + offset..])
        .unwrap_or_default();
    if inline_event
        || remainder.starts_with(b"data:")
        || remainder.starts_with(b"event:")
        || &buffer[start..end] == b"[DONE]"
    {
        Some(end)
    } else {
        None
    }
}

/// A log copy may consolidate fragments, but must never split matching across tool lanes.
/// Invalid/truncated streams are omitted by the caller instead of preserving raw text.
pub(super) fn redact_log(
    body: &[u8],
    redact: &mut dyn FnMut(&str, bool) -> Result<String, String>,
) -> Result<String, String> {
    let text = std::str::from_utf8(body).map_err(|_| "privacy_log_invalid")?;
    let sse = text.lines().any(|line| line.starts_with("data:"));
    let mut values = Vec::new();
    if sse {
        let mut remaining = body;
        while !remaining.is_empty() {
            let size = frame_len(remaining).ok_or("privacy_log_incomplete")?;
            let frame =
                std::str::from_utf8(&remaining[..size]).map_err(|_| "privacy_log_invalid")?;
            let data = frame
                .lines()
                .filter_map(|line| line.strip_prefix("data:"))
                .collect::<Vec<_>>()
                .join("\n");
            if !data.trim().is_empty() && data.trim() != "[DONE]" {
                values
                    .push(serde_json::from_str::<Value>(&data).map_err(|_| "privacy_log_invalid")?);
            }
            remaining = &remaining[size..];
        }
    } else {
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            values.push(serde_json::from_str::<Value>(line).map_err(|_| "privacy_log_invalid")?);
        }
    }
    let mut channels: HashMap<String, (usize, String, String, bool)> = HashMap::new();
    for (index, value) in values.iter_mut().enumerate() {
        for fragment in fragments(value) {
            let field = value
                .pointer_mut(&fragment.pointer)
                .ok_or("privacy_log_invalid")?;
            let string = field.as_str().unwrap_or_default();
            channels
                .entry(fragment.channel)
                .or_insert_with(|| (index, fragment.pointer, String::new(), fragment.json))
                .2
                .push_str(string);
            *field = Value::String(String::new());
        }
    }
    for (_, (index, pointer, text, json)) in channels {
        let replacement = if json {
            let mut value: Value =
                serde_json::from_str(&text).map_err(|_| "privacy_log_incomplete_arguments")?;
            super::payload::transform_business(&mut value, redact, true)?;
            value.to_string()
        } else {
            redact(&text, false)?
        };
        *values[index]
            .pointer_mut(&pointer)
            .ok_or("privacy_log_invalid")? = Value::String(replacement);
    }
    for value in &mut values {
        super::payload::transform_business(value, redact, true)?;
    }
    // This is a diagnostic JSON copy, not an executable wire replay.
    Ok(serde_json::json!({"privacy_log":"redacted_stream", "events":values}).to_string())
}

pub(crate) fn restore_sse_stream(inner: BodyStream, request: PrivacyRequest) -> BodyStream {
    struct State {
        inner: BodyStream,
        request: PrivacyRequest,
        restorer: EventRestorer,
        buffer: Vec<u8>,
        ready: VecDeque<Vec<u8>>,
        done: bool,
    }
    let state = State {
        inner,
        restorer: EventRestorer::new(request.clone()),
        request,
        buffer: Vec::new(),
        ready: VecDeque::new(),
        done: false,
    };
    Box::pin(futures_util::stream::try_unfold(
        state,
        |mut state| async move {
            loop {
                if let Some(bytes) = state.ready.pop_front() {
                    return Ok(Some((bytes, state)));
                }
                if state.done {
                    return Ok(None);
                }
                match state.inner.next().await {
                    Some(Ok(chunk)) => {
                        // Keep usage already received even when an earlier event in this
                        // same chunk fails restoration before its terminal can be emitted.
                        state.request.observe_usage(&chunk);
                        state.buffer.extend_from_slice(&chunk);
                        while let Some(length) =
                            flattened_frame_len(&state.buffer).or_else(|| frame_len(&state.buffer))
                        {
                            let frame = state.buffer.drain(..length).collect();
                            state.ready.extend(state.restorer.push(frame, true)?);
                        }
                        if state.buffer.len() > MAX_BUFFER_BYTES {
                            state.request.fail();
                            return Err("privacy_stream_buffer_limit".into());
                        }
                    }
                    Some(Err(error)) => return Err(error),
                    None => {
                        if !state.buffer.is_empty() {
                            state.ready.extend(
                                state
                                    .restorer
                                    .push(std::mem::take(&mut state.buffer), true)?,
                            );
                        }
                        state.ready.extend(state.restorer.finish()?);
                        state.done = true;
                    }
                }
            }
        },
    ))
}
