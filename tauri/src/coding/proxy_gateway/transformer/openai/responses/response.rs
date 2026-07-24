use super::super::chat::openai_usage_to_llm;
use crate::coding::proxy_gateway::transformer::llm::{
    ApiFormat, Choice, Function, FunctionCall, ImageUrl, Message, MessageContent,
    MessageContentPart, Request, RequestType, Response, ResponseCustomToolCall, Tool, ToolCall,
    Usage, TOOL_TYPE_FUNCTION, TOOL_TYPE_RESPONSES_CUSTOM_TOOL,
};
use crate::coding::proxy_gateway::transformer::shared::signature::{
    decode_signature_for, encode_signature, SignatureProvider,
};
use crate::coding::proxy_gateway::transformer::shared::{
    extract_error_code, extract_error_message, extract_error_param, extract_error_type,
    normalize_function_parameters_owned, should_emit_openai_request_metadata, stop_from_value,
    stop_to_value, tool_choice_from_openai, tool_choice_to_responses,
};
use serde_json::{json, Map, Value};
use std::collections::HashMap;


const RESPONSES_INCLUDE_METADATA_KEY: &str = "openai_responses_include";
const RESPONSES_MAX_TOOL_CALLS_METADATA_KEY: &str = "openai_responses_max_tool_calls";
const RESPONSES_PROMPT_CACHE_RETENTION_METADATA_KEY: &str =
    "openai_responses_prompt_cache_retention";
const RESPONSES_TRUNCATION_METADATA_KEY: &str = "openai_responses_truncation";
const RESPONSES_COMPACTION_ENCRYPTED_CONTENT_METADATA_KEY: &str =
    "openai_responses_compaction_encrypted_content";
const RESPONSES_COMPACTION_CREATED_BY_METADATA_KEY: &str = "openai_responses_compaction_created_by";
const RESPONSES_RAW_TOOLS_METADATA_KEY: &str = "openai_responses_raw_tools";
const RESPONSES_TOOL_SIGNATURES_METADATA_KEY: &str = "openai_responses_tool_signatures";
const RESPONSES_RAW_TOOL_CHOICE_METADATA_KEY: &str = "openai_responses_raw_tool_choice";
const RESPONSES_RAW_INPUT_ITEMS_METADATA_KEY: &str = "openai_responses_raw_input_items";
/// Request top-level `reasoning.context` (e.g. `"all_turns"`), not input-item context.
const RESPONSES_REQUEST_REASONING_CONTEXT_METADATA_KEY: &str =
    "openai_responses_request_reasoning_context";

use super::shared::*;

pub fn responses_response_to_llm(body: Value) -> Response {
    let mut message = Message {
        role: "assistant".to_string(),
        ..Default::default()
    };
    let mut parts = Vec::new();
    let mut tool_calls = Vec::new();
    if let Some(output) = body.get("output").and_then(Value::as_array) {
        for item in output {
            match item.get("type").and_then(Value::as_str) {
                Some("message") => {
                    if let Some(content) = item.get("content") {
                        if let Some(content_array) = content.as_array() {
                            for part in content_array {
                                match part.get("type").and_then(Value::as_str) {
                                    Some("output_text") | Some("input_text") | Some("text") => {
                                        if let Some(annotations) =
                                            part.get("annotations").and_then(Value::as_array)
                                        {
                                            message.annotations.extend(annotations.iter().cloned());
                                        }
                                        if let Some(part) = responses_content_part_to_llm(part) {
                                            parts.push(part);
                                        }
                                    }
                                    Some("refusal") => {
                                        message.refusal = part
                                            .get("refusal")
                                            .and_then(Value::as_str)
                                            .unwrap_or_default()
                                            .to_string();
                                    }
                                    _ => {}
                                }
                            }
                        } else {
                            message
                                .annotations
                                .extend(content_annotations_from_value(content));
                            match responses_value_to_message_content(content) {
                                MessageContent::Text(text) => parts.push(MessageContentPart {
                                    part_type: "text".to_string(),
                                    text: Some(text),
                                    ..Default::default()
                                }),
                                MessageContent::Parts(content_parts) => parts.extend(content_parts),
                                MessageContent::Empty => {}
                            }
                        }
                    }
                }
                Some("output_text") | Some("input_text") | Some("text") => {
                    message.annotations.extend(part_annotations(item));
                    if let Some(part) = responses_content_part_to_llm(item) {
                        parts.push(part);
                    }
                }
                Some("function_call") | Some("custom_tool_call") => {
                    let index = tool_calls.len();
                    tool_calls.push(responses_call_to_tool_call(item, index));
                }
                Some("input_image") => {
                    if let Some(part) = responses_input_image_part(item) {
                        parts.push(part);
                    }
                }
                Some("compaction") | Some("compaction_summary") => {
                    parts.push(responses_compaction_part(item));
                }
                Some("reasoning") => {
                    let reasoning = responses_reasoning_text(item);
                    message.reasoning_content = reasoning.clone();
                    message.reasoning = reasoning;
                    message.reasoning_signature = item
                        .get("encrypted_content")
                        .and_then(Value::as_str)
                        .filter(|signature| !signature.is_empty())
                        .map(|signature| {
                            encode_signature(SignatureProvider::OpenAiResponses, signature)
                        });
                }
                _ => {}
            }
        }
    }
    message.content = MessageContent::Parts(parts);
    message.tool_calls = tool_calls;
    let has_tool = !message.tool_calls.is_empty();
    Response {
        id: body
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        object: "response".to_string(),
        model: body
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        created: body
            .get("created_at")
            .or_else(|| body.get("created"))
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        previous_response_id: body
            .get("previous_response_id")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        choices: vec![Choice {
            index: 0,
            message,
            finish_reason: Some(responses_status_to_finish(
                body.get("status").and_then(Value::as_str),
                has_tool,
            )),
            ..Default::default()
        }],
        usage: Some(responses_usage_to_llm(body.get("usage"))),
        ..Default::default()
    }
}

pub fn responses_compact_response_to_llm(body: Value) -> Response {
    let mut response = responses_response_to_llm(body);
    response.object = "response.compaction".to_string();
    response
}

pub fn llm_response_to_responses(response: Response) -> Value {
    let previous_response_id = response.previous_response_id.clone();
    let choice = response.choices.first().cloned().unwrap_or_default();
    let mut output = Vec::new();
    if let Some(reasoning_item) = responses_reasoning_item_from_message(&choice.message) {
        output.push(reasoning_item);
    }
    append_responses_message_content_items(
        "assistant".to_string(),
        choice.message.content.clone(),
        choice.message.annotations.clone(),
        choice.message.refusal.clone(),
        &mut output,
    );
    for tool_call in choice.message.tool_calls {
        output.push(tool_call_to_responses_item(tool_call));
    }
    let mut body = json!({
        "id": response.id,
        "object": "response",
        "created_at": response.created,
        "status": finish_to_responses_status(choice.finish_reason.as_deref()),
        "model": response.model,
        "output": output,
        "usage": usage_to_responses(response.usage.as_ref())
    });
    if let Some(previous_response_id) = previous_response_id {
        body["previous_response_id"] = json!(previous_response_id);
    }
    body
}

pub fn llm_response_to_responses_compact(response: Response) -> Value {
    let mut body = llm_response_to_responses(response);
    if let Some(object) = body.as_object_mut() {
        object.insert("object".to_string(), json!("response.compaction"));
        if !object.contains_key("status") && !object.contains_key("error") {
            object.insert("status".to_string(), json!("completed"));
        }
    }
    body
}

