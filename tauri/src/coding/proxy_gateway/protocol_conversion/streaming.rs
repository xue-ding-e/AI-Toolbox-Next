use super::sse::{
    append_utf8_safe, parse_sse_block, sse_done, sse_event, take_sse_block, ParsedSseBlock,
};
use super::types::{AiProtocol, ConversionRoute};
use futures_util::{stream, Stream, StreamExt};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::pin::Pin;

const GEMINI_SYNTHETIC_TOOL_ID_PREFIX: &str = "gemini_synth_";

pub type ConversionByteStream =
    Pin<Box<dyn Stream<Item = Result<Vec<u8>, String>> + Send + 'static>>;

pub fn convert_sse_stream(
    route: ConversionRoute,
    inner: ConversionByteStream,
) -> ConversionByteStream {
    if route.identity() {
        return inner;
    }
    let state = StreamConversionState {
        inner,
        parser_buffer: String::new(),
        utf8_remainder: Vec::new(),
        pending: VecDeque::new(),
        converter: SseEventConverter::new(route),
        source_finished: false,
    };
    Box::pin(stream::unfold(state, |mut state| async move {
        loop {
            if let Some(output) = state.pending.pop_front() {
                return Some((output, state));
            }
            if state.source_finished {
                return None;
            }
            match state.inner.next().await {
                Some(Ok(chunk)) => {
                    append_utf8_safe(&mut state.parser_buffer, &mut state.utf8_remainder, &chunk);
                    while let Some(block) = take_sse_block(&mut state.parser_buffer) {
                        for output in state.converter.convert_block(&block) {
                            state.pending.push_back(Ok(output));
                        }
                    }
                }
                Some(Err(error)) => return Some((Err(error), state)),
                None => {
                    state.source_finished = true;
                    for output in state.converter.finish() {
                        state.pending.push_back(Ok(output));
                    }
                }
            }
        }
    }))
}

struct StreamConversionState {
    inner: ConversionByteStream,
    parser_buffer: String,
    utf8_remainder: Vec<u8>,
    pending: VecDeque<Result<Vec<u8>, String>>,
    converter: SseEventConverter,
    source_finished: bool,
}

struct SseEventConverter {
    route: ConversionRoute,
    sent_start: bool,
    opened_text: bool,
    finished: bool,
    sent_chat_finish_delta: bool,
    message_id: String,
    model: String,
    anthropic_next_block_index: usize,
    anthropic_open_block: Option<AnthropicOpenBlock>,
    anthropic_stop_reason: Option<String>,
    openai_stream_tools: BTreeMap<usize, OpenAiStreamToolCall>,
    anthropic_source_tools: HashMap<usize, AnthropicSourceToolBlock>,
    anthropic_source_next_tool_index: usize,
    responses_stream_tools: HashMap<String, ResponsesStreamToolCall>,
    responses_next_tool_index: usize,
    responses_saw_tool_call: bool,
    gemini_accumulated_text: String,
    gemini_tool_calls: Vec<GeminiStreamToolCall>,
    gemini_latest_usage: Option<Value>,
    gemini_latest_finish_reason: Option<String>,
    gemini_blocked_text: Option<String>,
    anthropic_current_tool: Option<AnthropicGeminiToolBlock>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnthropicOpenBlockKind {
    Text,
    Thinking,
    Tool,
}

#[derive(Debug, Clone)]
struct AnthropicOpenBlock {
    index: usize,
    kind: AnthropicOpenBlockKind,
    tool_source_index: Option<usize>,
}

#[derive(Debug, Clone)]
struct OpenAiStreamToolCall {
    id: String,
    name: String,
    arguments: String,
    block_index: Option<usize>,
}

#[derive(Debug, Clone)]
struct AnthropicSourceToolBlock {
    tool_index: usize,
    id: String,
    name: String,
    arguments: String,
}

#[derive(Debug, Clone)]
struct ResponsesStreamToolCall {
    call_id: String,
    arguments: String,
    tool_index: usize,
    block_index: Option<usize>,
}

#[derive(Debug, Clone)]
struct GeminiStreamToolCall {
    id: String,
    name: String,
    args: Value,
}

#[derive(Debug, Clone)]
struct AnthropicGeminiToolBlock {
    id: String,
    name: String,
    partial_json: String,
}

impl SseEventConverter {
    fn new(route: ConversionRoute) -> Self {
        Self {
            route,
            sent_start: false,
            opened_text: false,
            finished: false,
            sent_chat_finish_delta: false,
            message_id: String::new(),
            model: String::new(),
            anthropic_next_block_index: 0,
            anthropic_open_block: None,
            anthropic_stop_reason: None,
            openai_stream_tools: BTreeMap::new(),
            anthropic_source_tools: HashMap::new(),
            anthropic_source_next_tool_index: 0,
            responses_stream_tools: HashMap::new(),
            responses_next_tool_index: 0,
            responses_saw_tool_call: false,
            gemini_accumulated_text: String::new(),
            gemini_tool_calls: Vec::new(),
            gemini_latest_usage: None,
            gemini_latest_finish_reason: None,
            gemini_blocked_text: None,
            anthropic_current_tool: None,
        }
    }

    fn convert_block(&mut self, block: &str) -> Vec<Vec<u8>> {
        let parsed = parse_sse_block(block);
        if parsed.data.trim().is_empty() {
            return Vec::new();
        }
        if parsed.data.trim() == "[DONE]" {
            return self.finish();
        }
        let Ok(value) = serde_json::from_str::<Value>(&parsed.data) else {
            return Vec::new();
        };

        match (self.route.source, self.route.target) {
            (AiProtocol::OpenAiChat, AiProtocol::AnthropicMessages) => {
                self.chat_chunk_to_anthropic(value)
            }
            (AiProtocol::OpenAiChat, AiProtocol::OpenAiResponses) => {
                self.chat_chunk_to_responses(value)
            }
            (AiProtocol::AnthropicMessages, AiProtocol::OpenAiChat) => {
                self.anthropic_event_to_chat(parsed, value)
            }
            (AiProtocol::AnthropicMessages, AiProtocol::OpenAiResponses) => {
                self.anthropic_event_to_responses(parsed, value)
            }
            (AiProtocol::OpenAiResponses, AiProtocol::AnthropicMessages) => {
                self.responses_event_to_anthropic(parsed, value)
            }
            (AiProtocol::OpenAiResponses, AiProtocol::OpenAiChat) => {
                self.responses_event_to_chat(parsed, value)
            }
            (AiProtocol::GeminiNative, AiProtocol::AnthropicMessages) => {
                self.gemini_chunk_to_anthropic(value)
            }
            (AiProtocol::AnthropicMessages, AiProtocol::GeminiNative) => {
                self.anthropic_event_to_gemini(parsed, value)
            }
            _ => Vec::new(),
        }
    }

    fn finish(&mut self) -> Vec<Vec<u8>> {
        if self.finished {
            return Vec::new();
        }
        self.finished = true;

        match self.route.target {
            AiProtocol::AnthropicMessages => {
                if self.route.source == AiProtocol::GeminiNative {
                    return self.finish_gemini_to_anthropic();
                }
                let mut out = Vec::new();
                self.close_anthropic_open_block(&mut out);
                if self.sent_start {
                    let stop_reason = self.anthropic_stop_reason.as_deref().unwrap_or("end_turn");
                    out.push(sse_event(
                        Some("message_delta"),
                        &json!({
                            "type": "message_delta",
                            "delta": {"stop_reason": stop_reason, "stop_sequence": null},
                            "usage": {"output_tokens": 0}
                        }),
                    ));
                    out.push(sse_event(
                        Some("message_stop"),
                        &json!({"type": "message_stop"}),
                    ));
                }
                out
            }
            AiProtocol::OpenAiChat => {
                let mut out = Vec::new();
                if self.sent_start && !self.sent_chat_finish_delta {
                    self.sent_chat_finish_delta = true;
                    let finish_reason = if self.responses_saw_tool_call {
                        "tool_calls"
                    } else {
                        "stop"
                    };
                    out.push(self.chat_delta(json!({}), Some(finish_reason)));
                }
                out.push(sse_done());
                out
            }
            AiProtocol::OpenAiResponses => {
                if self.sent_start {
                    vec![sse_event(
                        Some("response.completed"),
                        &json!({
                            "type": "response.completed",
                            "response": {
                                "id": self.message_id,
                                "object": "response",
                                "status": "completed",
                                "model": self.model,
                                "output": [],
                                "usage": null
                            }
                        }),
                    )]
                } else {
                    Vec::new()
                }
            }
            AiProtocol::GeminiNative => self.finish_anthropic_to_gemini(),
        }
    }

    fn ensure_anthropic_message_start(&mut self, out: &mut Vec<Vec<u8>>) {
        if self.sent_start {
            return;
        }
        self.sent_start = true;
        out.push(sse_event(
            Some("message_start"),
            &json!({
                "type": "message_start",
                "message": {
                    "id": if self.message_id.is_empty() { "msg_gateway" } else { &self.message_id },
                    "type": "message",
                    "role": "assistant",
                    "model": self.model,
                    "content": [],
                    "usage": {"input_tokens": 0, "output_tokens": 0}
                }
            }),
        ));
    }

    fn ensure_anthropic_text_block(&mut self, out: &mut Vec<Vec<u8>>) -> usize {
        if let Some(block) = &self.anthropic_open_block {
            if block.kind == AnthropicOpenBlockKind::Text {
                return block.index;
            }
        }
        self.close_anthropic_open_block(out);
        self.ensure_anthropic_message_start(out);
        let index = self.anthropic_next_block_index;
        self.anthropic_next_block_index += 1;
        self.anthropic_open_block = Some(AnthropicOpenBlock {
            index,
            kind: AnthropicOpenBlockKind::Text,
            tool_source_index: None,
        });
        out.push(sse_event(
            Some("content_block_start"),
            &json!({
                "type": "content_block_start",
                "index": index,
                "content_block": {"type": "text", "text": ""}
            }),
        ));
        index
    }

    fn ensure_anthropic_thinking_block(&mut self, out: &mut Vec<Vec<u8>>) -> usize {
        if let Some(block) = &self.anthropic_open_block {
            if block.kind == AnthropicOpenBlockKind::Thinking {
                return block.index;
            }
        }
        self.close_anthropic_open_block(out);
        self.ensure_anthropic_message_start(out);
        let index = self.anthropic_next_block_index;
        self.anthropic_next_block_index += 1;
        self.anthropic_open_block = Some(AnthropicOpenBlock {
            index,
            kind: AnthropicOpenBlockKind::Thinking,
            tool_source_index: None,
        });
        out.push(sse_event(
            Some("content_block_start"),
            &json!({
                "type": "content_block_start",
                "index": index,
                "content_block": {"type": "thinking", "thinking": ""}
            }),
        ));
        index
    }

    fn ensure_anthropic_tool_block(
        &mut self,
        out: &mut Vec<Vec<u8>>,
        source_index: usize,
        id: &str,
        name: &str,
    ) -> usize {
        if let Some(block) = &self.anthropic_open_block {
            if block.kind == AnthropicOpenBlockKind::Tool
                && block.tool_source_index == Some(source_index)
            {
                return block.index;
            }
        }
        self.close_anthropic_open_block(out);
        self.ensure_anthropic_message_start(out);
        let index = self.anthropic_next_block_index;
        self.anthropic_next_block_index += 1;
        self.anthropic_open_block = Some(AnthropicOpenBlock {
            index,
            kind: AnthropicOpenBlockKind::Tool,
            tool_source_index: Some(source_index),
        });
        out.push(sse_event(
            Some("content_block_start"),
            &json!({
                "type": "content_block_start",
                "index": index,
                "content_block": {
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": {}
                }
            }),
        ));
        index
    }

    fn close_anthropic_open_block(&mut self, out: &mut Vec<Vec<u8>>) {
        let Some(block) = self.anthropic_open_block.take() else {
            return;
        };
        out.push(sse_event(
            Some("content_block_stop"),
            &json!({"type": "content_block_stop", "index": block.index}),
        ));
    }

    fn chat_chunk_to_anthropic(&mut self, value: Value) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        self.capture_openai_id_model(&value);
        self.ensure_anthropic_message_start(&mut out);
        let Some(choice) = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
        else {
            return out;
        };
        let delta = choice
            .get("delta")
            .unwrap_or(choice.get("message").unwrap_or(&Value::Null));
        if let Some(reasoning) = delta
            .get("reasoning_content")
            .or_else(|| delta.get("reasoning"))
            .and_then(Value::as_str)
        {
            if !reasoning.is_empty() {
                let index = self.ensure_anthropic_thinking_block(&mut out);
                out.push(sse_event(
                    Some("content_block_delta"),
                    &json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {"type": "thinking_delta", "thinking": reasoning}
                    }),
                ));
            }
        }
        if let Some(content) = delta.get("content").and_then(Value::as_str) {
            if !content.is_empty() {
                let index = self.ensure_anthropic_text_block(&mut out);
                out.push(sse_event(
                    Some("content_block_delta"),
                    &json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {"type": "text_delta", "text": content}
                    }),
                ));
            }
        }
        if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for tool_call in tool_calls {
                self.append_openai_tool_call_as_anthropic(tool_call, &mut out);
            }
        }
        if let Some(function_call) = delta.get("function_call") {
            self.append_legacy_function_call_as_anthropic(function_call, &mut out);
        }
        if let Some(finish_reason) = choice
            .get("finish_reason")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            self.anthropic_stop_reason = Some(
                chat_finish_to_anthropic_stop(finish_reason, !self.openai_stream_tools.is_empty())
                    .to_string(),
            );
            out.extend(self.finish());
        }
        out
    }

    fn append_openai_tool_call_as_anthropic(&mut self, tool_call: &Value, out: &mut Vec<Vec<u8>>) {
        let source_index = tool_call
            .get("index")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or_else(|| self.openai_stream_tools.len());
        let function = tool_call.get("function").unwrap_or(tool_call);
        let incoming_id = tool_call
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty());
        let incoming_name = function
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty());
        let incoming_arguments = function
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or_default();

        let (id, name, block_index) = {
            let state = self
                .openai_stream_tools
                .entry(source_index)
                .or_insert_with(|| OpenAiStreamToolCall {
                    id: incoming_id
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("call_{source_index}")),
                    name: incoming_name.unwrap_or_default().to_string(),
                    arguments: String::new(),
                    block_index: None,
                });
            if let Some(id) = incoming_id {
                state.id = id.to_string();
            }
            if let Some(name) = incoming_name {
                state.name = name.to_string();
            }
            state.arguments.push_str(incoming_arguments);
            (state.id.clone(), state.name.clone(), state.block_index)
        };

        let index = match block_index {
            Some(index) => {
                if self
                    .anthropic_open_block
                    .as_ref()
                    .is_none_or(|block| block.index != index)
                {
                    self.close_anthropic_open_block(out);
                }
                index
            }
            None => {
                let index = self.ensure_anthropic_tool_block(out, source_index, &id, &name);
                if let Some(state) = self.openai_stream_tools.get_mut(&source_index) {
                    state.block_index = Some(index);
                }
                index
            }
        };

        if !incoming_arguments.is_empty() {
            out.push(sse_event(
                Some("content_block_delta"),
                &json!({
                    "type": "content_block_delta",
                    "index": index,
                    "delta": {
                        "type": "input_json_delta",
                        "partial_json": incoming_arguments
                    }
                }),
            ));
        }
    }

    fn append_legacy_function_call_as_anthropic(
        &mut self,
        function_call: &Value,
        out: &mut Vec<Vec<u8>>,
    ) {
        let tool_call = json!({
            "index": 0,
            "id": function_call.get("id").and_then(Value::as_str).unwrap_or("function_call"),
            "function": {
                "name": function_call.get("name").and_then(Value::as_str).unwrap_or_default(),
                "arguments": function_call.get("arguments").and_then(Value::as_str).unwrap_or_default(),
            }
        });
        self.append_openai_tool_call_as_anthropic(&tool_call, out);
    }

    fn chat_chunk_to_responses(&mut self, value: Value) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        self.capture_openai_id_model(&value);
        if !self.sent_start {
            self.sent_start = true;
            out.push(sse_event(
                Some("response.created"),
                &json!({
                    "type": "response.created",
                    "response": {
                        "id": self.message_id,
                        "object": "response",
                        "status": "in_progress",
                        "model": self.model,
                        "output": []
                    }
                }),
            ));
        }
        if let Some(choice) = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
        {
            let delta = choice.get("delta").unwrap_or(&Value::Null);
            if let Some(reasoning) = delta
                .get("reasoning_content")
                .or_else(|| delta.get("reasoning"))
                .and_then(Value::as_str)
            {
                if !reasoning.is_empty() {
                    out.push(sse_event(
                        Some("response.reasoning_summary_text.delta"),
                        &json!({
                            "type": "response.reasoning_summary_text.delta",
                            "delta": reasoning,
                            "item_id": if self.message_id.is_empty() { "rs_gateway" } else { &self.message_id },
                            "output_index": 0,
                            "summary_index": 0
                        }),
                    ));
                }
            }
            if let Some(content) = delta.get("content").and_then(Value::as_str) {
                if !content.is_empty() {
                    out.push(sse_event(
                        Some("response.output_text.delta"),
                        &json!({
                            "type": "response.output_text.delta",
                            "delta": content,
                            "item_id": self.message_id,
                            "output_index": 0,
                            "content_index": 0
                        }),
                    ));
                }
            }
            if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
                for tool_call in tool_calls {
                    self.append_openai_tool_call_as_responses(tool_call, &mut out);
                }
            }
            if let Some(function_call) = delta.get("function_call") {
                let tool_call = json!({
                    "index": 0,
                    "id": function_call.get("id").and_then(Value::as_str).unwrap_or("function_call"),
                    "function": {
                        "name": function_call.get("name").and_then(Value::as_str).unwrap_or_default(),
                        "arguments": function_call.get("arguments").and_then(Value::as_str).unwrap_or_default(),
                    }
                });
                self.append_openai_tool_call_as_responses(&tool_call, &mut out);
            }
            if choice
                .get("finish_reason")
                .is_some_and(|value| !value.is_null())
            {
                out.extend(self.finish());
            }
        }
        out
    }

    fn append_openai_tool_call_as_responses(&mut self, tool_call: &Value, out: &mut Vec<Vec<u8>>) {
        let source_index = tool_call
            .get("index")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or_else(|| self.openai_stream_tools.len());
        let function = tool_call.get("function").unwrap_or(tool_call);
        let incoming_id = tool_call
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty());
        let incoming_name = function
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty());
        let incoming_arguments = function
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or_default();

        let mut emit_added = false;
        let (id, name) = {
            let state = self
                .openai_stream_tools
                .entry(source_index)
                .or_insert_with(|| {
                    emit_added = true;
                    OpenAiStreamToolCall {
                        id: incoming_id
                            .map(str::to_string)
                            .unwrap_or_else(|| format!("call_{source_index}")),
                        name: incoming_name.unwrap_or_default().to_string(),
                        arguments: String::new(),
                        block_index: None,
                    }
                });
            if let Some(id) = incoming_id {
                state.id = id.to_string();
            }
            if let Some(name) = incoming_name {
                state.name = name.to_string();
            }
            state.arguments.push_str(incoming_arguments);
            (state.id.clone(), state.name.clone())
        };
        self.responses_saw_tool_call = true;

        if emit_added {
            out.push(sse_event(
                Some("response.output_item.added"),
                &json!({
                    "type": "response.output_item.added",
                    "output_index": source_index,
                    "item": {
                        "id": id,
                        "type": "function_call",
                        "status": "in_progress",
                        "call_id": id,
                        "name": name
                    }
                }),
            ));
        }
        if !incoming_arguments.is_empty() {
            out.push(sse_event(
                Some("response.function_call_arguments.delta"),
                &json!({
                    "type": "response.function_call_arguments.delta",
                    "item_id": id,
                    "output_index": source_index,
                    "delta": incoming_arguments
                }),
            ));
        }
    }

    fn anthropic_event_to_chat(&mut self, parsed: ParsedSseBlock, value: Value) -> Vec<Vec<u8>> {
        let event_type = anthropic_event_type(&parsed, &value);
        match event_type.as_deref() {
            Some("message_start") => {
                let message = value.get("message").unwrap_or(&value);
                self.message_id = message
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("chatcmpl_gateway")
                    .to_string();
                self.model = message
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                self.sent_start = true;
                vec![self.chat_delta(json!({"role": "assistant"}), None)]
            }
            Some("content_block_start") => {
                let block = value.get("content_block").unwrap_or(&Value::Null);
                if block.get("type").and_then(Value::as_str) != Some("tool_use") {
                    return Vec::new();
                }
                let block_index = value
                    .get("index")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize)
                    .unwrap_or(self.anthropic_source_next_tool_index);
                let tool_index = self.anthropic_source_next_tool_index;
                self.anthropic_source_next_tool_index += 1;
                let id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                self.anthropic_source_tools.insert(
                    block_index,
                    AnthropicSourceToolBlock {
                        tool_index,
                        id: id.clone(),
                        name: name.clone(),
                        arguments: String::new(),
                    },
                );
                vec![self.chat_delta(
                    json!({
                        "tool_calls": [{
                            "index": tool_index,
                            "id": id,
                            "type": "function",
                            "function": {"name": name, "arguments": ""}
                        }]
                    }),
                    None,
                )]
            }
            Some("content_block_delta") => {
                if let Some(text) = value.pointer("/delta/text").and_then(Value::as_str) {
                    if text.is_empty() {
                        return Vec::new();
                    }
                    return vec![self.chat_delta(json!({"content": text}), None)];
                }
                if let Some(thinking) = value.pointer("/delta/thinking").and_then(Value::as_str) {
                    if thinking.is_empty() {
                        return Vec::new();
                    }
                    return vec![self.chat_delta(json!({"reasoning_content": thinking}), None)];
                }
                if let Some(partial_json) =
                    value.pointer("/delta/partial_json").and_then(Value::as_str)
                {
                    let block_index = value
                        .get("index")
                        .and_then(Value::as_u64)
                        .map(|value| value as usize)
                        .unwrap_or(0);
                    if let Some(tool) = self.anthropic_source_tools.get_mut(&block_index) {
                        tool.arguments.push_str(partial_json);
                        let tool_index = tool.tool_index;
                        let tool_id = tool.id.clone();
                        return vec![self.chat_delta(
                            json!({
                                "tool_calls": [{
                                    "index": tool_index,
                                    "id": tool_id,
                                    "type": "function",
                                    "function": {"arguments": partial_json}
                                }]
                            }),
                            None,
                        )];
                    }
                }
                Vec::new()
            }
            Some("message_delta") => {
                let finish = anthropic_stop_to_chat_finish(
                    value.pointer("/delta/stop_reason").and_then(Value::as_str),
                );
                if finish.is_some() {
                    self.sent_chat_finish_delta = true;
                }
                vec![self.chat_delta(json!({}), finish)]
            }
            Some("message_stop") => self.finish(),
            _ => Vec::new(),
        }
    }

    fn anthropic_event_to_responses(
        &mut self,
        parsed: ParsedSseBlock,
        value: Value,
    ) -> Vec<Vec<u8>> {
        let event_type = anthropic_event_type(&parsed, &value);
        match event_type.as_deref() {
            Some("message_start") => {
                let message = value.get("message").unwrap_or(&value);
                self.message_id = message
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("resp_gateway")
                    .to_string();
                self.model = message
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                self.sent_start = true;
                vec![sse_event(
                    Some("response.created"),
                    &json!({
                        "type": "response.created",
                        "response": {
                            "id": self.message_id,
                            "object": "response",
                            "status": "in_progress",
                            "model": self.model,
                            "output": []
                        }
                    }),
                )]
            }
            Some("content_block_start") => {
                let block = value.get("content_block").unwrap_or(&Value::Null);
                if block.get("type").and_then(Value::as_str) != Some("tool_use") {
                    return Vec::new();
                }
                let block_index = value
                    .get("index")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize)
                    .unwrap_or(self.anthropic_source_next_tool_index);
                let tool_index = self.anthropic_source_next_tool_index;
                self.anthropic_source_next_tool_index += 1;
                self.responses_saw_tool_call = true;
                let id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                self.anthropic_source_tools.insert(
                    block_index,
                    AnthropicSourceToolBlock {
                        tool_index,
                        id: id.clone(),
                        name: name.clone(),
                        arguments: String::new(),
                    },
                );
                vec![sse_event(
                    Some("response.output_item.added"),
                    &json!({
                        "type": "response.output_item.added",
                        "output_index": tool_index,
                        "item": {
                            "id": id,
                            "type": "function_call",
                            "status": "in_progress",
                            "call_id": id,
                            "name": name
                        }
                    }),
                )]
            }
            Some("content_block_delta") => {
                if let Some(text) = value.pointer("/delta/text").and_then(Value::as_str) {
                    if text.is_empty() {
                        return Vec::new();
                    }
                    return vec![sse_event(
                        Some("response.output_text.delta"),
                        &json!({
                            "type": "response.output_text.delta",
                            "delta": text,
                            "item_id": self.message_id,
                            "output_index": 0,
                            "content_index": 0
                        }),
                    )];
                }
                if let Some(thinking) = value.pointer("/delta/thinking").and_then(Value::as_str) {
                    if thinking.is_empty() {
                        return Vec::new();
                    }
                    return vec![sse_event(
                        Some("response.reasoning_summary_text.delta"),
                        &json!({
                            "type": "response.reasoning_summary_text.delta",
                            "delta": thinking,
                            "item_id": self.message_id,
                            "output_index": 0,
                            "summary_index": 0
                        }),
                    )];
                }
                if let Some(partial_json) =
                    value.pointer("/delta/partial_json").and_then(Value::as_str)
                {
                    let block_index = value
                        .get("index")
                        .and_then(Value::as_u64)
                        .map(|value| value as usize)
                        .unwrap_or(0);
                    if let Some(tool) = self.anthropic_source_tools.get_mut(&block_index) {
                        tool.arguments.push_str(partial_json);
                        return vec![sse_event(
                            Some("response.function_call_arguments.delta"),
                            &json!({
                                "type": "response.function_call_arguments.delta",
                                "item_id": tool.id,
                                "output_index": tool.tool_index,
                                "delta": partial_json
                            }),
                        )];
                    }
                }
                Vec::new()
            }
            Some("content_block_stop") => {
                let block_index = value
                    .get("index")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize)
                    .unwrap_or(0);
                let Some(tool) = self.anthropic_source_tools.get(&block_index) else {
                    return Vec::new();
                };
                vec![
                    sse_event(
                        Some("response.function_call_arguments.done"),
                        &json!({
                            "type": "response.function_call_arguments.done",
                            "item_id": tool.id,
                            "output_index": tool.tool_index,
                            "call_id": tool.id,
                            "name": tool.name,
                            "arguments": tool.arguments,
                        }),
                    ),
                    sse_event(
                        Some("response.output_item.done"),
                        &json!({
                            "type": "response.output_item.done",
                            "output_index": tool.tool_index,
                            "item": {
                                "id": tool.id,
                                "type": "function_call",
                                "status": "completed",
                                "call_id": tool.id,
                                "name": tool.name,
                                "arguments": tool.arguments,
                            }
                        }),
                    ),
                ]
            }
            Some("message_stop") => self.finish(),
            _ => Vec::new(),
        }
    }

    fn responses_event_to_anthropic(
        &mut self,
        parsed: ParsedSseBlock,
        value: Value,
    ) -> Vec<Vec<u8>> {
        let event_name = parsed
            .event
            .as_deref()
            .or_else(|| value.get("type").and_then(Value::as_str))
            .unwrap_or_default();
        let mut out = Vec::new();
        match event_name {
            "response.created" => {
                let response = value.get("response").unwrap_or(&value);
                self.message_id = response
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("msg_gateway")
                    .to_string();
                self.model = response
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                self.sent_start = true;
                out.push(sse_event(
                    Some("message_start"),
                    &json!({
                        "type": "message_start",
                        "message": {
                            "id": self.message_id,
                            "type": "message",
                            "role": "assistant",
                            "model": self.model,
                            "content": [],
                            "usage": {"input_tokens": 0, "output_tokens": 0}
                        }
                    }),
                ));
            }
            "response.output_item.added" => {
                let item = value.get("item").unwrap_or(&Value::Null);
                let item_type = item.get("type").and_then(Value::as_str);
                if !matches!(item_type, Some("function_call") | Some("custom_tool_call")) {
                    return out;
                }
                self.responses_saw_tool_call = true;
                let item_id = item
                    .get("id")
                    .and_then(Value::as_str)
                    .or_else(|| value.get("item_id").and_then(Value::as_str))
                    .unwrap_or_default()
                    .to_string();
                let call_id = item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(&item_id)
                    .to_string();
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let tool_index = self.responses_next_tool_index;
                self.responses_next_tool_index += 1;
                let block_index =
                    self.ensure_anthropic_tool_block(&mut out, tool_index, &call_id, &name);
                self.responses_stream_tools.insert(
                    item_id,
                    ResponsesStreamToolCall {
                        call_id,
                        arguments: String::new(),
                        tool_index,
                        block_index: Some(block_index),
                    },
                );
            }
            "response.function_call_arguments.delta" | "response.custom_tool_call_input.delta" => {
                let item_id = value
                    .get("item_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let delta = value
                    .get("delta")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if delta.is_empty() {
                    return out;
                }
                if let Some(tool) = self.responses_stream_tools.get_mut(&item_id) {
                    tool.arguments.push_str(delta);
                    let index = tool.block_index.unwrap_or_else(|| {
                        self.anthropic_open_block
                            .as_ref()
                            .map(|block| block.index)
                            .unwrap_or(0)
                    });
                    out.push(sse_event(
                        Some("content_block_delta"),
                        &json!({
                            "type": "content_block_delta",
                            "index": index,
                            "delta": {
                                "type": "input_json_delta",
                                "partial_json": delta
                            }
                        }),
                    ));
                }
            }
            "response.output_text.delta" => {
                if !self.sent_start {
                    self.ensure_anthropic_message_start(&mut out);
                }
                let index = self.ensure_anthropic_text_block(&mut out);
                let text = value
                    .get("delta")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                out.push(sse_event(
                    Some("content_block_delta"),
                    &json!({
                        "type": "content_block_delta",
                        "index": index,
                        "delta": {"type": "text_delta", "text": text}
                    }),
                ));
            }
            "response.reasoning_summary_text.delta" => {
                let thinking = value
                    .get("delta")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if !thinking.is_empty() {
                    let index = self.ensure_anthropic_thinking_block(&mut out);
                    out.push(sse_event(
                        Some("content_block_delta"),
                        &json!({
                            "type": "content_block_delta",
                            "index": index,
                            "delta": {"type": "thinking_delta", "thinking": thinking}
                        }),
                    ));
                }
            }
            "response.completed" => {
                self.anthropic_stop_reason = Some(if self.responses_saw_tool_call {
                    "tool_use".to_string()
                } else {
                    "end_turn".to_string()
                });
                out.extend(self.finish());
            }
            "response.incomplete" => {
                self.anthropic_stop_reason = Some("max_tokens".to_string());
                out.extend(self.finish());
            }
            _ => {}
        }
        out
    }

    fn responses_event_to_chat(&mut self, parsed: ParsedSseBlock, value: Value) -> Vec<Vec<u8>> {
        match parsed
            .event
            .as_deref()
            .or_else(|| value.get("type").and_then(Value::as_str))
            .unwrap_or_default()
        {
            "response.created" => {
                let response = value.get("response").unwrap_or(&value);
                self.message_id = response
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("chatcmpl_gateway")
                    .to_string();
                self.model = response
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                self.sent_start = true;
                vec![self.chat_delta(json!({"role": "assistant"}), None)]
            }
            "response.output_item.added" => {
                let item = value.get("item").unwrap_or(&Value::Null);
                let item_type = item.get("type").and_then(Value::as_str);
                if !matches!(item_type, Some("function_call") | Some("custom_tool_call")) {
                    return Vec::new();
                }
                self.responses_saw_tool_call = true;
                let item_id = item
                    .get("id")
                    .and_then(Value::as_str)
                    .or_else(|| value.get("item_id").and_then(Value::as_str))
                    .unwrap_or_default()
                    .to_string();
                let call_id = item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(&item_id)
                    .to_string();
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let tool_index = self.responses_next_tool_index;
                self.responses_next_tool_index += 1;
                self.responses_stream_tools.insert(
                    item_id,
                    ResponsesStreamToolCall {
                        call_id: call_id.clone(),
                        arguments: String::new(),
                        tool_index,
                        block_index: None,
                    },
                );
                vec![self.chat_delta(
                    json!({
                        "tool_calls": [{
                            "index": tool_index,
                            "id": call_id,
                            "type": "function",
                            "function": {"name": name, "arguments": ""}
                        }]
                    }),
                    None,
                )]
            }
            "response.function_call_arguments.delta" | "response.custom_tool_call_input.delta" => {
                let item_id = value
                    .get("item_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let delta = value
                    .get("delta")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if delta.is_empty() {
                    return Vec::new();
                }
                let Some(tool) = self.responses_stream_tools.get_mut(&item_id) else {
                    return Vec::new();
                };
                tool.arguments.push_str(delta);
                let tool_index = tool.tool_index;
                let call_id = tool.call_id.clone();
                vec![self.chat_delta(
                    json!({
                        "tool_calls": [{
                            "index": tool_index,
                            "id": call_id,
                            "type": "function",
                            "function": {"arguments": delta}
                        }]
                    }),
                    None,
                )]
            }
            "response.output_text.delta" => {
                let text = value
                    .get("delta")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                vec![self.chat_delta(json!({"content": text}), None)]
            }
            "response.reasoning_summary_text.delta" => {
                let thinking = value
                    .get("delta")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if thinking.is_empty() {
                    Vec::new()
                } else {
                    vec![self.chat_delta(json!({"reasoning_content": thinking}), None)]
                }
            }
            "response.completed" => self.finish(),
            "response.incomplete" => {
                self.sent_chat_finish_delta = true;
                vec![self.chat_delta(json!({}), Some("length")), sse_done()]
            }
            _ => Vec::new(),
        }
    }

    fn gemini_chunk_to_anthropic(&mut self, value: Value) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        if self.message_id.is_empty() {
            self.message_id = value
                .get("responseId")
                .and_then(Value::as_str)
                .unwrap_or("msg_gateway")
                .to_string();
        }
        if self.model.is_empty() {
            self.model = value
                .get("modelVersion")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
        }
        if let Some(usage) = value.get("usageMetadata") {
            self.gemini_latest_usage = Some(usage.clone());
        }
        if !self.sent_start {
            self.sent_start = true;
            out.push(sse_event(
                Some("message_start"),
                &json!({
                    "type": "message_start",
                    "message": {
                        "id": self.message_id,
                        "type": "message",
                        "role": "assistant",
                        "model": self.model,
                        "content": [],
                        "usage": gemini_usage_to_anthropic(self.gemini_latest_usage.as_ref())
                    }
                }),
            ));
        }

        if let Some(reason) = value
            .pointer("/promptFeedback/blockReason")
            .and_then(Value::as_str)
        {
            self.gemini_blocked_text = Some(format!(
                "Request blocked by Gemini safety filters: {reason}"
            ));
        }

        let Some(candidate) = value
            .get("candidates")
            .and_then(Value::as_array)
            .and_then(|candidates| candidates.first())
        else {
            return out;
        };
        if let Some(reason) = candidate.get("finishReason").and_then(Value::as_str) {
            self.gemini_latest_finish_reason = Some(reason.to_string());
        }
        let Some(parts) = candidate
            .pointer("/content/parts")
            .and_then(Value::as_array)
        else {
            return out;
        };

        let visible_text = gemini_visible_text(parts);
        if !visible_text.is_empty() {
            let delta = if visible_text.starts_with(&self.gemini_accumulated_text) {
                visible_text[self.gemini_accumulated_text.len()..].to_string()
            } else {
                visible_text.clone()
            };
            if !delta.is_empty() {
                if !self.opened_text {
                    self.opened_text = true;
                    out.push(sse_event(
                        Some("content_block_start"),
                        &json!({
                            "type": "content_block_start",
                            "index": 0,
                            "content_block": {"type": "text", "text": ""}
                        }),
                    ));
                }
                out.push(sse_event(
                    Some("content_block_delta"),
                    &json!({
                        "type": "content_block_delta",
                        "index": 0,
                        "delta": {"type": "text_delta", "text": delta}
                    }),
                ));
                if visible_text.starts_with(&self.gemini_accumulated_text) {
                    self.gemini_accumulated_text = visible_text;
                } else {
                    self.gemini_accumulated_text.push_str(&delta);
                }
            }
        }
        self.merge_gemini_tool_calls(parts);
        if candidate.get("finishReason").is_some() {
            out.extend(self.finish());
        }
        out
    }

    fn anthropic_event_to_gemini(&mut self, parsed: ParsedSseBlock, value: Value) -> Vec<Vec<u8>> {
        let event_type = anthropic_event_type(&parsed, &value);
        match event_type.as_deref() {
            Some("message_start") => {
                let message = value.get("message").unwrap_or(&value);
                self.message_id = message
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                self.model = message
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                self.sent_start = true;
                Vec::new()
            }
            Some("content_block_start") => {
                let block = value.get("content_block").unwrap_or(&Value::Null);
                if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                    self.anthropic_current_tool = Some(AnthropicGeminiToolBlock {
                        id: block
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        name: block
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        partial_json: String::new(),
                    });
                }
                Vec::new()
            }
            Some("content_block_delta") => {
                if let Some(text) = value.pointer("/delta/text").and_then(Value::as_str) {
                    if text.is_empty() {
                        return Vec::new();
                    }
                    self.sent_start = true;
                    return vec![self.gemini_sse_chunk(vec![json!({ "text": text })], None, None)];
                }
                if let Some(partial_json) =
                    value.pointer("/delta/partial_json").and_then(Value::as_str)
                {
                    if let Some(tool) = self.anthropic_current_tool.as_mut() {
                        tool.partial_json.push_str(partial_json);
                    }
                }
                Vec::new()
            }
            Some("content_block_stop") => {
                let Some(tool) = self.anthropic_current_tool.take() else {
                    return Vec::new();
                };
                let args =
                    serde_json::from_str::<Value>(&tool.partial_json).unwrap_or_else(|_| json!({}));
                let mut function_call = Map::new();
                function_call.insert("name".to_string(), json!(tool.name));
                function_call.insert("args".to_string(), args);
                if !tool.id.is_empty() && !tool.id.starts_with(GEMINI_SYNTHETIC_TOOL_ID_PREFIX) {
                    function_call.insert("id".to_string(), json!(tool.id));
                }
                self.sent_start = true;
                vec![self.gemini_sse_chunk(
                    vec![json!({ "functionCall": Value::Object(function_call) })],
                    None,
                    None,
                )]
            }
            Some("message_delta") => {
                self.gemini_latest_finish_reason = value
                    .pointer("/delta/stop_reason")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                if let Some(usage) = value.get("usage") {
                    self.gemini_latest_usage = Some(anthropic_usage_to_gemini(usage));
                }
                Vec::new()
            }
            Some("message_stop") => self.finish(),
            _ => Vec::new(),
        }
    }

    fn finish_gemini_to_anthropic(&mut self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        if !self.sent_start {
            self.sent_start = true;
            out.push(sse_event(
                Some("message_start"),
                &json!({
                    "type": "message_start",
                    "message": {
                        "id": self.message_id,
                        "type": "message",
                        "role": "assistant",
                        "model": self.model,
                        "content": [],
                        "usage": gemini_usage_to_anthropic(self.gemini_latest_usage.as_ref())
                    }
                }),
            ));
        }
        if self.gemini_accumulated_text.is_empty() {
            if let Some(blocked_text) = self.gemini_blocked_text.clone() {
                if !self.opened_text {
                    self.opened_text = true;
                    out.push(sse_event(
                        Some("content_block_start"),
                        &json!({
                            "type": "content_block_start",
                            "index": 0,
                            "content_block": {"type": "text", "text": ""}
                        }),
                    ));
                }
                out.push(sse_event(
                    Some("content_block_delta"),
                    &json!({
                        "type": "content_block_delta",
                        "index": 0,
                        "delta": {"type": "text_delta", "text": blocked_text}
                    }),
                ));
            }
        }
        if self.opened_text {
            out.push(sse_event(
                Some("content_block_stop"),
                &json!({"type": "content_block_stop", "index": 0}),
            ));
            self.opened_text = false;
        }
        let mut next_index =
            if self.gemini_accumulated_text.is_empty() && self.gemini_blocked_text.is_none() {
                0
            } else {
                1
            };
        for tool_call in &self.gemini_tool_calls {
            out.push(sse_event(
                Some("content_block_start"),
                &json!({
                    "type": "content_block_start",
                    "index": next_index,
                    "content_block": {
                        "type": "tool_use",
                        "id": tool_call.id,
                        "name": tool_call.name,
                    }
                }),
            ));
            out.push(sse_event(
                Some("content_block_delta"),
                &json!({
                    "type": "content_block_delta",
                    "index": next_index,
                    "delta": {
                        "type": "input_json_delta",
                        "partial_json": serde_json::to_string(&tool_call.args)
                            .unwrap_or_else(|_| "{}".to_string())
                    }
                }),
            ));
            out.push(sse_event(
                Some("content_block_stop"),
                &json!({"type": "content_block_stop", "index": next_index}),
            ));
            next_index += 1;
        }
        let stop_reason = gemini_finish_reason_to_anthropic(
            self.gemini_latest_finish_reason.as_deref(),
            !self.gemini_tool_calls.is_empty(),
            self.gemini_blocked_text.is_some(),
        );
        out.push(sse_event(
            Some("message_delta"),
            &json!({
                "type": "message_delta",
                "delta": {"stop_reason": stop_reason, "stop_sequence": null},
                "usage": gemini_usage_to_anthropic(self.gemini_latest_usage.as_ref())
            }),
        ));
        out.push(sse_event(
            Some("message_stop"),
            &json!({"type": "message_stop"}),
        ));
        out
    }

    fn finish_anthropic_to_gemini(&mut self) -> Vec<Vec<u8>> {
        if !self.sent_start && self.message_id.is_empty() && self.model.is_empty() {
            return Vec::new();
        }
        vec![self.gemini_sse_chunk(
            Vec::new(),
            Some(anthropic_stop_to_gemini_finish(
                self.gemini_latest_finish_reason.as_deref(),
            )),
            self.gemini_latest_usage.clone(),
        )]
    }

    fn merge_gemini_tool_calls(&mut self, parts: &[Value]) {
        for (position, part) in parts.iter().enumerate() {
            let Some(function_call) = part.get("functionCall") else {
                continue;
            };
            let id = function_call
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{GEMINI_SYNTHETIC_TOOL_ID_PREFIX}{position}"));
            let tool_call = GeminiStreamToolCall {
                id,
                name: function_call
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                args: function_call
                    .get("args")
                    .cloned()
                    .unwrap_or_else(|| json!({})),
            };
            match self
                .gemini_tool_calls
                .iter()
                .position(|existing| existing.id == tool_call.id)
            {
                Some(index) => self.gemini_tool_calls[index] = tool_call,
                None => self.gemini_tool_calls.push(tool_call),
            }
        }
    }

    fn gemini_sse_chunk(
        &self,
        parts: Vec<Value>,
        finish_reason: Option<&str>,
        usage: Option<Value>,
    ) -> Vec<u8> {
        let mut candidate = Map::new();
        candidate.insert(
            "content".to_string(),
            json!({
                "role": "model",
                "parts": parts,
            }),
        );
        if let Some(finish_reason) = finish_reason {
            candidate.insert("finishReason".to_string(), json!(finish_reason));
        }
        let mut payload = Map::new();
        payload.insert("responseId".to_string(), json!(self.message_id));
        payload.insert("modelVersion".to_string(), json!(self.model));
        payload.insert("candidates".to_string(), json!([Value::Object(candidate)]));
        if let Some(usage) = usage {
            payload.insert("usageMetadata".to_string(), usage);
        }
        sse_event(None, &Value::Object(payload))
    }

    fn chat_delta(&self, delta: Value, finish_reason: Option<&str>) -> Vec<u8> {
        sse_event(
            None,
            &json!({
                "id": if self.message_id.is_empty() { "chatcmpl_gateway" } else { &self.message_id },
                "object": "chat.completion.chunk",
                "created": unix_timestamp(),
                "model": self.model,
                "choices": [{
                    "index": 0,
                    "delta": delta,
                    "finish_reason": finish_reason,
                }]
            }),
        )
    }

    fn capture_openai_id_model(&mut self, value: &Value) {
        if self.message_id.is_empty() {
            self.message_id = value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("msg_gateway")
                .to_string();
        }
        if self.model.is_empty() {
            self.model = value
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{stream, StreamExt};

    async fn collect_converted(route: ConversionRoute, chunks: Vec<&'static str>) -> String {
        let input: ConversionByteStream = Box::pin(stream::iter(
            chunks
                .into_iter()
                .map(|chunk| Ok(chunk.as_bytes().to_vec())),
        ));
        let output = convert_sse_stream(route, input)
            .collect::<Vec<Result<Vec<u8>, String>>>()
            .await
            .into_iter()
            .map(Result::unwrap)
            .flatten()
            .collect::<Vec<u8>>();
        String::from_utf8(output).unwrap()
    }

    #[tokio::test]
    async fn chat_to_anthropic_stream_finish_is_idempotent() {
        let output = collect_converted(
            ConversionRoute::new(AiProtocol::OpenAiChat, AiProtocol::AnthropicMessages),
            vec![
                r#"data: {"id":"chatcmpl_1","model":"gpt-test","choices":[{"index":0,"delta":{"content":"hi"},"finish_reason":null}]}"#,
                "\n\n",
                r#"data: {"id":"chatcmpl_1","model":"gpt-test","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
                "\n\n",
                "data: [DONE]\n\n",
            ],
        )
        .await;

        assert_eq!(output.matches("event: message_stop").count(), 1);
        assert_eq!(output.matches("event: message_delta").count(), 1);
    }

    #[tokio::test]
    async fn chat_to_anthropic_stream_maps_reasoning_and_tool_call() {
        let output = collect_converted(
            ConversionRoute::new(AiProtocol::OpenAiChat, AiProtocol::AnthropicMessages),
            vec![
                r#"data: {"id":"chatcmpl_tool","model":"gpt-test","choices":[{"index":0,"delta":{"reasoning_content":"think"},"finish_reason":null}]}"#,
                "\n\n",
                r#"data: {"id":"chatcmpl_tool","model":"gpt-test","choices":[{"index":0,"delta":{"content":"answer"},"finish_reason":null}]}"#,
                "\n\n",
                r#"data: {"id":"chatcmpl_tool","model":"gpt-test","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"a.txt\"}"}}]},"finish_reason":null}]}"#,
                "\n\n",
                r#"data: {"id":"chatcmpl_tool","model":"gpt-test","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#,
                "\n\n",
            ],
        )
        .await;

        assert!(output.contains(r#""type":"thinking""#));
        assert!(output.contains(r#""thinking":"think""#));
        assert!(output.contains(r#""text":"answer""#));
        assert!(output.contains(r#""type":"tool_use""#));
        assert!(output.contains(r#""id":"call_1""#));
        assert!(output.contains(r#""name":"read_file""#));
        assert!(output.contains(r#""partial_json":"{\"path\":\"a.txt\"}""#));
        assert!(output.contains(r#""stop_reason":"tool_use""#));
    }

    #[tokio::test]
    async fn chat_to_responses_stream_maps_reasoning_and_tool_call() {
        let output = collect_converted(
            ConversionRoute::new(AiProtocol::OpenAiChat, AiProtocol::OpenAiResponses),
            vec![
                r#"data: {"id":"chatcmpl_resp","model":"gpt-test","choices":[{"index":0,"delta":{"reasoning_content":"think"},"finish_reason":null}]}"#,
                "\n\n",
                r#"data: {"id":"chatcmpl_resp","model":"gpt-test","choices":[{"index":0,"delta":{"content":"answer"},"finish_reason":null}]}"#,
                "\n\n",
                r#"data: {"id":"chatcmpl_resp","model":"gpt-test","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"a.txt\"}"}}]},"finish_reason":null}]}"#,
                "\n\n",
                r#"data: {"id":"chatcmpl_resp","model":"gpt-test","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#,
                "\n\n",
            ],
        )
        .await;

        assert!(output.contains("event: response.created"));
        assert!(output.contains("event: response.reasoning_summary_text.delta"));
        assert!(output.contains(r#""delta":"think""#));
        assert!(output.contains("event: response.output_text.delta"));
        assert!(output.contains(r#""delta":"answer""#));
        assert!(output.contains("event: response.output_item.added"));
        assert!(output.contains(r#""type":"function_call""#));
        assert!(output.contains("event: response.function_call_arguments.delta"));
        assert!(output.contains(r#""delta":"{\"path\":\"a.txt\"}""#));
        assert!(output.contains("event: response.completed"));
    }

    #[tokio::test]
    async fn responses_to_chat_stream_finish_is_idempotent() {
        let output = collect_converted(
            ConversionRoute::new(AiProtocol::OpenAiResponses, AiProtocol::OpenAiChat),
            vec![
                "event: response.created\n",
                r#"data: {"type":"response.created","response":{"id":"resp_1","model":"gpt-test"}}"#,
                "\n\n",
                "event: response.completed\n",
                r#"data: {"type":"response.completed","response":{"id":"resp_1","model":"gpt-test"}}"#,
                "\n\n",
                "event: response.completed\n",
                r#"data: {"type":"response.completed","response":{"id":"resp_1","model":"gpt-test"}}"#,
                "\n\n",
            ],
        )
        .await;

        assert_eq!(output.matches("data: [DONE]").count(), 1);
        assert_eq!(output.matches("\"finish_reason\":\"stop\"").count(), 1);
    }

    #[tokio::test]
    async fn responses_to_anthropic_stream_maps_reasoning_and_tool_call() {
        let output = collect_converted(
            ConversionRoute::new(AiProtocol::OpenAiResponses, AiProtocol::AnthropicMessages),
            vec![
                "event: response.created\n",
                r#"data: {"type":"response.created","response":{"id":"resp_tool","model":"gpt-test"}}"#,
                "\n\n",
                "event: response.output_item.added\n",
                r#"data: {"type":"response.output_item.added","output_index":0,"item":{"id":"item_call_1","type":"function_call","call_id":"call_1","name":"read_file"}}"#,
                "\n\n",
                "event: response.function_call_arguments.delta\n",
                r#"data: {"type":"response.function_call_arguments.delta","item_id":"item_call_1","output_index":0,"delta":"{\"path\":\"a.txt\"}"}"#,
                "\n\n",
                "event: response.reasoning_summary_text.delta\n",
                r#"data: {"type":"response.reasoning_summary_text.delta","delta":"think","item_id":"rs_1","output_index":1,"summary_index":0}"#,
                "\n\n",
                "event: response.completed\n",
                r#"data: {"type":"response.completed","response":{"id":"resp_tool","model":"gpt-test"}}"#,
                "\n\n",
            ],
        )
        .await;

        assert!(output.contains(r#""type":"tool_use""#));
        assert!(output.contains(r#""id":"call_1""#));
        assert!(output.contains(r#""name":"read_file""#));
        assert!(output.contains(r#""partial_json":"{\"path\":\"a.txt\"}""#));
        assert!(output.contains(r#""type":"thinking""#));
        assert!(output.contains(r#""thinking":"think""#));
        assert!(output.contains(r#""stop_reason":"tool_use""#));
        assert_eq!(output.matches("event: message_stop").count(), 1);
    }

    #[tokio::test]
    async fn responses_to_chat_stream_maps_reasoning_and_tool_call() {
        let output = collect_converted(
            ConversionRoute::new(AiProtocol::OpenAiResponses, AiProtocol::OpenAiChat),
            vec![
                "event: response.created\n",
                r#"data: {"type":"response.created","response":{"id":"resp_chat","model":"gpt-test"}}"#,
                "\n\n",
                "event: response.output_item.added\n",
                r#"data: {"type":"response.output_item.added","output_index":0,"item":{"id":"item_call_1","type":"function_call","call_id":"call_1","name":"read_file"}}"#,
                "\n\n",
                "event: response.function_call_arguments.delta\n",
                r#"data: {"type":"response.function_call_arguments.delta","item_id":"item_call_1","output_index":0,"delta":"{\"path\":\"a.txt\"}"}"#,
                "\n\n",
                "event: response.reasoning_summary_text.delta\n",
                r#"data: {"type":"response.reasoning_summary_text.delta","delta":"think","item_id":"rs_1","output_index":1,"summary_index":0}"#,
                "\n\n",
                "event: response.completed\n",
                r#"data: {"type":"response.completed","response":{"id":"resp_chat","model":"gpt-test"}}"#,
                "\n\n",
            ],
        )
        .await;

        assert!(output.contains(r#""tool_calls":[{"#));
        assert!(output.contains(r#""id":"call_1""#));
        assert!(output.contains(r#""name":"read_file""#));
        assert!(output.contains(r#""arguments":"""#));
        assert!(output.contains(r#""function":{"arguments":"{\"path\":\"a.txt\"}"}"#));
        assert!(output.contains(r#""reasoning_content":"think""#));
        assert!(output.contains(r#""finish_reason":"tool_calls""#));
        assert_eq!(output.matches("data: [DONE]").count(), 1);
    }

    #[tokio::test]
    async fn gemini_to_anthropic_stream_maps_cumulative_text_and_finish_once() {
        let output = collect_converted(
            ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::AnthropicMessages),
            vec![
                r#"data: {"responseId":"resp_1","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"role":"model","parts":[{"text":"hel"}]}}],"usageMetadata":{"promptTokenCount":5,"cachedContentTokenCount":1,"totalTokenCount":8}}"#,
                "\n\n",
                r#"data: {"responseId":"resp_1","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"role":"model","parts":[{"text":"hello"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":5,"cachedContentTokenCount":1,"candidatesTokenCount":3,"totalTokenCount":8}}"#,
                "\n\n",
                "data: [DONE]\n\n",
            ],
        )
        .await;

        assert_eq!(output.matches("event: message_start").count(), 1);
        assert_eq!(output.matches("event: message_stop").count(), 1);
        assert_eq!(output.matches("event: message_delta").count(), 1);
        assert!(output.contains(r#""text":"hel""#));
        assert!(output.contains(r#""text":"lo""#));
        assert!(output.contains(r#""input_tokens":4"#));
        assert!(output.contains(r#""output_tokens":3"#));
    }

    #[tokio::test]
    async fn gemini_to_anthropic_stream_maps_function_call() {
        let output = collect_converted(
            ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::AnthropicMessages),
            vec![
                r#"data: {"responseId":"resp_2","modelVersion":"gemini-2.5-pro","candidates":[{"content":{"role":"model","parts":[{"functionCall":{"name":"read_file","args":{"path":"a.txt"}}}]},"finishReason":"STOP"}]}"#,
                "\n\n",
            ],
        )
        .await;

        assert!(output.contains(r#""type":"tool_use""#));
        assert!(output.contains(r#""id":"gemini_synth_0""#));
        assert!(output.contains(r#""name":"read_file""#));
        assert!(output.contains(r#""partial_json":"{\"path\":\"a.txt\"}""#));
        assert_eq!(output.matches("event: message_stop").count(), 1);
    }

    #[tokio::test]
    async fn gemini_to_anthropic_stream_ignores_invalid_json_without_panic() {
        let output = collect_converted(
            ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::AnthropicMessages),
            vec!["data: not-json\n\n"],
        )
        .await;

        assert_eq!(output.matches("event: message_stop").count(), 1);
    }

    #[tokio::test]
    async fn anthropic_to_gemini_stream_maps_text_and_finish() {
        let output = collect_converted(
            ConversionRoute::new(AiProtocol::AnthropicMessages, AiProtocol::GeminiNative),
            vec![
                "event: message_start\n",
                r#"data: {"type":"message_start","message":{"id":"msg_1","model":"claude-sonnet"}}"#,
                "\n\n",
                "event: content_block_delta\n",
                r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}}"#,
                "\n\n",
                "event: message_delta\n",
                r#"data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":4,"cache_read_input_tokens":1,"output_tokens":2}}"#,
                "\n\n",
                "event: message_stop\n",
                r#"data: {"type":"message_stop"}"#,
                "\n\n",
            ],
        )
        .await;

        assert!(output.contains(r#""text":"hi""#));
        assert!(output.contains(r#""finishReason":"STOP""#));
        assert!(output.contains(r#""promptTokenCount":5"#));
        assert!(!output.contains("[DONE]"));
    }

    #[tokio::test]
    async fn anthropic_to_chat_stream_maps_reasoning_and_tool_call() {
        let output = collect_converted(
            ConversionRoute::new(AiProtocol::AnthropicMessages, AiProtocol::OpenAiChat),
            vec![
                "event: message_start\n",
                r#"data: {"type":"message_start","message":{"id":"msg_tool","model":"claude-sonnet"}}"#,
                "\n\n",
                "event: content_block_delta\n",
                r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"think"}}"#,
                "\n\n",
                "event: content_block_start\n",
                r#"data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"read_file"}}"#,
                "\n\n",
                "event: content_block_delta\n",
                r#"data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"a.txt\"}"}}"#,
                "\n\n",
                "event: message_delta\n",
                r#"data: {"type":"message_delta","delta":{"stop_reason":"tool_use"}}"#,
                "\n\n",
                "event: message_stop\n",
                r#"data: {"type":"message_stop"}"#,
                "\n\n",
            ],
        )
        .await;

        assert!(output.contains(r#""reasoning_content":"think""#));
        assert!(output.contains(r#""tool_calls":[{"#));
        assert!(output.contains(r#""id":"toolu_1""#));
        assert!(output.contains(r#""name":"read_file""#));
        assert!(output.contains(r#""arguments":"""#));
        assert!(output.contains(r#""function":{"arguments":"{\"path\":\"a.txt\"}"}"#));
        assert!(output.contains(r#""finish_reason":"tool_calls""#));
        assert_eq!(output.matches("data: [DONE]").count(), 1);
    }

    #[tokio::test]
    async fn anthropic_to_responses_stream_maps_reasoning_and_tool_call() {
        let output = collect_converted(
            ConversionRoute::new(AiProtocol::AnthropicMessages, AiProtocol::OpenAiResponses),
            vec![
                "event: message_start\n",
                r#"data: {"type":"message_start","message":{"id":"msg_resp_tool","model":"claude-sonnet"}}"#,
                "\n\n",
                "event: content_block_delta\n",
                r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"think"}}"#,
                "\n\n",
                "event: content_block_start\n",
                r#"data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_1","name":"read_file"}}"#,
                "\n\n",
                "event: content_block_delta\n",
                r#"data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"a.txt\"}"}}"#,
                "\n\n",
                "event: content_block_stop\n",
                r#"data: {"type":"content_block_stop","index":1}"#,
                "\n\n",
                "event: message_stop\n",
                r#"data: {"type":"message_stop"}"#,
                "\n\n",
            ],
        )
        .await;

        assert!(output.contains("event: response.created"));
        assert!(output.contains("event: response.reasoning_summary_text.delta"));
        assert!(output.contains(r#""delta":"think""#));
        assert!(output.contains("event: response.output_item.added"));
        assert!(output.contains(r#""type":"function_call""#));
        assert!(output.contains("event: response.function_call_arguments.delta"));
        assert!(output.contains(r#""delta":"{\"path\":\"a.txt\"}""#));
        assert!(output.contains("event: response.function_call_arguments.done"));
        assert!(output.contains("event: response.completed"));
    }

    #[tokio::test]
    async fn anthropic_to_gemini_stream_maps_tool_use() {
        let output = collect_converted(
            ConversionRoute::new(AiProtocol::AnthropicMessages, AiProtocol::GeminiNative),
            vec![
                "event: message_start\n",
                r#"data: {"type":"message_start","message":{"id":"msg_2","model":"claude-sonnet"}}"#,
                "\n\n",
                "event: content_block_start\n",
                r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_1","name":"read_file"}}"#,
                "\n\n",
                "event: content_block_delta\n",
                r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"a.txt\"}"}}"#,
                "\n\n",
                "event: content_block_stop\n",
                r#"data: {"type":"content_block_stop","index":0}"#,
                "\n\n",
                "event: message_stop\n",
                r#"data: {"type":"message_stop"}"#,
                "\n\n",
            ],
        )
        .await;

        assert!(output.contains(r#""functionCall""#));
        assert!(output.contains(r#""id":"toolu_1""#));
        assert!(output.contains(r#""name":"read_file""#));
        assert!(output.contains(r#""path":"a.txt""#));
        assert!(output.contains(r#""finishReason":"STOP""#));
    }
}

fn anthropic_event_type(parsed: &ParsedSseBlock, value: &Value) -> Option<String> {
    parsed.event.clone().or_else(|| {
        value
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

fn chat_finish_to_anthropic_stop(reason: &str, has_tool_use: bool) -> &'static str {
    match reason {
        "length" => "max_tokens",
        "tool_calls" | "function_call" => "tool_use",
        _ if has_tool_use => "tool_use",
        _ => "end_turn",
    }
}

fn anthropic_stop_to_chat_finish(reason: Option<&str>) -> Option<&'static str> {
    match reason {
        Some("max_tokens") => Some("length"),
        Some("tool_use") => Some("tool_calls"),
        Some("end_turn") | Some("stop_sequence") => Some("stop"),
        Some(_) => Some("stop"),
        None => None,
    }
}

fn gemini_visible_text(parts: &[Value]) -> String {
    parts
        .iter()
        .filter(|part| part.get("thought").and_then(Value::as_bool) != Some(true))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<String>()
}

fn gemini_usage_to_anthropic(usage: Option<&Value>) -> Value {
    let usage = usage.unwrap_or(&Value::Null);
    let prompt = usage
        .get("promptTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cached = usage
        .get("cachedContentTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = usage
        .get("candidatesTokenCount")
        .and_then(Value::as_u64)
        .or_else(|| {
            usage
                .get("totalTokenCount")
                .and_then(Value::as_u64)
                .map(|total| total.saturating_sub(prompt))
        })
        .unwrap_or(0);
    json!({
        "input_tokens": prompt.saturating_sub(cached),
        "output_tokens": output,
        "cache_read_input_tokens": cached,
    })
}

fn anthropic_usage_to_gemini(usage: &Value) -> Value {
    let input = usage
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_read = usage
        .get("cache_read_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_creation = usage
        .get("cache_creation_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let prompt = input
        .saturating_add(cache_read)
        .saturating_add(cache_creation);
    let output = usage
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    json!({
        "promptTokenCount": prompt,
        "candidatesTokenCount": output,
        "totalTokenCount": prompt.saturating_add(output),
        "cachedContentTokenCount": cache_read,
    })
}

fn gemini_finish_reason_to_anthropic(
    reason: Option<&str>,
    has_tool_use: bool,
    blocked: bool,
) -> &'static str {
    if blocked {
        return "refusal";
    }
    match reason {
        Some("MAX_TOKENS") => "max_tokens",
        Some("SAFETY")
        | Some("RECITATION")
        | Some("SPII")
        | Some("BLOCKLIST")
        | Some("PROHIBITED_CONTENT") => "refusal",
        _ if has_tool_use => "tool_use",
        _ => "end_turn",
    }
}

fn anthropic_stop_to_gemini_finish(reason: Option<&str>) -> &'static str {
    match reason {
        Some("max_tokens") => "MAX_TOKENS",
        Some("refusal") => "SAFETY",
        _ => "STOP",
    }
}

fn unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
