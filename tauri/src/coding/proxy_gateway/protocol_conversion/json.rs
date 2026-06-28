use super::error::ProtocolConversionError;
use super::types::{AiProtocol, ConversionRoute};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

const GEMINI_SYNTHETIC_TOOL_ID_PREFIX: &str = "gemini_synth_";

pub fn convert_request_body(
    route: ConversionRoute,
    body: &[u8],
) -> Result<Vec<u8>, ProtocolConversionError> {
    if route.identity() {
        return Ok(body.to_vec());
    }
    let value = serde_json::from_slice::<Value>(body)
        .map_err(|error| ProtocolConversionError::InvalidJson(error.to_string()))?;
    let converted = convert_request_value(route, value)?;
    serde_json::to_vec(&converted)
        .map_err(|error| ProtocolConversionError::Transform(error.to_string()))
}

pub fn convert_response_body(
    route: ConversionRoute,
    body: &[u8],
) -> Result<Vec<u8>, ProtocolConversionError> {
    if route.identity() {
        return Ok(body.to_vec());
    }
    let value = serde_json::from_slice::<Value>(body)
        .map_err(|error| ProtocolConversionError::InvalidJson(error.to_string()))?;
    let converted = convert_response_value(route, value)?;
    serde_json::to_vec(&converted)
        .map_err(|error| ProtocolConversionError::Transform(error.to_string()))
}

pub fn convert_error_response_body(route: ConversionRoute, body: &[u8]) -> Vec<u8> {
    if route.identity() {
        return body.to_vec();
    }

    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return body.to_vec();
    };
    let Some(error) = extract_protocol_error(&value) else {
        return body.to_vec();
    };

    let converted = match route.target {
        AiProtocol::AnthropicMessages => protocol_error_to_anthropic(error),
        AiProtocol::OpenAiChat | AiProtocol::OpenAiResponses => protocol_error_to_openai(error),
        AiProtocol::GeminiNative => protocol_error_to_gemini(error),
    };
    serde_json::to_vec(&converted).unwrap_or_else(|_| body.to_vec())
}

pub fn convert_request_value(
    route: ConversionRoute,
    value: Value,
) -> Result<Value, ProtocolConversionError> {
    if route.identity() {
        return Ok(value);
    }

    match (route.source, route.target) {
        (AiProtocol::AnthropicMessages, AiProtocol::OpenAiChat) => anthropic_request_to_chat(value),
        (AiProtocol::OpenAiChat, AiProtocol::AnthropicMessages) => chat_request_to_anthropic(value),
        (AiProtocol::AnthropicMessages, AiProtocol::OpenAiResponses) => {
            anthropic_request_to_responses(value)
        }
        (AiProtocol::OpenAiResponses, AiProtocol::AnthropicMessages) => {
            responses_request_to_anthropic(value)
        }
        (AiProtocol::AnthropicMessages, AiProtocol::GeminiNative) => {
            anthropic_request_to_gemini(value)
        }
        (AiProtocol::GeminiNative, AiProtocol::AnthropicMessages) => {
            gemini_request_to_anthropic(value)
        }
        (AiProtocol::OpenAiResponses, AiProtocol::OpenAiChat) => responses_request_to_chat(value),
        (AiProtocol::OpenAiChat, AiProtocol::OpenAiResponses) => chat_request_to_responses(value),
        (_, AiProtocol::GeminiNative) | (AiProtocol::GeminiNative, _) => {
            Err(ProtocolConversionError::UnsupportedRoute(route))
        }
        _ => Err(ProtocolConversionError::UnsupportedRoute(route)),
    }
}

pub fn convert_response_value(
    route: ConversionRoute,
    value: Value,
) -> Result<Value, ProtocolConversionError> {
    if route.identity() {
        return Ok(value);
    }

    match (route.source, route.target) {
        (AiProtocol::OpenAiChat, AiProtocol::AnthropicMessages) => {
            chat_response_to_anthropic(value)
        }
        (AiProtocol::AnthropicMessages, AiProtocol::OpenAiChat) => {
            anthropic_response_to_chat(value)
        }
        (AiProtocol::OpenAiResponses, AiProtocol::AnthropicMessages) => {
            responses_response_to_anthropic(value)
        }
        (AiProtocol::AnthropicMessages, AiProtocol::OpenAiResponses) => {
            anthropic_response_to_responses(value)
        }
        (AiProtocol::GeminiNative, AiProtocol::AnthropicMessages) => {
            gemini_response_to_anthropic(value)
        }
        (AiProtocol::AnthropicMessages, AiProtocol::GeminiNative) => {
            anthropic_response_to_gemini(value)
        }
        (AiProtocol::OpenAiChat, AiProtocol::OpenAiResponses) => chat_response_to_responses(value),
        (AiProtocol::OpenAiResponses, AiProtocol::OpenAiChat) => responses_response_to_chat(value),
        (_, AiProtocol::GeminiNative) | (AiProtocol::GeminiNative, _) => {
            Err(ProtocolConversionError::UnsupportedRoute(route))
        }
        _ => Err(ProtocolConversionError::UnsupportedRoute(route)),
    }
}

fn anthropic_request_to_chat(body: Value) -> Result<Value, ProtocolConversionError> {
    let mut result = Map::new();
    copy_if_present(&body, &mut result, "model");

    let mut messages = Vec::new();
    append_anthropic_system_as_chat(&body, &mut messages);
    if let Some(items) = body.get("messages").and_then(Value::as_array) {
        for message in items {
            append_anthropic_message_as_chat(message, &mut messages);
        }
    }
    normalize_system_messages(&mut messages);
    result.insert("messages".to_string(), Value::Array(messages));

    copy_anthropic_generation_params_to_chat(&body, &mut result);
    append_anthropic_tools_as_chat(&body, &mut result);
    if result
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        ensure_openai_stream_usage(&mut result);
    }

    Ok(Value::Object(result))
}

fn chat_request_to_anthropic(body: Value) -> Result<Value, ProtocolConversionError> {
    let mut result = Map::new();
    copy_if_present(&body, &mut result, "model");
    if let Some(max_tokens) = body
        .get("max_tokens")
        .or_else(|| body.get("max_completion_tokens"))
    {
        result.insert("max_tokens".to_string(), max_tokens.clone());
    }
    for key in ["temperature", "top_p", "stream"] {
        copy_if_present(&body, &mut result, key);
    }
    if let Some(stop) = body.get("stop") {
        result.insert("stop_sequences".to_string(), stop.clone());
    }

    let mut system_chunks = Vec::new();
    let mut messages = Vec::new();
    if let Some(items) = body.get("messages").and_then(Value::as_array) {
        for message in items {
            let role = message
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("user");
            if role == "system" || role == "developer" {
                let text = openai_content_text(message.get("content"));
                if !text.is_empty() {
                    system_chunks.push(text);
                }
                continue;
            }
            append_chat_message_as_anthropic(message, &mut messages);
        }
    }
    if !system_chunks.is_empty() {
        result.insert("system".to_string(), json!(system_chunks.join("\n\n")));
    }
    result.insert("messages".to_string(), Value::Array(messages));

    append_chat_tools_as_anthropic(&body, &mut result);
    Ok(Value::Object(result))
}

fn anthropic_request_to_responses(body: Value) -> Result<Value, ProtocolConversionError> {
    let mut result = Map::new();
    copy_if_present(&body, &mut result, "model");
    if let Some(system) = body.get("system") {
        let instructions = anthropic_system_text(system);
        if !instructions.is_empty() {
            result.insert("instructions".to_string(), json!(instructions));
        }
    }
    if let Some(messages) = body.get("messages").and_then(Value::as_array) {
        result.insert(
            "input".to_string(),
            Value::Array(anthropic_messages_to_responses_input(messages)),
        );
    }
    if let Some(max_tokens) = body.get("max_tokens") {
        result.insert("max_output_tokens".to_string(), max_tokens.clone());
    }
    for key in ["temperature", "top_p", "stream"] {
        copy_if_present(&body, &mut result, key);
    }
    if let Some(tools) = body.get("tools").and_then(Value::as_array) {
        let converted: Vec<Value> = tools
            .iter()
            .filter_map(anthropic_tool_to_responses_tool)
            .collect();
        if !converted.is_empty() {
            result.insert("tools".to_string(), Value::Array(converted));
        }
    }
    if let Some(tool_choice) = body.get("tool_choice") {
        result.insert(
            "tool_choice".to_string(),
            anthropic_tool_choice_to_responses(tool_choice),
        );
    }
    Ok(Value::Object(result))
}

fn responses_request_to_anthropic(body: Value) -> Result<Value, ProtocolConversionError> {
    let mut result = Map::new();
    copy_if_present(&body, &mut result, "model");
    if let Some(max_tokens) = body
        .get("max_output_tokens")
        .or_else(|| body.get("max_tokens"))
    {
        result.insert("max_tokens".to_string(), max_tokens.clone());
    }
    for key in ["temperature", "top_p", "stream"] {
        copy_if_present(&body, &mut result, key);
    }
    if let Some(instructions) = body.get("instructions") {
        let text = instruction_text(instructions);
        if !text.is_empty() {
            result.insert("system".to_string(), json!(text));
        }
    }
    let messages = responses_input_to_anthropic_messages(body.get("input"));
    result.insert("messages".to_string(), Value::Array(messages));
    if let Some(tools) = body.get("tools").and_then(Value::as_array) {
        let converted: Vec<Value> = tools
            .iter()
            .filter_map(responses_tool_to_anthropic_tool)
            .collect();
        if !converted.is_empty() {
            result.insert("tools".to_string(), Value::Array(converted));
        }
    }
    if let Some(tool_choice) = body.get("tool_choice") {
        result.insert(
            "tool_choice".to_string(),
            responses_tool_choice_to_anthropic(tool_choice),
        );
    }
    Ok(Value::Object(result))
}

fn responses_request_to_chat(body: Value) -> Result<Value, ProtocolConversionError> {
    let mut result = Map::new();
    copy_if_present(&body, &mut result, "model");
    let mut messages = Vec::new();
    if let Some(instructions) = body.get("instructions") {
        let text = instruction_text(instructions);
        if !text.is_empty() {
            messages.push(json!({"role": "system", "content": text}));
        }
    }
    append_responses_input_as_chat(body.get("input"), &mut messages);
    normalize_system_messages(&mut messages);
    result.insert("messages".to_string(), Value::Array(messages));
    if let Some(max_tokens) = body
        .get("max_output_tokens")
        .or_else(|| body.get("max_tokens"))
    {
        result.insert("max_tokens".to_string(), max_tokens.clone());
    }
    for key in [
        "temperature",
        "top_p",
        "stream",
        "frequency_penalty",
        "presence_penalty",
        "response_format",
        "seed",
        "stop",
        "user",
    ] {
        copy_if_present(&body, &mut result, key);
    }
    if let Some(tools) = body.get("tools").and_then(Value::as_array) {
        let converted: Vec<Value> = tools
            .iter()
            .filter_map(responses_tool_to_chat_tool)
            .collect();
        if !converted.is_empty() {
            result.insert("tools".to_string(), Value::Array(converted));
        }
    }
    if let Some(tool_choice) = body.get("tool_choice") {
        result.insert(
            "tool_choice".to_string(),
            responses_tool_choice_to_chat(tool_choice),
        );
    }
    if result
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        ensure_openai_stream_usage(&mut result);
    }
    drop_tool_choice_without_tools(&mut result);
    Ok(Value::Object(result))
}

fn chat_request_to_responses(body: Value) -> Result<Value, ProtocolConversionError> {
    let mut result = Map::new();
    copy_if_present(&body, &mut result, "model");
    let mut instructions = Vec::new();
    let mut input = Vec::new();
    if let Some(messages) = body.get("messages").and_then(Value::as_array) {
        for message in messages {
            let role = message
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("user");
            if role == "system" || role == "developer" {
                let text = openai_content_text(message.get("content"));
                if !text.is_empty() {
                    instructions.push(text);
                }
                continue;
            }
            append_chat_message_as_responses_input(message, &mut input);
        }
    }
    if !instructions.is_empty() {
        result.insert("instructions".to_string(), json!(instructions.join("\n\n")));
    }
    result.insert("input".to_string(), Value::Array(input));
    if let Some(max_tokens) = body
        .get("max_completion_tokens")
        .or_else(|| body.get("max_tokens"))
    {
        result.insert("max_output_tokens".to_string(), max_tokens.clone());
    }
    for key in ["temperature", "top_p", "stream", "metadata", "user"] {
        copy_if_present(&body, &mut result, key);
    }
    if let Some(stop) = body.get("stop") {
        result.insert("stop".to_string(), stop.clone());
    }
    if let Some(tools) = body.get("tools").and_then(Value::as_array) {
        let converted: Vec<Value> = tools
            .iter()
            .filter_map(chat_tool_to_responses_tool)
            .collect();
        if !converted.is_empty() {
            result.insert("tools".to_string(), Value::Array(converted));
        }
    }
    if let Some(tool_choice) = body.get("tool_choice") {
        result.insert(
            "tool_choice".to_string(),
            chat_tool_choice_to_responses(tool_choice),
        );
    }
    Ok(Value::Object(result))
}

fn anthropic_request_to_gemini(body: Value) -> Result<Value, ProtocolConversionError> {
    let mut result = Map::new();

    let mut system_chunks = Vec::new();
    if let Some(system) = body.get("system") {
        let system_text = anthropic_system_text(system);
        if !system_text.is_empty() {
            system_chunks.push(system_text);
        }
    }
    if let Some(messages) = body.get("messages").and_then(Value::as_array) {
        for message in messages {
            if message.get("role").and_then(Value::as_str) == Some("system") {
                let text = anthropic_content_text(message.get("content"));
                if !text.is_empty() {
                    system_chunks.push(text);
                }
            }
        }
    }
    if !system_chunks.is_empty() {
        result.insert(
            "systemInstruction".to_string(),
            json!({ "parts": [{ "text": system_chunks.join("\n\n") }] }),
        );
    }

    let mut tool_names_by_id = collect_anthropic_tool_names(body.get("messages"));
    if let Some(messages) = body.get("messages").and_then(Value::as_array) {
        let contents: Vec<Value> = messages
            .iter()
            .filter_map(|message| {
                anthropic_message_to_gemini_content(message, &mut tool_names_by_id)
            })
            .collect();
        result.insert("contents".to_string(), Value::Array(contents));
    }

    if let Some(generation_config) = anthropic_generation_config_to_gemini(&body) {
        result.insert("generationConfig".to_string(), generation_config);
    }
    if let Some(tools) = anthropic_tools_to_gemini(body.get("tools")) {
        result.insert("tools".to_string(), tools);
    }
    if let Some(tool_config) = anthropic_tool_choice_to_gemini(body.get("tool_choice")) {
        result.insert("toolConfig".to_string(), tool_config);
    }

    Ok(Value::Object(result))
}

fn gemini_request_to_anthropic(body: Value) -> Result<Value, ProtocolConversionError> {
    let mut result = Map::new();
    copy_if_present(&body, &mut result, "model");
    if let Some(config) = body.get("generationConfig") {
        if let Some(max_tokens) = config.get("maxOutputTokens") {
            result.insert("max_tokens".to_string(), max_tokens.clone());
        }
        if let Some(temperature) = config.get("temperature") {
            result.insert("temperature".to_string(), temperature.clone());
        }
        if let Some(top_p) = config.get("topP") {
            result.insert("top_p".to_string(), top_p.clone());
        }
        if let Some(stop_sequences) = config.get("stopSequences") {
            result.insert("stop_sequences".to_string(), stop_sequences.clone());
        }
    }
    copy_if_present(&body, &mut result, "stream");

    let system = gemini_parts_text(
        body.pointer("/systemInstruction/parts")
            .and_then(Value::as_array),
    );
    if !system.is_empty() {
        result.insert("system".to_string(), json!(system));
    }

    let messages = body
        .get("contents")
        .and_then(Value::as_array)
        .map(|contents| {
            contents
                .iter()
                .map(gemini_content_to_anthropic_message)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    result.insert("messages".to_string(), Value::Array(messages));

    if let Some(tools) = gemini_tools_to_anthropic(body.get("tools")) {
        result.insert("tools".to_string(), tools);
    }
    if let Some(tool_choice) = gemini_tool_config_to_anthropic(body.get("toolConfig")) {
        result.insert("tool_choice".to_string(), tool_choice);
    }

    Ok(Value::Object(result))
}

fn chat_response_to_anthropic(body: Value) -> Result<Value, ProtocolConversionError> {
    let choice = first_choice(&body)?;
    let message = choice
        .get("message")
        .ok_or_else(|| ProtocolConversionError::Transform("missing chat message".to_string()))?;
    let mut content = Vec::new();
    if let Some(reasoning) = message.get("reasoning_content").and_then(Value::as_str) {
        if !reasoning.is_empty() {
            content.push(json!({"type": "thinking", "thinking": reasoning}));
        }
    }
    append_openai_content_as_anthropic_blocks(message.get("content"), "assistant", &mut content);
    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        for tool_call in tool_calls {
            content.push(chat_tool_call_to_anthropic_block(tool_call));
        }
    }
    if let Some(function_call) = message.get("function_call") {
        content.push(chat_function_call_to_anthropic_block(function_call));
    }
    let has_tool = content.iter().any(|block| {
        block
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "tool_use")
    });
    Ok(json!({
        "id": body.get("id").and_then(Value::as_str).unwrap_or_default(),
        "type": "message",
        "role": "assistant",
        "content": content,
        "model": body.get("model").and_then(Value::as_str).unwrap_or_default(),
        "stop_reason": chat_finish_reason_to_anthropic(
            choice.get("finish_reason").and_then(Value::as_str),
            has_tool
        ),
        "stop_sequence": Value::Null,
        "usage": openai_usage_to_anthropic(body.get("usage")),
    }))
}

fn anthropic_response_to_chat(body: Value) -> Result<Value, ProtocolConversionError> {
    let mut message = Map::new();
    message.insert("role".to_string(), json!("assistant"));
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    if let Some(content) = body.get("content").and_then(Value::as_array) {
        for block in content {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(chunk) = block.get("text").and_then(Value::as_str) {
                        text.push_str(chunk);
                    }
                }
                Some("thinking") => {
                    if let Some(thinking) = block.get("thinking").and_then(Value::as_str) {
                        message.insert("reasoning_content".to_string(), json!(thinking));
                    }
                }
                Some("tool_use") => tool_calls.push(anthropic_tool_use_to_chat_tool_call(block)),
                _ => {}
            }
        }
    }
    message.insert("content".to_string(), json!(text));
    if !tool_calls.is_empty() {
        message.insert("tool_calls".to_string(), Value::Array(tool_calls));
    }
    Ok(json!({
        "id": body.get("id").and_then(Value::as_str).unwrap_or_default(),
        "object": "chat.completion",
        "created": unix_timestamp(),
        "model": body.get("model").and_then(Value::as_str).unwrap_or_default(),
        "choices": [{
            "index": 0,
            "message": Value::Object(message),
            "finish_reason": anthropic_stop_reason_to_chat(body.get("stop_reason").and_then(Value::as_str))
        }],
        "usage": anthropic_usage_to_openai(body.get("usage")),
    }))
}

fn responses_response_to_anthropic(body: Value) -> Result<Value, ProtocolConversionError> {
    let mut content = Vec::new();
    let mut has_tool = false;
    if let Some(output) = body.get("output").and_then(Value::as_array) {
        for item in output {
            match item.get("type").and_then(Value::as_str) {
                Some("message") => {
                    if let Some(parts) = item.get("content").and_then(Value::as_array) {
                        for part in parts {
                            match part.get("type").and_then(Value::as_str) {
                                Some("output_text") => {
                                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                                        if !text.is_empty() {
                                            content.push(json!({"type": "text", "text": text}));
                                        }
                                    }
                                }
                                Some("refusal") => {
                                    if let Some(text) = part.get("refusal").and_then(Value::as_str)
                                    {
                                        if !text.is_empty() {
                                            content.push(json!({"type": "text", "text": text}));
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Some("function_call") | Some("custom_tool_call") => {
                    content.push(responses_function_call_to_anthropic_block(item));
                    has_tool = true;
                }
                Some("reasoning") => {
                    if let Some(text) = responses_reasoning_text(item) {
                        content.push(json!({"type": "thinking", "thinking": text}));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(json!({
        "id": body.get("id").and_then(Value::as_str).unwrap_or_default(),
        "type": "message",
        "role": "assistant",
        "content": content,
        "model": body.get("model").and_then(Value::as_str).unwrap_or_default(),
        "stop_reason": responses_status_to_anthropic_stop(
            body.get("status").and_then(Value::as_str),
            has_tool,
            body.pointer("/incomplete_details/reason").and_then(Value::as_str)
        ),
        "stop_sequence": Value::Null,
        "usage": responses_usage_to_anthropic(body.get("usage")),
    }))
}

fn anthropic_response_to_responses(body: Value) -> Result<Value, ProtocolConversionError> {
    let output = anthropic_content_to_responses_output(&body);
    Ok(json!({
        "id": body.get("id").and_then(Value::as_str).unwrap_or_default(),
        "object": "response",
        "created_at": unix_timestamp(),
        "status": anthropic_stop_reason_to_responses_status(body.get("stop_reason").and_then(Value::as_str)),
        "model": body.get("model").and_then(Value::as_str).unwrap_or_default(),
        "output": output,
        "usage": anthropic_usage_to_responses(body.get("usage")),
    }))
}

fn chat_response_to_responses(body: Value) -> Result<Value, ProtocolConversionError> {
    let choice = first_choice(&body)?;
    let message = choice
        .get("message")
        .ok_or_else(|| ProtocolConversionError::Transform("missing chat message".to_string()))?;
    let mut output = Vec::new();
    let mut content = Vec::new();
    if let Some(text) = message.get("content").and_then(Value::as_str) {
        if !text.is_empty() {
            content.push(json!({"type": "output_text", "text": text}));
        }
    }
    if !content.is_empty() {
        output.push(json!({
            "type": "message",
            "role": "assistant",
            "content": content,
        }));
    }
    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        for tool_call in tool_calls {
            output.push(chat_tool_call_to_responses_output(tool_call));
        }
    }
    Ok(json!({
        "id": body.get("id").and_then(Value::as_str).unwrap_or_default(),
        "object": "response",
        "created_at": unix_timestamp(),
        "status": chat_finish_reason_to_responses_status(choice.get("finish_reason").and_then(Value::as_str)),
        "model": body.get("model").and_then(Value::as_str).unwrap_or_default(),
        "output": output,
        "usage": openai_usage_to_responses(body.get("usage")),
    }))
}

fn responses_response_to_chat(body: Value) -> Result<Value, ProtocolConversionError> {
    let mut message = Map::new();
    message.insert("role".to_string(), json!("assistant"));
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    if let Some(output) = body.get("output").and_then(Value::as_array) {
        for item in output {
            match item.get("type").and_then(Value::as_str) {
                Some("message") => {
                    if let Some(parts) = item.get("content").and_then(Value::as_array) {
                        for part in parts {
                            if let Some(part_text) = part
                                .get("text")
                                .or_else(|| part.get("refusal"))
                                .and_then(Value::as_str)
                            {
                                text.push_str(part_text);
                            }
                        }
                    }
                }
                Some("function_call") | Some("custom_tool_call") => {
                    tool_calls.push(responses_function_call_to_chat_tool_call(item));
                }
                Some("reasoning") => {
                    if let Some(reasoning) = responses_reasoning_text(item) {
                        message.insert("reasoning_content".to_string(), json!(reasoning));
                    }
                }
                _ => {}
            }
        }
    }
    message.insert("content".to_string(), json!(text));
    let has_tool_calls = !tool_calls.is_empty();
    if !tool_calls.is_empty() {
        message.insert("tool_calls".to_string(), Value::Array(tool_calls));
    }
    Ok(json!({
        "id": body.get("id").and_then(Value::as_str).unwrap_or_default(),
        "object": "chat.completion",
        "created": unix_timestamp(),
        "model": body.get("model").and_then(Value::as_str).unwrap_or_default(),
        "choices": [{
            "index": 0,
            "message": Value::Object(message),
            "finish_reason": responses_status_to_chat_finish(body.get("status").and_then(Value::as_str), has_tool_calls)
        }],
        "usage": responses_usage_to_openai(body.get("usage")),
    }))
}

fn gemini_response_to_anthropic(body: Value) -> Result<Value, ProtocolConversionError> {
    if let Some(block_reason) = body
        .pointer("/promptFeedback/blockReason")
        .and_then(Value::as_str)
    {
        return Ok(json!({
            "id": body.get("responseId").and_then(Value::as_str).unwrap_or_default(),
            "type": "message",
            "role": "assistant",
            "content": [{
                "type": "text",
                "text": format!("Request blocked by Gemini safety filters: {block_reason}")
            }],
            "model": body.get("modelVersion").and_then(Value::as_str).unwrap_or_default(),
            "stop_reason": "refusal",
            "stop_sequence": Value::Null,
            "usage": gemini_usage_to_anthropic(body.get("usageMetadata")),
        }));
    }

    let candidate = body
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first());
    let parts = candidate
        .and_then(|candidate| candidate.pointer("/content/parts"))
        .and_then(Value::as_array);
    let mut content = Vec::new();
    let mut has_tool = false;
    if let Some(parts) = parts {
        for (index, part) in parts.iter().enumerate() {
            if part.get("thought").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                if !text.is_empty() {
                    content.push(json!({"type": "text", "text": text}));
                }
                continue;
            }
            if let Some(function_call) = part.get("functionCall") {
                has_tool = true;
                content.push(gemini_function_call_to_anthropic_tool_use(
                    function_call,
                    index,
                ));
            }
        }
    }

    Ok(json!({
        "id": body.get("responseId").and_then(Value::as_str).unwrap_or_default(),
        "type": "message",
        "role": "assistant",
        "content": content,
        "model": body.get("modelVersion").and_then(Value::as_str).unwrap_or_default(),
        "stop_reason": gemini_finish_reason_to_anthropic(
            candidate.and_then(|candidate| candidate.get("finishReason")).and_then(Value::as_str),
            has_tool,
        ),
        "stop_sequence": Value::Null,
        "usage": gemini_usage_to_anthropic(body.get("usageMetadata")),
    }))
}

fn anthropic_response_to_gemini(body: Value) -> Result<Value, ProtocolConversionError> {
    let mut parts = Vec::new();
    let mut has_tool = false;
    if let Some(content) = body.get("content").and_then(Value::as_array) {
        for block in content {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(text) = block.get("text").and_then(Value::as_str) {
                        if !text.is_empty() {
                            parts.push(json!({ "text": text }));
                        }
                    }
                }
                Some("tool_use") => {
                    has_tool = true;
                    parts.push(anthropic_tool_use_to_gemini_function_call(block));
                }
                _ => {}
            }
        }
    }

    Ok(json!({
        "responseId": body.get("id").and_then(Value::as_str).unwrap_or_default(),
        "modelVersion": body.get("model").and_then(Value::as_str).unwrap_or_default(),
        "candidates": [{
            "content": {
                "role": "model",
                "parts": parts,
            },
            "finishReason": anthropic_stop_reason_to_gemini(
                body.get("stop_reason").and_then(Value::as_str),
                has_tool,
            )
        }],
        "usageMetadata": anthropic_usage_to_gemini(body.get("usage")),
    }))
}

fn collect_anthropic_tool_names(messages: Option<&Value>) -> HashMap<String, String> {
    let mut tool_names = HashMap::new();
    let Some(messages) = messages.and_then(Value::as_array) else {
        return tool_names;
    };
    for message in messages {
        let Some(blocks) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for block in blocks {
            if block.get("type").and_then(Value::as_str) != Some("tool_use") {
                continue;
            }
            let Some(id) = block.get("id").and_then(Value::as_str) else {
                continue;
            };
            let Some(name) = block.get("name").and_then(Value::as_str) else {
                continue;
            };
            if !id.is_empty() && !name.is_empty() {
                tool_names.insert(id.to_string(), name.to_string());
            }
        }
    }
    tool_names
}

fn anthropic_message_to_gemini_content(
    message: &Value,
    tool_names_by_id: &mut HashMap<String, String>,
) -> Option<Value> {
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("user");
    if role == "system" {
        return None;
    }
    let parts = anthropic_content_to_gemini_parts(message.get("content"), role, tool_names_by_id);
    if parts.is_empty() {
        return None;
    }
    Some(json!({
        "role": if role == "assistant" { "model" } else { "user" },
        "parts": parts,
    }))
}

fn anthropic_content_to_gemini_parts(
    content: Option<&Value>,
    role: &str,
    tool_names_by_id: &mut HashMap<String, String>,
) -> Vec<Value> {
    match content {
        Some(Value::String(text)) => {
            if text.is_empty() {
                Vec::new()
            } else {
                vec![json!({ "text": text })]
            }
        }
        Some(Value::Array(blocks)) => {
            let mut parts = Vec::new();
            for block in blocks {
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(text) = block.get("text").and_then(Value::as_str) {
                            if !text.is_empty() {
                                parts.push(json!({ "text": text }));
                            }
                        }
                    }
                    Some("image") | Some("document") if role == "user" => {
                        if let Some(part) = anthropic_media_block_to_gemini_part(block) {
                            parts.push(part);
                        }
                    }
                    Some("tool_use") if role == "assistant" => {
                        if let (Some(id), Some(name)) = (
                            block.get("id").and_then(Value::as_str),
                            block.get("name").and_then(Value::as_str),
                        ) {
                            if !id.is_empty() && !name.is_empty() {
                                tool_names_by_id.insert(id.to_string(), name.to_string());
                            }
                        }
                        parts.push(anthropic_tool_use_to_gemini_function_call(block));
                    }
                    Some("tool_result") => {
                        parts.push(anthropic_tool_result_to_gemini_function_response(
                            block,
                            tool_names_by_id,
                        ));
                    }
                    Some("thinking") | Some("redacted_thinking") => {}
                    _ => {}
                }
            }
            parts
        }
        _ => Vec::new(),
    }
}

fn anthropic_media_block_to_gemini_part(block: &Value) -> Option<Value> {
    let source = block.get("source")?;
    if source.get("type").and_then(Value::as_str) != Some("base64") {
        return None;
    }
    let mime_type = source
        .get("media_type")
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            if block.get("type").and_then(Value::as_str) == Some("document") {
                "application/pdf"
            } else {
                "image/png"
            }
        });
    let data = source.get("data").and_then(Value::as_str)?;
    Some(json!({
        "inlineData": {
            "mimeType": mime_type,
            "data": data,
        }
    }))
}

fn anthropic_tool_use_to_gemini_function_call(block: &Value) -> Value {
    let mut function_call = Map::new();
    function_call.insert(
        "name".to_string(),
        json!(block
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()),
    );
    function_call.insert(
        "args".to_string(),
        block.get("input").cloned().unwrap_or_else(|| json!({})),
    );
    if let Some(id) = block
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty() && !is_gemini_synthetic_tool_id(id))
    {
        function_call.insert("id".to_string(), json!(id));
    }
    json!({ "functionCall": Value::Object(function_call) })
}

fn anthropic_tool_result_to_gemini_function_response(
    block: &Value,
    tool_names_by_id: &HashMap<String, String>,
) -> Value {
    let tool_use_id = block
        .get("tool_use_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut function_response = Map::new();
    function_response.insert(
        "name".to_string(),
        json!(tool_names_by_id
            .get(tool_use_id)
            .map(String::as_str)
            .unwrap_or(tool_use_id)),
    );
    function_response.insert(
        "response".to_string(),
        normalize_gemini_function_response(block.get("content")),
    );
    if !tool_use_id.is_empty() && !is_gemini_synthetic_tool_id(tool_use_id) {
        function_response.insert("id".to_string(), json!(tool_use_id));
    }
    json!({ "functionResponse": Value::Object(function_response) })
}

fn normalize_gemini_function_response(content: Option<&Value>) -> Value {
    match content {
        Some(Value::String(text)) => json!({ "content": text }),
        Some(Value::Array(blocks)) => {
            let texts: Vec<&str> = blocks
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect();
            if texts.is_empty() {
                json!({ "content": blocks })
            } else {
                json!({ "content": texts.join("\n") })
            }
        }
        Some(value) => json!({ "content": value }),
        None => json!({ "content": "" }),
    }
}

fn anthropic_generation_config_to_gemini(body: &Value) -> Option<Value> {
    let mut config = Map::new();
    if let Some(max_tokens) = body.get("max_tokens") {
        config.insert("maxOutputTokens".to_string(), max_tokens.clone());
    }
    if let Some(temperature) = body.get("temperature") {
        config.insert("temperature".to_string(), temperature.clone());
    }
    if let Some(top_p) = body.get("top_p") {
        config.insert("topP".to_string(), top_p.clone());
    }
    if let Some(stop_sequences) = body.get("stop_sequences") {
        config.insert("stopSequences".to_string(), stop_sequences.clone());
    }
    (!config.is_empty()).then(|| Value::Object(config))
}

fn anthropic_tools_to_gemini(tools: Option<&Value>) -> Option<Value> {
    let declarations: Vec<Value> = tools
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|tool| tool.get("type").and_then(Value::as_str) != Some("BatchTool"))
        .filter_map(|tool| {
            let name = tool.get("name").and_then(Value::as_str)?;
            let mut declaration = Map::new();
            declaration.insert("name".to_string(), json!(name));
            if let Some(description) = tool.get("description").and_then(Value::as_str) {
                declaration.insert("description".to_string(), json!(description));
            }
            declaration.insert(
                "parameters".to_string(),
                tool.get("input_schema")
                    .cloned()
                    .unwrap_or_else(|| json!({})),
            );
            Some(Value::Object(declaration))
        })
        .collect();
    (!declarations.is_empty()).then(|| {
        json!([{
            "functionDeclarations": declarations
        }])
    })
}

fn anthropic_tool_choice_to_gemini(tool_choice: Option<&Value>) -> Option<Value> {
    let tool_choice = tool_choice?;
    let config = match tool_choice {
        Value::String(value) => match value.as_str() {
            "auto" => json!({ "mode": "AUTO" }),
            "none" => json!({ "mode": "NONE" }),
            "any" => json!({ "mode": "ANY" }),
            _ => return None,
        },
        Value::Object(object) => match object.get("type").and_then(Value::as_str) {
            Some("auto") => json!({ "mode": "AUTO" }),
            Some("none") => json!({ "mode": "NONE" }),
            Some("any") => json!({ "mode": "ANY" }),
            Some("tool") => json!({
                "mode": "ANY",
                "allowedFunctionNames": [object.get("name").and_then(Value::as_str).unwrap_or_default()]
            }),
            _ => return None,
        },
        _ => return None,
    };
    Some(json!({ "functionCallingConfig": config }))
}

fn gemini_content_to_anthropic_message(content: &Value) -> Value {
    let role = if content.get("role").and_then(Value::as_str) == Some("model") {
        "assistant"
    } else {
        "user"
    };
    let blocks = content
        .get("parts")
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .enumerate()
                .filter_map(|(index, part)| gemini_part_to_anthropic_block(part, role, index))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "role": role,
        "content": blocks,
    })
}

fn gemini_part_to_anthropic_block(part: &Value, role: &str, index: usize) -> Option<Value> {
    if let Some(text) = part.get("text").and_then(Value::as_str) {
        return (!text.is_empty()).then(|| json!({ "type": "text", "text": text }));
    }
    if let Some(inline_data) = part
        .get("inlineData")
        .or_else(|| part.get("inline_data"))
        .filter(|_| role == "user")
    {
        let mime_type = inline_data
            .get("mimeType")
            .or_else(|| inline_data.get("mime_type"))
            .and_then(Value::as_str)
            .unwrap_or("application/octet-stream");
        let data = inline_data.get("data").and_then(Value::as_str)?;
        let block_type = if mime_type.starts_with("image/") {
            "image"
        } else {
            "document"
        };
        return Some(json!({
            "type": block_type,
            "source": {
                "type": "base64",
                "media_type": mime_type,
                "data": data,
            }
        }));
    }
    if let Some(function_call) = part.get("functionCall").filter(|_| role == "assistant") {
        return Some(gemini_function_call_to_anthropic_tool_use(
            function_call,
            index,
        ));
    }
    if let Some(function_response) = part.get("functionResponse") {
        return Some(json!({
            "type": "tool_result",
            "tool_use_id": function_response
                .get("id")
                .and_then(Value::as_str)
                .or_else(|| function_response.get("name").and_then(Value::as_str))
                .unwrap_or_default(),
            "content": gemini_function_response_text(function_response.get("response")),
        }));
    }
    None
}

fn gemini_function_call_to_anthropic_tool_use(function_call: &Value, index: usize) -> Value {
    json!({
        "type": "tool_use",
        "id": function_call
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| synthetic_gemini_tool_id(index)),
        "name": function_call.get("name").and_then(Value::as_str).unwrap_or_default(),
        "input": function_call.get("args").cloned().unwrap_or_else(|| json!({})),
    })
}

fn gemini_function_response_text(response: Option<&Value>) -> String {
    let Some(response) = response else {
        return String::new();
    };
    if let Some(text) = response.get("content").and_then(Value::as_str) {
        return text.to_string();
    }
    canonical_json_string(response)
}

fn gemini_parts_text(parts: Option<&Vec<Value>>) -> String {
    parts
        .into_iter()
        .flatten()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn gemini_tools_to_anthropic(tools: Option<&Value>) -> Option<Value> {
    let mut anthropic_tools = Vec::new();
    for tool in tools.and_then(Value::as_array).into_iter().flatten() {
        let Some(declarations) = tool.get("functionDeclarations").and_then(Value::as_array) else {
            continue;
        };
        for declaration in declarations {
            let Some(name) = declaration.get("name").and_then(Value::as_str) else {
                continue;
            };
            anthropic_tools.push(json!({
                "name": name,
                "description": declaration.get("description").cloned().unwrap_or(Value::Null),
                "input_schema": declaration.get("parameters").cloned().unwrap_or_else(|| json!({})),
            }));
        }
    }
    (!anthropic_tools.is_empty()).then(|| Value::Array(anthropic_tools))
}

fn gemini_tool_config_to_anthropic(tool_config: Option<&Value>) -> Option<Value> {
    let config = tool_config?.get("functionCallingConfig")?;
    match config.get("mode").and_then(Value::as_str) {
        Some("AUTO") => Some(json!({ "type": "auto" })),
        Some("NONE") => Some(json!({ "type": "none" })),
        Some("ANY") => {
            let allowed = config
                .get("allowedFunctionNames")
                .and_then(Value::as_array)
                .and_then(|values| values.first())
                .and_then(Value::as_str);
            match allowed {
                Some(name) if !name.is_empty() => Some(json!({ "type": "tool", "name": name })),
                _ => Some(json!({ "type": "any" })),
            }
        }
        _ => None,
    }
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

fn anthropic_usage_to_gemini(usage: Option<&Value>) -> Value {
    let usage = usage.unwrap_or(&Value::Null);
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

fn gemini_finish_reason_to_anthropic(reason: Option<&str>, has_tool: bool) -> &'static str {
    match reason {
        Some("MAX_TOKENS") => "max_tokens",
        Some("SAFETY")
        | Some("RECITATION")
        | Some("SPII")
        | Some("BLOCKLIST")
        | Some("PROHIBITED_CONTENT") => "refusal",
        _ if has_tool => "tool_use",
        _ => "end_turn",
    }
}

fn anthropic_stop_reason_to_gemini(reason: Option<&str>, has_tool: bool) -> &'static str {
    match reason {
        Some("max_tokens") => "MAX_TOKENS",
        Some("refusal") => "SAFETY",
        Some("tool_use") if has_tool => "STOP",
        _ => "STOP",
    }
}

fn synthetic_gemini_tool_id(index: usize) -> String {
    format!("{GEMINI_SYNTHETIC_TOOL_ID_PREFIX}{index}")
}

fn is_gemini_synthetic_tool_id(id: &str) -> bool {
    id.starts_with(GEMINI_SYNTHETIC_TOOL_ID_PREFIX)
}

fn append_anthropic_system_as_chat(body: &Value, messages: &mut Vec<Value>) {
    if let Some(system) = body.get("system") {
        let text = anthropic_system_text(system);
        if !text.is_empty() {
            messages.push(json!({"role": "system", "content": text}));
        }
    }
}

fn append_anthropic_message_as_chat(message: &Value, messages: &mut Vec<Value>) {
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("user");
    let content = message.get("content");
    let mut text_parts = Vec::new();
    let mut content_parts = Vec::new();
    let mut tool_calls = Vec::new();

    match content {
        Some(Value::String(text)) => text_parts.push(text.clone()),
        Some(Value::Array(blocks)) => {
            for block in blocks {
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(text) = block.get("text").and_then(Value::as_str) {
                            text_parts.push(text.to_string());
                            content_parts.push(json!({"type": "text", "text": text}));
                        }
                    }
                    Some("image") if role == "user" => {
                        if let Some(image_url) = anthropic_image_block_to_data_url(block) {
                            content_parts.push(json!({
                                "type": "image_url",
                                "image_url": { "url": image_url }
                            }));
                        }
                    }
                    Some("tool_use") if role == "assistant" => {
                        tool_calls.push(anthropic_tool_use_to_chat_tool_call(block));
                    }
                    Some("tool_result") => {
                        messages.push(json!({
                            "role": "tool",
                            "tool_call_id": block.get("tool_use_id").and_then(Value::as_str).unwrap_or_default(),
                            "content": tool_result_text(block.get("content")),
                        }));
                    }
                    Some("thinking") if role == "assistant" => {
                        if let Some(text) = block.get("thinking").and_then(Value::as_str) {
                            text_parts.push(text.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    let mut chat_message = Map::new();
    chat_message.insert("role".to_string(), json!(role));
    if role == "user" && !content_parts.is_empty() {
        chat_message.insert("content".to_string(), Value::Array(content_parts));
    } else {
        chat_message.insert("content".to_string(), json!(text_parts.join("\n")));
    }
    if !tool_calls.is_empty() {
        chat_message.insert("tool_calls".to_string(), Value::Array(tool_calls));
    }
    messages.push(Value::Object(chat_message));
}

fn append_chat_message_as_anthropic(message: &Value, messages: &mut Vec<Value>) {
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("user");
    if role == "tool" {
        messages.push(json!({
            "role": "user",
            "content": [{
                "type": "tool_result",
                "tool_use_id": message.get("tool_call_id").and_then(Value::as_str).unwrap_or_default(),
                "content": openai_content_text(message.get("content")),
            }]
        }));
        return;
    }

    let anthropic_role = if role == "assistant" {
        "assistant"
    } else {
        "user"
    };
    let mut content = Vec::new();
    append_openai_content_as_anthropic_blocks(message.get("content"), anthropic_role, &mut content);
    if role == "assistant" {
        if let Some(reasoning) = message.get("reasoning_content").and_then(Value::as_str) {
            if !reasoning.is_empty() {
                content.insert(0, json!({"type": "thinking", "thinking": reasoning}));
            }
        }
        if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
            for tool_call in tool_calls {
                content.push(chat_tool_call_to_anthropic_block(tool_call));
            }
        }
        if let Some(function_call) = message.get("function_call") {
            content.push(chat_function_call_to_anthropic_block(function_call));
        }
    }
    messages.push(json!({
        "role": anthropic_role,
        "content": content,
    }));
}

fn anthropic_messages_to_responses_input(messages: &[Value]) -> Vec<Value> {
    let mut input = Vec::new();
    for message in messages {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("user");
        match message.get("content") {
            Some(Value::String(text)) => input.push(json!({
                "role": role,
                "content": [{
                    "type": if role == "assistant" { "output_text" } else { "input_text" },
                    "text": text
                }]
            })),
            Some(Value::Array(blocks)) => {
                let mut content = Vec::new();
                for block in blocks {
                    match block.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            if let Some(text) = block.get("text").and_then(Value::as_str) {
                                content.push(json!({
                                    "type": if role == "assistant" { "output_text" } else { "input_text" },
                                    "text": text
                                }));
                            }
                        }
                        Some("image") if role == "user" => {
                            if let Some(image_url) = anthropic_image_block_to_data_url(block) {
                                content
                                    .push(json!({"type": "input_image", "image_url": image_url}));
                            }
                        }
                        Some("tool_use") => {
                            flush_responses_message(role, &mut content, &mut input);
                            let arguments =
                                block.get("input").cloned().unwrap_or_else(|| json!({}));
                            input.push(json!({
                                "type": "function_call",
                                "call_id": block.get("id").and_then(Value::as_str).unwrap_or_default(),
                                "name": block.get("name").and_then(Value::as_str).unwrap_or_default(),
                                "arguments": canonical_json_string(&arguments),
                            }));
                        }
                        Some("tool_result") => {
                            flush_responses_message(role, &mut content, &mut input);
                            input.push(json!({
                                "type": "function_call_output",
                                "call_id": block.get("tool_use_id").and_then(Value::as_str).unwrap_or_default(),
                                "output": tool_result_text(block.get("content")),
                            }));
                        }
                        Some("thinking") => {
                            if let Some(text) = block.get("thinking").and_then(Value::as_str) {
                                input.push(json!({
                                    "type": "reasoning",
                                    "summary": [{"type": "summary_text", "text": text}]
                                }));
                            }
                        }
                        _ => {}
                    }
                }
                flush_responses_message(role, &mut content, &mut input);
            }
            _ => {}
        }
    }
    input
}

fn responses_input_to_anthropic_messages(input: Option<&Value>) -> Vec<Value> {
    let Some(input) = input else {
        return Vec::new();
    };
    let mut messages = Vec::new();
    match input {
        Value::String(text) => messages.push(json!({"role": "user", "content": text})),
        Value::Array(items) => {
            for item in items {
                append_responses_item_as_anthropic_message(item, &mut messages);
            }
        }
        Value::Object(_) => append_responses_item_as_anthropic_message(input, &mut messages),
        _ => {}
    }
    messages
}

fn append_responses_item_as_anthropic_message(item: &Value, messages: &mut Vec<Value>) {
    match item.get("type").and_then(Value::as_str) {
        Some("function_call") | Some("custom_tool_call") => {
            messages.push(json!({
                "role": "assistant",
                "content": [responses_function_call_to_anthropic_block(item)]
            }));
        }
        Some("function_call_output") => {
            messages.push(json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": item.get("call_id").and_then(Value::as_str).unwrap_or_default(),
                    "content": item.get("output").and_then(Value::as_str).unwrap_or_default(),
                }]
            }));
        }
        Some("message") | None => {
            let role = item.get("role").and_then(Value::as_str).unwrap_or("user");
            let mut content = Vec::new();
            if let Some(parts) = item.get("content").and_then(Value::as_array) {
                for part in parts {
                    append_responses_content_as_anthropic_block(part, role, &mut content);
                }
            } else if let Some(text) = item.get("content").and_then(Value::as_str) {
                content.push(json!({"type": "text", "text": text}));
            }
            messages.push(json!({ "role": role, "content": content }));
        }
        Some("reasoning") => {
            if let Some(text) = responses_reasoning_text(item) {
                messages.push(json!({
                    "role": "assistant",
                    "content": [{"type": "thinking", "thinking": text}]
                }));
            }
        }
        _ => {}
    }
}

fn append_responses_input_as_chat(input: Option<&Value>, messages: &mut Vec<Value>) {
    let Some(input) = input else {
        return;
    };
    match input {
        Value::String(text) => messages.push(json!({"role": "user", "content": text})),
        Value::Array(items) => {
            let mut pending_tool_calls = Vec::new();
            for item in items {
                append_responses_item_as_chat(item, messages, &mut pending_tool_calls);
            }
            flush_pending_chat_tool_calls(messages, &mut pending_tool_calls);
        }
        Value::Object(_) => {
            let mut pending_tool_calls = Vec::new();
            append_responses_item_as_chat(input, messages, &mut pending_tool_calls);
            flush_pending_chat_tool_calls(messages, &mut pending_tool_calls);
        }
        _ => {}
    }
}

fn append_responses_item_as_chat(
    item: &Value,
    messages: &mut Vec<Value>,
    pending_tool_calls: &mut Vec<Value>,
) {
    match item.get("type").and_then(Value::as_str) {
        Some("function_call") | Some("custom_tool_call") => {
            pending_tool_calls.push(responses_function_call_to_chat_tool_call(item));
        }
        Some("function_call_output") => {
            flush_pending_chat_tool_calls(messages, pending_tool_calls);
            messages.push(json!({
                "role": "tool",
                "tool_call_id": item.get("call_id").and_then(Value::as_str).unwrap_or_default(),
                "content": item.get("output").and_then(Value::as_str).unwrap_or_default(),
            }));
        }
        Some("message") | None => {
            flush_pending_chat_tool_calls(messages, pending_tool_calls);
            let role = item.get("role").and_then(Value::as_str).unwrap_or("user");
            messages.push(json!({
                "role": if role == "assistant" { "assistant" } else { "user" },
                "content": responses_content_text(item.get("content")),
            }));
        }
        _ => {}
    }
}

fn append_chat_message_as_responses_input(message: &Value, input: &mut Vec<Value>) {
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("user");
    if role == "tool" {
        input.push(json!({
            "type": "function_call_output",
            "call_id": message.get("tool_call_id").and_then(Value::as_str).unwrap_or_default(),
            "output": openai_content_text(message.get("content")),
        }));
        return;
    }

    let response_role = if role == "assistant" {
        "assistant"
    } else {
        "user"
    };
    let content_type = if response_role == "assistant" {
        "output_text"
    } else {
        "input_text"
    };
    let mut content = Vec::new();
    append_openai_content_as_responses_parts(message.get("content"), content_type, &mut content);
    if !content.is_empty() {
        input.push(json!({
            "role": response_role,
            "content": content,
        }));
    }
    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        for tool_call in tool_calls {
            input.push(chat_tool_call_to_responses_output(tool_call));
        }
    }
}

fn append_openai_content_as_anthropic_blocks(
    content: Option<&Value>,
    role: &str,
    blocks: &mut Vec<Value>,
) {
    match content {
        Some(Value::String(text)) => {
            if !text.is_empty() {
                blocks.push(json!({"type": "text", "text": text}));
            }
        }
        Some(Value::Array(parts)) => {
            for part in parts {
                match part.get("type").and_then(Value::as_str) {
                    Some("text") | Some("input_text") | Some("output_text") => {
                        if let Some(text) = part.get("text").and_then(Value::as_str) {
                            blocks.push(json!({"type": "text", "text": text}));
                        }
                    }
                    Some("image_url") | Some("input_image") if role == "user" => {
                        if let Some(source) = openai_image_part_to_anthropic_source(part) {
                            blocks.push(json!({"type": "image", "source": source}));
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn append_openai_content_as_responses_parts(
    content: Option<&Value>,
    content_type: &str,
    parts: &mut Vec<Value>,
) {
    match content {
        Some(Value::String(text)) => {
            if !text.is_empty() {
                parts.push(json!({"type": content_type, "text": text}));
            }
        }
        Some(Value::Array(items)) => {
            for item in items {
                match item.get("type").and_then(Value::as_str) {
                    Some("text") | Some("input_text") | Some("output_text") => {
                        if let Some(text) = item.get("text").and_then(Value::as_str) {
                            parts.push(json!({"type": content_type, "text": text}));
                        }
                    }
                    Some("image_url") | Some("input_image") => {
                        if let Some(url) = item
                            .pointer("/image_url/url")
                            .or_else(|| item.get("image_url"))
                            .and_then(Value::as_str)
                        {
                            parts.push(json!({"type": "input_image", "image_url": url}));
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn append_responses_content_as_anthropic_block(part: &Value, role: &str, blocks: &mut Vec<Value>) {
    match part.get("type").and_then(Value::as_str) {
        Some("input_text") | Some("output_text") => {
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                blocks.push(json!({"type": "text", "text": text}));
            }
        }
        Some("input_image") if role == "user" => {
            if let Some(source) = openai_image_part_to_anthropic_source(part) {
                blocks.push(json!({"type": "image", "source": source}));
            }
        }
        _ => {}
    }
}

fn anthropic_content_to_responses_output(body: &Value) -> Vec<Value> {
    let mut output = Vec::new();
    let mut text_content = Vec::new();
    if let Some(content) = body.get("content").and_then(Value::as_array) {
        for block in content {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(text) = block.get("text").and_then(Value::as_str) {
                        text_content.push(json!({"type": "output_text", "text": text}));
                    }
                }
                Some("tool_use") => {
                    if !text_content.is_empty() {
                        output.push(json!({
                            "type": "message",
                            "role": "assistant",
                            "content": std::mem::take(&mut text_content),
                        }));
                    }
                    output.push(anthropic_tool_use_to_responses_output(block));
                }
                Some("thinking") => {
                    if let Some(text) = block.get("thinking").and_then(Value::as_str) {
                        output.push(json!({
                            "type": "reasoning",
                            "summary": [{"type": "summary_text", "text": text}]
                        }));
                    }
                }
                _ => {}
            }
        }
    }
    if !text_content.is_empty() {
        output.push(json!({
            "type": "message",
            "role": "assistant",
            "content": text_content,
        }));
    }
    output
}

fn copy_anthropic_generation_params_to_chat(body: &Value, result: &mut Map<String, Value>) {
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if let Some(max_tokens) = body.get("max_tokens") {
        let key = if is_openai_o_series(model) {
            "max_completion_tokens"
        } else {
            "max_tokens"
        };
        result.insert(key.to_string(), max_tokens.clone());
    }
    for key in ["temperature", "top_p", "stream"] {
        copy_if_present(body, result, key);
    }
    if let Some(stop) = body.get("stop_sequences") {
        result.insert("stop".to_string(), stop.clone());
    }
    if supports_reasoning_effort(model) {
        if let Some(effort) = anthropic_reasoning_effort(body) {
            result.insert("reasoning_effort".to_string(), json!(effort));
        }
    }
}

fn append_anthropic_tools_as_chat(body: &Value, result: &mut Map<String, Value>) {
    let Some(tools) = body.get("tools").and_then(Value::as_array) else {
        return;
    };
    let converted: Vec<Value> = tools
        .iter()
        .filter_map(anthropic_tool_to_chat_tool)
        .collect();
    if !converted.is_empty() {
        result.insert("tools".to_string(), Value::Array(converted));
    }
    if let Some(tool_choice) = body.get("tool_choice") {
        result.insert(
            "tool_choice".to_string(),
            anthropic_tool_choice_to_chat(tool_choice),
        );
    }
}

fn append_chat_tools_as_anthropic(body: &Value, result: &mut Map<String, Value>) {
    let Some(tools) = body.get("tools").and_then(Value::as_array) else {
        return;
    };
    let converted: Vec<Value> = tools
        .iter()
        .filter_map(chat_tool_to_anthropic_tool)
        .collect();
    if !converted.is_empty() {
        result.insert("tools".to_string(), Value::Array(converted));
    }
    if let Some(tool_choice) = body.get("tool_choice") {
        result.insert(
            "tool_choice".to_string(),
            chat_tool_choice_to_anthropic(tool_choice),
        );
    }
}

fn anthropic_tool_to_chat_tool(tool: &Value) -> Option<Value> {
    if tool.get("type").and_then(Value::as_str) == Some("BatchTool") {
        return None;
    }
    Some(json!({
        "type": "function",
        "function": {
            "name": tool.get("name").and_then(Value::as_str).unwrap_or_default(),
            "description": tool.get("description").cloned().unwrap_or(Value::Null),
            "parameters": tool.get("input_schema").cloned().unwrap_or_else(|| json!({})),
        }
    }))
}

fn chat_tool_to_anthropic_tool(tool: &Value) -> Option<Value> {
    let function = tool.get("function").unwrap_or(tool);
    let name = function.get("name").and_then(Value::as_str)?;
    Some(json!({
        "name": name,
        "description": function.get("description").cloned().unwrap_or(Value::Null),
        "input_schema": function.get("parameters").cloned().unwrap_or_else(|| json!({})),
    }))
}

fn anthropic_tool_to_responses_tool(tool: &Value) -> Option<Value> {
    let name = tool.get("name").and_then(Value::as_str)?;
    Some(json!({
        "type": "function",
        "name": name,
        "description": tool.get("description").cloned().unwrap_or(Value::Null),
        "parameters": tool.get("input_schema").cloned().unwrap_or_else(|| json!({})),
    }))
}

fn responses_tool_to_anthropic_tool(tool: &Value) -> Option<Value> {
    let name = tool.get("name").and_then(Value::as_str)?;
    Some(json!({
        "name": name,
        "description": tool.get("description").cloned().unwrap_or(Value::Null),
        "input_schema": tool.get("parameters").cloned().unwrap_or_else(|| json!({})),
    }))
}

fn responses_tool_to_chat_tool(tool: &Value) -> Option<Value> {
    let name = tool.get("name").and_then(Value::as_str)?;
    Some(json!({
        "type": "function",
        "function": {
            "name": name,
            "description": tool.get("description").cloned().unwrap_or(Value::Null),
            "parameters": tool.get("parameters").cloned().unwrap_or_else(|| json!({})),
        }
    }))
}

fn chat_tool_to_responses_tool(tool: &Value) -> Option<Value> {
    let function = tool.get("function").unwrap_or(tool);
    let name = function.get("name").and_then(Value::as_str)?;
    Some(json!({
        "type": "function",
        "name": name,
        "description": function.get("description").cloned().unwrap_or(Value::Null),
        "parameters": function.get("parameters").cloned().unwrap_or_else(|| json!({})),
    }))
}

fn anthropic_tool_use_to_chat_tool_call(block: &Value) -> Value {
    let arguments = block.get("input").cloned().unwrap_or_else(|| json!({}));
    json!({
        "id": block.get("id").and_then(Value::as_str).unwrap_or_default(),
        "type": "function",
        "function": {
            "name": block.get("name").and_then(Value::as_str).unwrap_or_default(),
            "arguments": canonical_json_string(&arguments),
        }
    })
}

fn chat_tool_call_to_anthropic_block(tool_call: &Value) -> Value {
    let function = tool_call.get("function").unwrap_or(tool_call);
    let args = function
        .get("arguments")
        .and_then(Value::as_str)
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .unwrap_or_else(|| json!({}));
    json!({
        "type": "tool_use",
        "id": tool_call.get("id").and_then(Value::as_str).unwrap_or_default(),
        "name": function.get("name").and_then(Value::as_str).unwrap_or_default(),
        "input": args,
    })
}

fn chat_function_call_to_anthropic_block(function_call: &Value) -> Value {
    let args = function_call
        .get("arguments")
        .and_then(Value::as_str)
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .unwrap_or_else(|| json!({}));
    json!({
        "type": "tool_use",
        "id": function_call.get("id").and_then(Value::as_str).unwrap_or_default(),
        "name": function_call.get("name").and_then(Value::as_str).unwrap_or_default(),
        "input": args,
    })
}

fn responses_function_call_to_anthropic_block(item: &Value) -> Value {
    let args = item
        .get("arguments")
        .and_then(Value::as_str)
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .unwrap_or_else(|| json!({}));
    json!({
        "type": "tool_use",
        "id": item.get("call_id").and_then(Value::as_str).unwrap_or_default(),
        "name": item.get("name").and_then(Value::as_str).unwrap_or_default(),
        "input": args,
    })
}

fn responses_function_call_to_chat_tool_call(item: &Value) -> Value {
    json!({
        "id": item.get("call_id").and_then(Value::as_str).unwrap_or_default(),
        "type": "function",
        "function": {
            "name": item.get("name").and_then(Value::as_str).unwrap_or_default(),
            "arguments": item.get("arguments").and_then(Value::as_str).unwrap_or("{}"),
        }
    })
}

fn chat_tool_call_to_responses_output(tool_call: &Value) -> Value {
    let function = tool_call.get("function").unwrap_or(tool_call);
    json!({
        "type": "function_call",
        "call_id": tool_call.get("id").and_then(Value::as_str).unwrap_or_default(),
        "name": function.get("name").and_then(Value::as_str).unwrap_or_default(),
        "arguments": function.get("arguments").and_then(Value::as_str).unwrap_or("{}"),
    })
}

fn anthropic_tool_use_to_responses_output(block: &Value) -> Value {
    let arguments = block.get("input").cloned().unwrap_or_else(|| json!({}));
    json!({
        "type": "function_call",
        "call_id": block.get("id").and_then(Value::as_str).unwrap_or_default(),
        "name": block.get("name").and_then(Value::as_str).unwrap_or_default(),
        "arguments": canonical_json_string(&arguments),
    })
}

fn copy_if_present(source: &Value, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key) {
        target.insert(key.to_string(), value.clone());
    }
}

fn anthropic_system_text(system: &Value) -> String {
    match system {
        Value::String(text) => strip_leading_anthropic_billing_header(text).to_string(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .and_then(Value::as_str)
                    .or_else(|| part.as_str())
            })
            .map(strip_leading_anthropic_billing_header)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => String::new(),
    }
}

fn anthropic_content_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .and_then(Value::as_str)
                    .or_else(|| part.as_str())
            })
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => String::new(),
    }
}

fn instruction_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .and_then(Value::as_str)
                    .or_else(|| part.as_str())
            })
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => String::new(),
    }
}

fn openai_content_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .or_else(|| part.get("content"))
                    .and_then(Value::as_str)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn responses_content_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .or_else(|| part.get("refusal"))
                    .and_then(Value::as_str)
            })
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn tool_result_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .and_then(Value::as_str)
                    .or_else(|| part.as_str())
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Some(other) => canonical_json_string(other),
        None => String::new(),
    }
}

fn anthropic_image_block_to_data_url(block: &Value) -> Option<String> {
    let source = block.get("source")?;
    let media_type = source
        .get("media_type")
        .and_then(Value::as_str)
        .unwrap_or("image/png");
    let data = source.get("data").and_then(Value::as_str)?;
    Some(format!("data:{media_type};base64,{data}"))
}

fn openai_image_part_to_anthropic_source(part: &Value) -> Option<Value> {
    let url = part
        .pointer("/image_url/url")
        .or_else(|| part.get("image_url"))
        .and_then(Value::as_str)?;
    let (metadata, data) = url.split_once(";base64,")?;
    let media_type = metadata.strip_prefix("data:").unwrap_or("image/png");
    Some(json!({
        "type": "base64",
        "media_type": media_type,
        "data": data,
    }))
}

fn ensure_openai_stream_usage(result: &mut Map<String, Value>) {
    match result.get_mut("stream_options") {
        Some(Value::Object(options)) => {
            options.insert("include_usage".to_string(), json!(true));
        }
        _ => {
            result.insert(
                "stream_options".to_string(),
                json!({ "include_usage": true }),
            );
        }
    }
}

fn drop_tool_choice_without_tools(result: &mut Map<String, Value>) {
    let has_tools = result
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| !tools.is_empty());
    if !has_tools {
        result.remove("tool_choice");
        result.remove("parallel_tool_calls");
    }
}

fn flush_responses_message(role: &str, content: &mut Vec<Value>, input: &mut Vec<Value>) {
    if content.is_empty() {
        return;
    }
    input.push(json!({
        "role": role,
        "content": std::mem::take(content),
    }));
}

fn flush_pending_chat_tool_calls(messages: &mut Vec<Value>, pending_tool_calls: &mut Vec<Value>) {
    if pending_tool_calls.is_empty() {
        return;
    }
    messages.push(json!({
        "role": "assistant",
        "content": "",
        "tool_calls": std::mem::take(pending_tool_calls),
    }));
}

fn first_choice(body: &Value) -> Result<&Value, ProtocolConversionError> {
    body.get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .ok_or_else(|| ProtocolConversionError::Transform("missing chat choices".to_string()))
}

fn openai_usage_to_anthropic(usage: Option<&Value>) -> Value {
    let usage = usage.unwrap_or(&Value::Null);
    json!({
        "input_tokens": usage.get("prompt_tokens").or_else(|| usage.get("input_tokens")).and_then(Value::as_u64).unwrap_or(0),
        "output_tokens": usage.get("completion_tokens").or_else(|| usage.get("output_tokens")).and_then(Value::as_u64).unwrap_or(0),
        "cache_read_input_tokens": usage.pointer("/prompt_tokens_details/cached_tokens")
            .or_else(|| usage.pointer("/input_tokens_details/cached_tokens"))
            .and_then(Value::as_u64)
            .unwrap_or(0),
    })
}

fn anthropic_usage_to_openai(usage: Option<&Value>) -> Value {
    let usage = usage.unwrap_or(&Value::Null);
    let input = usage
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_read = usage
        .get("cache_read_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    json!({
        "prompt_tokens": input.saturating_add(cache_read),
        "completion_tokens": output,
        "total_tokens": input.saturating_add(cache_read).saturating_add(output),
        "prompt_tokens_details": { "cached_tokens": cache_read },
    })
}

fn responses_usage_to_anthropic(usage: Option<&Value>) -> Value {
    let usage = usage.unwrap_or(&Value::Null);
    let input = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_read = usage
        .pointer("/input_tokens_details/cached_tokens")
        .or_else(|| usage.pointer("/prompt_tokens_details/cached_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    json!({
        "input_tokens": input.saturating_sub(cache_read),
        "output_tokens": output,
        "cache_read_input_tokens": cache_read,
    })
}

fn anthropic_usage_to_responses(usage: Option<&Value>) -> Value {
    let usage = usage.unwrap_or(&Value::Null);
    let input = usage
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_read = usage
        .get("cache_read_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    json!({
        "input_tokens": input.saturating_add(cache_read),
        "output_tokens": output,
        "total_tokens": input.saturating_add(cache_read).saturating_add(output),
        "input_tokens_details": { "cached_tokens": cache_read },
    })
}

fn openai_usage_to_responses(usage: Option<&Value>) -> Value {
    let usage = usage.unwrap_or(&Value::Null);
    json!({
        "input_tokens": usage.get("prompt_tokens").or_else(|| usage.get("input_tokens")).and_then(Value::as_u64).unwrap_or(0),
        "output_tokens": usage.get("completion_tokens").or_else(|| usage.get("output_tokens")).and_then(Value::as_u64).unwrap_or(0),
        "total_tokens": usage.get("total_tokens").and_then(Value::as_u64).unwrap_or(0),
        "input_tokens_details": {
            "cached_tokens": usage.pointer("/prompt_tokens_details/cached_tokens")
                .or_else(|| usage.pointer("/input_tokens_details/cached_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        },
    })
}

fn responses_usage_to_openai(usage: Option<&Value>) -> Value {
    let usage = usage.unwrap_or(&Value::Null);
    json!({
        "prompt_tokens": usage.get("input_tokens").or_else(|| usage.get("prompt_tokens")).and_then(Value::as_u64).unwrap_or(0),
        "completion_tokens": usage.get("output_tokens").or_else(|| usage.get("completion_tokens")).and_then(Value::as_u64).unwrap_or(0),
        "total_tokens": usage.get("total_tokens").and_then(Value::as_u64).unwrap_or(0),
        "prompt_tokens_details": {
            "cached_tokens": usage.pointer("/input_tokens_details/cached_tokens")
                .or_else(|| usage.pointer("/prompt_tokens_details/cached_tokens"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        },
    })
}

fn anthropic_tool_choice_to_chat(tool_choice: &Value) -> Value {
    match tool_choice {
        Value::String(value) if value == "any" => json!("required"),
        Value::String(_) => tool_choice.clone(),
        Value::Object(object) => match object.get("type").and_then(Value::as_str) {
            Some("any") => json!("required"),
            Some("auto") => json!("auto"),
            Some("none") => json!("none"),
            Some("tool") => json!({
                "type": "function",
                "function": { "name": object.get("name").and_then(Value::as_str).unwrap_or_default() }
            }),
            _ => tool_choice.clone(),
        },
        _ => tool_choice.clone(),
    }
}

fn chat_tool_choice_to_anthropic(tool_choice: &Value) -> Value {
    match tool_choice {
        Value::String(value) if value == "required" => json!({"type": "any"}),
        Value::String(value) => json!({"type": value}),
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some("function") {
                json!({
                    "type": "tool",
                    "name": object
                        .get("function")
                        .and_then(|function| function.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                })
            } else {
                tool_choice.clone()
            }
        }
        _ => tool_choice.clone(),
    }
}

fn anthropic_tool_choice_to_responses(tool_choice: &Value) -> Value {
    match tool_choice {
        Value::String(value) if value == "any" => json!("required"),
        Value::String(_) => tool_choice.clone(),
        Value::Object(object) => match object.get("type").and_then(Value::as_str) {
            Some("any") => json!("required"),
            Some("auto") => json!("auto"),
            Some("none") => json!("none"),
            Some("tool") => json!({
                "type": "function",
                "name": object.get("name").and_then(Value::as_str).unwrap_or_default(),
            }),
            _ => tool_choice.clone(),
        },
        _ => tool_choice.clone(),
    }
}

fn responses_tool_choice_to_anthropic(tool_choice: &Value) -> Value {
    match tool_choice {
        Value::String(value) if value == "required" => json!({"type": "any"}),
        Value::String(value) => json!({"type": value}),
        Value::Object(object) if object.get("type").and_then(Value::as_str) == Some("function") => {
            json!({
                "type": "tool",
                "name": object.get("name").and_then(Value::as_str).unwrap_or_default(),
            })
        }
        _ => tool_choice.clone(),
    }
}

fn responses_tool_choice_to_chat(tool_choice: &Value) -> Value {
    match tool_choice {
        Value::String(value) if value == "required" => json!("required"),
        Value::String(_) => tool_choice.clone(),
        Value::Object(object) if object.get("type").and_then(Value::as_str) == Some("function") => {
            json!({
                "type": "function",
                "function": { "name": object.get("name").and_then(Value::as_str).unwrap_or_default() },
            })
        }
        _ => tool_choice.clone(),
    }
}

fn chat_tool_choice_to_responses(tool_choice: &Value) -> Value {
    match tool_choice {
        Value::Object(object) if object.get("type").and_then(Value::as_str) == Some("function") => {
            json!({
                "type": "function",
                "name": object
                    .get("function")
                    .and_then(|function| function.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            })
        }
        _ => tool_choice.clone(),
    }
}

fn chat_finish_reason_to_anthropic(reason: Option<&str>, has_tool: bool) -> Option<&'static str> {
    match reason {
        Some("stop") => Some("end_turn"),
        Some("length") => Some("max_tokens"),
        Some("tool_calls") | Some("function_call") => Some("tool_use"),
        Some(_) => Some("end_turn"),
        None if has_tool => Some("tool_use"),
        None => None,
    }
}

fn anthropic_stop_reason_to_chat(reason: Option<&str>) -> Option<&'static str> {
    match reason {
        Some("max_tokens") => Some("length"),
        Some("tool_use") => Some("tool_calls"),
        Some("stop_sequence") | Some("end_turn") => Some("stop"),
        Some(_) => Some("stop"),
        None => None,
    }
}

fn responses_status_to_anthropic_stop(
    status: Option<&str>,
    has_tool: bool,
    incomplete_reason: Option<&str>,
) -> Option<&'static str> {
    match status {
        Some("completed") if has_tool => Some("tool_use"),
        Some("incomplete")
            if matches!(
                incomplete_reason,
                Some("max_output_tokens") | Some("max_tokens")
            ) =>
        {
            Some("max_tokens")
        }
        Some("completed") | Some("incomplete") => Some("end_turn"),
        _ => None,
    }
}

fn responses_status_to_chat_finish(status: Option<&str>, has_tool: bool) -> Option<&'static str> {
    match status {
        Some("completed") if has_tool => Some("tool_calls"),
        Some("completed") => Some("stop"),
        Some("incomplete") => Some("length"),
        _ => None,
    }
}

fn anthropic_stop_reason_to_responses_status(reason: Option<&str>) -> &'static str {
    match reason {
        Some("max_tokens") => "incomplete",
        _ => "completed",
    }
}

fn chat_finish_reason_to_responses_status(reason: Option<&str>) -> &'static str {
    match reason {
        Some("length") => "incomplete",
        _ => "completed",
    }
}

fn responses_reasoning_text(item: &Value) -> Option<String> {
    item.get("summary")
        .and_then(Value::as_array)
        .map(|summary| {
            summary
                .iter()
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .filter(|text| !text.is_empty())
}

fn canonical_json_string(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
}

fn strip_leading_anthropic_billing_header(text: &str) -> &str {
    const PREFIX: &str = "x-anthropic-billing-header:";
    if !text.starts_with(PREFIX) {
        return text;
    }
    let Some(line_end) = text
        .as_bytes()
        .iter()
        .position(|byte| *byte == b'\n' || *byte == b'\r')
    else {
        return "";
    };
    let bytes = text.as_bytes();
    let mut rest_start = line_end + 1;
    if bytes[line_end] == b'\r' && bytes.get(line_end + 1) == Some(&b'\n') {
        rest_start += 1;
    }
    text[rest_start..]
        .strip_prefix("\r\n")
        .or_else(|| text[rest_start..].strip_prefix('\n'))
        .or_else(|| text[rest_start..].strip_prefix('\r'))
        .unwrap_or(&text[rest_start..])
}

fn is_openai_o_series(model: &str) -> bool {
    model.len() > 1
        && model.starts_with('o')
        && model
            .as_bytes()
            .get(1)
            .is_some_and(|byte| byte.is_ascii_digit())
}

fn supports_reasoning_effort(model: &str) -> bool {
    is_openai_o_series(model)
        || model
            .to_ascii_lowercase()
            .strip_prefix("gpt-")
            .and_then(|rest| rest.chars().next())
            .is_some_and(|ch| ch.is_ascii_digit() && ch >= '5')
}

fn anthropic_reasoning_effort(body: &Value) -> Option<&'static str> {
    if let Some(effort) = body
        .pointer("/output_config/effort")
        .and_then(Value::as_str)
    {
        return match effort {
            "low" => Some("low"),
            "medium" => Some("medium"),
            "high" => Some("high"),
            "max" => Some("xhigh"),
            _ => None,
        };
    }
    let thinking = body.get("thinking")?;
    match thinking.get("type").and_then(Value::as_str) {
        Some("adaptive") => Some("xhigh"),
        Some("enabled") => {
            let budget = thinking.get("budget_tokens").and_then(Value::as_u64);
            match budget {
                Some(value) if value < 4_000 => Some("low"),
                Some(value) if value < 16_000 => Some("medium"),
                Some(_) | None => Some("high"),
            }
        }
        _ => None,
    }
}

fn normalize_system_messages(messages: &mut Vec<Value>) {
    let mut system_chunks = Vec::new();
    messages.retain(|message| {
        if message.get("role").and_then(Value::as_str) != Some("system") {
            return true;
        }
        let text = openai_content_text(message.get("content"));
        if !text.trim().is_empty() {
            system_chunks.push(text);
        }
        false
    });
    if !system_chunks.is_empty() {
        messages.insert(
            0,
            json!({"role": "system", "content": system_chunks.join("\n\n")}),
        );
    }
}

fn unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone)]
struct ProtocolErrorShape {
    message: String,
    error_type: Option<String>,
    code: Option<Value>,
    param: Option<Value>,
}

fn extract_protocol_error(value: &Value) -> Option<ProtocolErrorShape> {
    let error = value.get("error").unwrap_or(value);
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| value.get("message").and_then(Value::as_str))
        .map(str::trim)
        .filter(|message| !message.is_empty())?
        .to_string();
    let error_type = error
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| error.get("status").and_then(Value::as_str))
        .or_else(|| value.get("type").and_then(Value::as_str))
        .or_else(|| value.get("status").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Some(ProtocolErrorShape {
        message,
        error_type,
        code: error.get("code").cloned(),
        param: error.get("param").cloned(),
    })
}

fn protocol_error_to_openai(error: ProtocolErrorShape) -> Value {
    json!({
        "error": {
            "message": error.message,
            "type": error.error_type.unwrap_or_else(|| "api_error".to_string()),
            "param": error.param.unwrap_or(Value::Null),
            "code": error.code.unwrap_or(Value::Null)
        }
    })
}

fn protocol_error_to_anthropic(error: ProtocolErrorShape) -> Value {
    json!({
        "type": "error",
        "error": {
            "type": error.error_type.unwrap_or_else(|| "api_error".to_string()),
            "message": error.message
        }
    })
}

fn protocol_error_to_gemini(error: ProtocolErrorShape) -> Value {
    let status = error
        .error_type
        .as_deref()
        .map(error_type_to_gemini_status)
        .unwrap_or("INTERNAL");
    let code = error
        .code
        .as_ref()
        .and_then(Value::as_u64)
        .unwrap_or_else(|| gemini_status_to_http_code(status));
    json!({
        "error": {
            "code": code,
            "message": error.message,
            "status": status
        }
    })
}

fn error_type_to_gemini_status(error_type: &str) -> &'static str {
    match error_type.trim().to_ascii_lowercase().as_str() {
        "invalid_request_error" | "bad_request" | "bad_model" | "invalid_argument" => {
            "INVALID_ARGUMENT"
        }
        "authentication_error" | "unauthorized" | "unauthenticated" => "UNAUTHENTICATED",
        "permission_error" | "forbidden" | "permission_denied" => "PERMISSION_DENIED",
        "not_found" | "model_not_found" => "NOT_FOUND",
        "rate_limit_error" | "rate_limit" | "resource_exhausted" => "RESOURCE_EXHAUSTED",
        "overloaded_error" | "api_error" | "internal" => "INTERNAL",
        "unavailable" => "UNAVAILABLE",
        _ => "INTERNAL",
    }
}

fn gemini_status_to_http_code(status: &str) -> u64 {
    match status {
        "INVALID_ARGUMENT" => 400,
        "UNAUTHENTICATED" => 401,
        "PERMISSION_DENIED" => 403,
        "NOT_FOUND" => 404,
        "RESOURCE_EXHAUSTED" => 429,
        "UNAVAILABLE" => 503,
        _ => 500,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_request_to_chat_maps_system_messages_and_tools() {
        let converted = convert_request_value(
            ConversionRoute::new(AiProtocol::AnthropicMessages, AiProtocol::OpenAiChat),
            json!({
                "model": "claude-sonnet",
                "system": "You are useful.",
                "messages": [{"role": "user", "content": "Hello"}],
                "tools": [{
                    "name": "Read",
                    "description": "Read file",
                    "input_schema": {"type": "object"}
                }],
                "stream": true
            }),
        )
        .unwrap();

        assert_eq!(converted["messages"][0]["role"], "system");
        assert_eq!(converted["tools"][0]["function"]["name"], "Read");
        assert_eq!(converted["stream_options"]["include_usage"], true);
    }

    #[test]
    fn responses_request_to_anthropic_maps_tool_outputs() {
        let converted = convert_request_value(
            ConversionRoute::new(AiProtocol::OpenAiResponses, AiProtocol::AnthropicMessages),
            json!({
                "model": "claude-sonnet",
                "input": [
                    {"role": "user", "content": [{"type": "input_text", "text": "hi"}]},
                    {"type": "function_call_output", "call_id": "call_1", "output": "ok"}
                ],
                "max_output_tokens": 128
            }),
        )
        .unwrap();

        assert_eq!(converted["max_tokens"], 128);
        assert_eq!(
            converted["messages"][1]["content"][0]["type"],
            "tool_result"
        );
    }

    #[test]
    fn chat_request_to_anthropic_maps_system_media_tool_calls_and_tool_results() {
        let converted = convert_request_value(
            ConversionRoute::new(AiProtocol::OpenAiChat, AiProtocol::AnthropicMessages),
            json!({
                "model": "gpt-5",
                "messages": [
                    {"role": "developer", "content": "developer rules"},
                    {"role": "system", "content": "system rules"},
                    {
                        "role": "user",
                        "content": [
                            {"type": "text", "text": "describe"},
                            {"type": "image_url", "image_url": {"url": "data:image/png;base64,aW1n"}}
                        ]
                    },
                    {
                        "role": "assistant",
                        "content": "I'll inspect",
                        "reasoning_content": "thinking",
                        "tool_calls": [{
                            "id": "call_1",
                            "type": "function",
                            "function": {"name": "read_file", "arguments": "{\"path\":\"a.txt\"}"}
                        }]
                    },
                    {"role": "tool", "tool_call_id": "call_1", "content": "ok"}
                ],
                "max_completion_tokens": 64,
                "stop": ["END"],
                "stream": true,
                "tools": [{
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "description": "Read file",
                        "parameters": {"type": "object"}
                    }
                }],
                "tool_choice": {"type": "function", "function": {"name": "read_file"}}
            }),
        )
        .unwrap();

        assert_eq!(converted["model"], "gpt-5");
        assert_eq!(converted["max_tokens"], 64);
        assert_eq!(converted["stop_sequences"][0], "END");
        assert_eq!(converted["system"], "developer rules\n\nsystem rules");
        assert_eq!(converted["messages"][0]["content"][1]["type"], "image");
        assert_eq!(converted["messages"][1]["content"][0]["type"], "thinking");
        assert_eq!(
            converted["messages"][1]["content"][2]["input"]["path"],
            "a.txt"
        );
        assert_eq!(
            converted["messages"][2]["content"][0]["type"],
            "tool_result"
        );
        assert_eq!(converted["tools"][0]["name"], "read_file");
        assert_eq!(converted["tool_choice"]["type"], "tool");
    }

    #[test]
    fn anthropic_request_to_responses_maps_images_thinking_tools_and_choice() {
        let converted = convert_request_value(
            ConversionRoute::new(AiProtocol::AnthropicMessages, AiProtocol::OpenAiResponses),
            json!({
                "model": "claude-sonnet",
                "system": [{"type": "text", "text": "system"}],
                "messages": [
                    {
                        "role": "user",
                        "content": [
                            {"type": "text", "text": "look"},
                            {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "aW1n"}}
                        ]
                    },
                    {
                        "role": "assistant",
                        "content": [
                            {"type": "thinking", "thinking": "think"},
                            {"type": "tool_use", "id": "toolu_1", "name": "read_file", "input": {"path": "a.txt"}}
                        ]
                    },
                    {
                        "role": "user",
                        "content": [{"type": "tool_result", "tool_use_id": "toolu_1", "content": "ok"}]
                    }
                ],
                "tools": [{"name": "read_file", "description": "Read file", "input_schema": {"type": "object"}}],
                "tool_choice": {"type": "tool", "name": "read_file"},
                "max_tokens": 32
            }),
        )
        .unwrap();

        assert_eq!(converted["instructions"], "system");
        assert_eq!(converted["max_output_tokens"], 32);
        assert_eq!(converted["input"][0]["content"][1]["type"], "input_image");
        assert_eq!(converted["input"][1]["type"], "reasoning");
        assert_eq!(converted["input"][2]["type"], "function_call");
        assert_eq!(converted["input"][2]["arguments"], "{\"path\":\"a.txt\"}");
        assert_eq!(converted["input"][3]["type"], "function_call_output");
        assert_eq!(converted["tools"][0]["name"], "read_file");
        assert_eq!(converted["tool_choice"]["type"], "function");
    }

    #[test]
    fn chat_response_to_responses_maps_tool_calls() {
        let converted = convert_response_value(
            ConversionRoute::new(AiProtocol::OpenAiChat, AiProtocol::OpenAiResponses),
            json!({
                "id": "chatcmpl_1",
                "model": "gpt-x",
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "ok",
                        "tool_calls": [{
                            "id": "call_1",
                            "type": "function",
                            "function": {"name": "Read", "arguments": "{\"path\":\"a\"}"}
                        }]
                    },
                    "finish_reason": "tool_calls"
                }],
                "usage": {"prompt_tokens": 10, "completion_tokens": 2, "total_tokens": 12}
            }),
        )
        .unwrap();

        assert_eq!(converted["output"][0]["type"], "message");
        assert_eq!(converted["output"][1]["type"], "function_call");
        assert_eq!(converted["usage"]["input_tokens"], 10);
    }

    #[test]
    fn anthropic_response_to_chat_maps_thinking_tool_usage_and_finish() {
        let converted = convert_response_value(
            ConversionRoute::new(AiProtocol::AnthropicMessages, AiProtocol::OpenAiChat),
            json!({
                "id": "msg_1",
                "model": "claude-sonnet",
                "content": [
                    {"type": "thinking", "thinking": "think"},
                    {"type": "text", "text": "answer"},
                    {"type": "tool_use", "id": "toolu_1", "name": "read_file", "input": {"path": "a.txt"}}
                ],
                "stop_reason": "tool_use",
                "usage": {
                    "input_tokens": 10,
                    "cache_read_input_tokens": 2,
                    "output_tokens": 4
                }
            }),
        )
        .unwrap();

        let message = &converted["choices"][0]["message"];
        assert_eq!(message["reasoning_content"], "think");
        assert_eq!(message["content"], "answer");
        assert_eq!(message["tool_calls"][0]["id"], "toolu_1");
        assert_eq!(
            message["tool_calls"][0]["function"]["arguments"],
            "{\"path\":\"a.txt\"}"
        );
        assert_eq!(converted["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(converted["usage"]["prompt_tokens"], 12);
        assert_eq!(
            converted["usage"]["prompt_tokens_details"]["cached_tokens"],
            2
        );
    }

    #[test]
    fn responses_response_to_anthropic_maps_reasoning_refusal_tool_usage_and_finish() {
        let converted = convert_response_value(
            ConversionRoute::new(AiProtocol::OpenAiResponses, AiProtocol::AnthropicMessages),
            json!({
                "id": "resp_1",
                "model": "gpt-test",
                "status": "completed",
                "output": [
                    {"type": "reasoning", "summary": [{"type": "summary_text", "text": "think"}]},
                    {
                        "type": "message",
                        "role": "assistant",
                        "content": [
                            {"type": "output_text", "text": "answer"},
                            {"type": "refusal", "refusal": "no"}
                        ]
                    },
                    {"type": "function_call", "call_id": "call_1", "name": "read_file", "arguments": "{\"path\":\"a.txt\"}"}
                ],
                "usage": {
                    "input_tokens": 12,
                    "output_tokens": 4,
                    "total_tokens": 16,
                    "input_tokens_details": {"cached_tokens": 2}
                }
            }),
        )
        .unwrap();

        assert_eq!(converted["content"][0]["type"], "thinking");
        assert_eq!(converted["content"][1]["text"], "answer");
        assert_eq!(converted["content"][2]["text"], "no");
        assert_eq!(converted["content"][3]["type"], "tool_use");
        assert_eq!(converted["content"][3]["input"]["path"], "a.txt");
        assert_eq!(converted["stop_reason"], "tool_use");
        assert_eq!(converted["usage"]["input_tokens"], 10);
        assert_eq!(converted["usage"]["cache_read_input_tokens"], 2);
    }

    #[test]
    fn responses_response_to_chat_maps_reasoning_tool_usage_and_incomplete_finish() {
        let converted = convert_response_value(
            ConversionRoute::new(AiProtocol::OpenAiResponses, AiProtocol::OpenAiChat),
            json!({
                "id": "resp_2",
                "model": "gpt-test",
                "status": "incomplete",
                "output": [
                    {"type": "reasoning", "summary": [{"type": "summary_text", "text": "think"}]},
                    {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "answer"}]},
                    {"type": "function_call", "call_id": "call_1", "name": "read_file", "arguments": "{\"path\":\"a.txt\"}"}
                ],
                "usage": {
                    "input_tokens": 12,
                    "output_tokens": 4,
                    "total_tokens": 16,
                    "input_tokens_details": {"cached_tokens": 2}
                }
            }),
        )
        .unwrap();

        let message = &converted["choices"][0]["message"];
        assert_eq!(message["reasoning_content"], "think");
        assert_eq!(message["content"], "answer");
        assert_eq!(message["tool_calls"][0]["id"], "call_1");
        assert_eq!(
            message["tool_calls"][0]["function"]["arguments"],
            "{\"path\":\"a.txt\"}"
        );
        assert_eq!(converted["choices"][0]["finish_reason"], "length");
        assert_eq!(
            converted["usage"]["prompt_tokens_details"]["cached_tokens"],
            2
        );
    }

    #[test]
    fn gemini_to_openai_route_stays_unsupported() {
        let error = convert_request_value(
            ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::OpenAiChat),
            json!({}),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ProtocolConversionError::UnsupportedRoute(_)
        ));
    }

    #[test]
    fn anthropic_request_to_gemini_maps_system_media_tools_and_tool_results() {
        let converted = convert_request_value(
            ConversionRoute::new(AiProtocol::AnthropicMessages, AiProtocol::GeminiNative),
            json!({
                "system": "You are useful.",
                "messages": [
                    {
                        "role": "user",
                        "content": [
                            {"type": "text", "text": "describe"},
                            {
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": "image/png",
                                    "data": "aW1n"
                                }
                            }
                        ]
                    },
                    {
                        "role": "assistant",
                        "content": [{
                            "type": "tool_use",
                            "id": "toolu_1",
                            "name": "read_file",
                            "input": {"path": "a.txt"}
                        }]
                    },
                    {
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": "toolu_1",
                            "content": "ok"
                        }]
                    }
                ],
                "tools": [{
                    "name": "read_file",
                    "description": "Read file",
                    "input_schema": {"type": "object", "properties": {"path": {"type": "string"}}}
                }],
                "tool_choice": {"type": "tool", "name": "read_file"},
                "max_tokens": 128,
                "temperature": 0.2,
                "top_p": 0.9
            }),
        )
        .unwrap();

        assert_eq!(
            converted["systemInstruction"]["parts"][0]["text"],
            "You are useful."
        );
        assert_eq!(converted["contents"][0]["role"], "user");
        assert_eq!(
            converted["contents"][0]["parts"][1]["inlineData"]["mimeType"],
            "image/png"
        );
        assert_eq!(
            converted["contents"][1]["parts"][0]["functionCall"]["name"],
            "read_file"
        );
        assert_eq!(
            converted["contents"][2]["parts"][0]["functionResponse"]["name"],
            "read_file"
        );
        assert_eq!(
            converted["tools"][0]["functionDeclarations"][0]["name"],
            "read_file"
        );
        assert_eq!(
            converted["toolConfig"]["functionCallingConfig"]["allowedFunctionNames"][0],
            "read_file"
        );
        assert_eq!(converted["generationConfig"]["maxOutputTokens"], 128);
    }

    #[test]
    fn gemini_request_to_anthropic_maps_generation_tools_and_stream() {
        let converted = convert_request_value(
            ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::AnthropicMessages),
            json!({
                "model": "claude-sonnet",
                "stream": true,
                "systemInstruction": {"parts": [{"text": "system"}]},
                "contents": [
                    {"role": "user", "parts": [{"text": "hi"}]},
                    {"role": "model", "parts": [{"functionCall": {"id": "call_1", "name": "read_file", "args": {"path": "a"}}}]},
                    {"role": "user", "parts": [{"functionResponse": {"id": "call_1", "name": "read_file", "response": {"content": "ok"}}}]}
                ],
                "generationConfig": {
                    "maxOutputTokens": 64,
                    "temperature": 0.3,
                    "topP": 0.8,
                    "stopSequences": ["stop"]
                },
                "tools": [{"functionDeclarations": [{"name": "read_file", "parameters": {"type": "object"}}]}],
                "toolConfig": {"functionCallingConfig": {"mode": "ANY", "allowedFunctionNames": ["read_file"]}}
            }),
        )
        .unwrap();

        assert_eq!(converted["model"], "claude-sonnet");
        assert_eq!(converted["stream"], true);
        assert_eq!(converted["system"], "system");
        assert_eq!(converted["max_tokens"], 64);
        assert_eq!(converted["messages"][1]["content"][0]["type"], "tool_use");
        assert_eq!(
            converted["messages"][2]["content"][0]["type"],
            "tool_result"
        );
        assert_eq!(converted["tools"][0]["name"], "read_file");
        assert_eq!(converted["tool_choice"]["type"], "tool");
    }

    #[test]
    fn gemini_response_to_anthropic_maps_text_tool_usage_and_finish() {
        let converted = convert_response_value(
            ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::AnthropicMessages),
            json!({
                "responseId": "resp_1",
                "modelVersion": "gemini-2.5-pro",
                "candidates": [{
                    "content": {
                        "role": "model",
                        "parts": [
                            {"text": "hello"},
                            {"functionCall": {"name": "read_file", "args": {"path": "a"}}}
                        ]
                    },
                    "finishReason": "STOP"
                }],
                "usageMetadata": {
                    "promptTokenCount": 12,
                    "cachedContentTokenCount": 2,
                    "candidatesTokenCount": 5,
                    "totalTokenCount": 17
                }
            }),
        )
        .unwrap();

        assert_eq!(converted["id"], "resp_1");
        assert_eq!(converted["content"][0]["text"], "hello");
        assert_eq!(converted["content"][1]["type"], "tool_use");
        assert_eq!(converted["content"][1]["id"], "gemini_synth_1");
        assert_eq!(converted["stop_reason"], "tool_use");
        assert_eq!(converted["usage"]["input_tokens"], 10);
        assert_eq!(converted["usage"]["cache_read_input_tokens"], 2);
        assert_eq!(converted["usage"]["output_tokens"], 5);
    }

    #[test]
    fn gemini_response_to_anthropic_handles_blocked_prompt_without_candidates() {
        let converted = convert_response_value(
            ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::AnthropicMessages),
            json!({
                "responseId": "resp_blocked",
                "promptFeedback": {"blockReason": "SAFETY"},
                "usageMetadata": {"promptTokenCount": 3, "totalTokenCount": 3}
            }),
        )
        .unwrap();

        assert_eq!(converted["stop_reason"], "refusal");
        assert_eq!(
            converted["content"][0]["text"],
            "Request blocked by Gemini safety filters: SAFETY"
        );
    }

    #[test]
    fn anthropic_response_to_gemini_maps_text_tool_and_usage() {
        let converted = convert_response_value(
            ConversionRoute::new(AiProtocol::AnthropicMessages, AiProtocol::GeminiNative),
            json!({
                "id": "msg_1",
                "model": "claude-sonnet",
                "content": [
                    {"type": "text", "text": "hello"},
                    {"type": "tool_use", "id": "toolu_1", "name": "read_file", "input": {"path": "a"}}
                ],
                "stop_reason": "tool_use",
                "usage": {
                    "input_tokens": 10,
                    "cache_read_input_tokens": 2,
                    "output_tokens": 4
                }
            }),
        )
        .unwrap();

        assert_eq!(converted["responseId"], "msg_1");
        assert_eq!(
            converted["candidates"][0]["content"]["parts"][0]["text"],
            "hello"
        );
        assert_eq!(
            converted["candidates"][0]["content"]["parts"][1]["functionCall"]["id"],
            "toolu_1"
        );
        assert_eq!(converted["usageMetadata"]["promptTokenCount"], 12);
        assert_eq!(converted["usageMetadata"]["candidatesTokenCount"], 4);
    }

    #[test]
    fn gemini_error_converts_to_anthropic_error_shape() {
        let converted = convert_error_response_body(
            ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::AnthropicMessages),
            br#"{"error":{"code":400,"message":"bad","status":"INVALID_ARGUMENT"}}"#,
        );
        let value: Value = serde_json::from_slice(&converted).unwrap();

        assert_eq!(value["type"], "error");
        assert_eq!(value["error"]["message"], "bad");
        assert_eq!(value["error"]["type"], "INVALID_ARGUMENT");
    }

    #[test]
    fn anthropic_error_converts_to_gemini_error_shape() {
        let converted = convert_error_response_body(
            ConversionRoute::new(AiProtocol::AnthropicMessages, AiProtocol::GeminiNative),
            br#"{"type":"error","error":{"type":"invalid_request_error","message":"bad request"}}"#,
        );
        let value: Value = serde_json::from_slice(&converted).unwrap();

        assert_eq!(value["error"]["code"], 400);
        assert_eq!(value["error"]["message"], "bad request");
        assert_eq!(value["error"]["status"], "INVALID_ARGUMENT");
    }

    #[test]
    fn anthropic_error_converts_to_openai_error_shape() {
        let converted = convert_error_response_body(
            ConversionRoute::new(AiProtocol::AnthropicMessages, AiProtocol::OpenAiResponses),
            br#"{"type":"error","error":{"type":"invalid_request_error","message":"bad request"}}"#,
        );
        let value: Value = serde_json::from_slice(&converted).unwrap();

        assert_eq!(value["error"]["message"], "bad request");
        assert_eq!(value["error"]["type"], "invalid_request_error");
    }

    #[test]
    fn openai_error_converts_to_anthropic_error_shape() {
        let converted = convert_error_response_body(
            ConversionRoute::new(AiProtocol::OpenAiChat, AiProtocol::AnthropicMessages),
            br#"{"error":{"message":"nope","type":"invalid_request_error","code":"bad_model","param":"model"}}"#,
        );
        let value: Value = serde_json::from_slice(&converted).unwrap();

        assert_eq!(value["type"], "error");
        assert_eq!(value["error"]["message"], "nope");
        assert_eq!(value["error"]["type"], "invalid_request_error");
    }

    #[test]
    fn non_json_error_body_is_preserved() {
        let body = b"upstream unavailable";
        let converted = convert_error_response_body(
            ConversionRoute::new(AiProtocol::OpenAiChat, AiProtocol::AnthropicMessages),
            body,
        );

        assert_eq!(converted, body);
    }
}
