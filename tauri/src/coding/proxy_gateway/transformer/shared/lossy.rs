use super::super::types::{AiProtocol, ConversionRoute};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LossyConversionIssue {
    pub path: String,
    pub message: String,
}

pub fn check_lossy_conversion(route: ConversionRoute, value: &Value) -> Vec<LossyConversionIssue> {
    if route.identity() {
        return Vec::new();
    }
    let mut issues = Vec::new();
    if route.source == AiProtocol::OpenAiChat {
        check_openai_chat_lossy_items(route.target, value, &mut issues);
    }
    if route.source == AiProtocol::OpenAiResponses {
        check_openai_responses_lossy_items(route.target, value, &mut issues);
    }
    if route.source == AiProtocol::AnthropicMessages {
        check_anthropic_lossy_items(route.target, value, &mut issues);
    }
    if route.source == AiProtocol::GeminiNative {
        check_gemini_lossy_items(route.target, value, &mut issues);
    }
    issues
}

fn check_openai_chat_lossy_items(
    target: AiProtocol,
    value: &Value,
    issues: &mut Vec<LossyConversionIssue>,
) {
    if target == AiProtocol::OpenAiChat {
        return;
    }
    if let Some(modalities) = value.get("modalities").and_then(Value::as_array) {
        for (index, modality) in modalities.iter().enumerate() {
            if modality.as_str() != Some("text") {
                issues.push(LossyConversionIssue {
                    path: format!("/modalities/{index}"),
                    message: format!(
                        "OpenAI Chat modality '{}' cannot be represented in {target:?}",
                        modality.as_str().unwrap_or("<non-string>")
                    ),
                });
            }
        }
    }
    if value.get("audio").is_some() {
        issues.push(LossyConversionIssue {
            path: "/audio".to_string(),
            message: format!(
                "OpenAI Chat audio output options cannot be represented in {target:?}"
            ),
        });
    }
    if value.get("parallel_tool_calls").and_then(Value::as_bool) == Some(true)
        && !matches!(target, AiProtocol::OpenAiResponses)
    {
        issues.push(LossyConversionIssue {
            path: "/parallel_tool_calls".to_string(),
            message: format!("OpenAI Chat parallel_tool_calls cannot be represented in {target:?}"),
        });
    }
    let Some(messages) = value.get("messages").and_then(Value::as_array) else {
        return;
    };
    for (message_index, message) in messages.iter().enumerate() {
        let Some(content) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for (part_index, part) in content.iter().enumerate() {
            let Some(part_type) = part.get("type").and_then(Value::as_str) else {
                continue;
            };
            if is_openai_chat_content_part_lossy_for_target(part_type, target) {
                issues.push(LossyConversionIssue {
                    path: format!("/messages/{message_index}/content/{part_index}"),
                    message: format!(
                        "OpenAI Chat content part type '{part_type}' cannot be represented in {target:?}"
                    ),
                });
            }
        }
    }
}

fn is_openai_chat_content_part_lossy_for_target(part_type: &str, target: AiProtocol) -> bool {
    if target == AiProtocol::OpenAiChat {
        return false;
    }
    !matches!(part_type, "text" | "image_url")
}

fn check_anthropic_lossy_items(
    target: AiProtocol,
    value: &Value,
    issues: &mut Vec<LossyConversionIssue>,
) {
    if target == AiProtocol::AnthropicMessages {
        return;
    }
    check_anthropic_lossy_tools(value.get("tools"), issues);
    let Some(messages) = value.get("messages").and_then(Value::as_array) else {
        return;
    };
    for (message_index, message) in messages.iter().enumerate() {
        let Some(content) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for (part_index, part) in content.iter().enumerate() {
            let Some(block_type) = part.get("type").and_then(Value::as_str) else {
                continue;
            };
            if is_anthropic_content_block_lossy_for_non_anthropic(block_type) {
                issues.push(LossyConversionIssue {
                    path: format!("/messages/{message_index}/content/{part_index}"),
                    message: format!(
                        "Anthropic content block type '{block_type}' cannot be represented in {target:?}"
                    ),
                });
            }
        }
    }
}

fn check_anthropic_lossy_tools(value: Option<&Value>, issues: &mut Vec<LossyConversionIssue>) {
    let Some(Value::Array(tools)) = value else {
        return;
    };
    for (index, tool) in tools.iter().enumerate() {
        let Some(tool_type) = tool.get("type").and_then(Value::as_str) else {
            continue;
        };
        if is_anthropic_tool_lossy_for_non_anthropic(tool_type) {
            issues.push(LossyConversionIssue {
                path: format!("/tools/{index}"),
                message: format!(
                    "Anthropic native tool type '{tool_type}' cannot be represented outside Anthropic Messages"
                ),
            });
        }
    }
}

fn is_anthropic_content_block_lossy_for_non_anthropic(block_type: &str) -> bool {
    matches!(
        block_type,
        "server_tool_use"
            | "web_search_tool_use"
            | "web_search_tool_result"
            | "mcp_tool_use"
            | "mcp_tool_result"
    )
}

fn is_anthropic_tool_lossy_for_non_anthropic(tool_type: &str) -> bool {
    matches!(tool_type, "web_search_20250305" | "mcp")
}

fn check_openai_responses_lossy_items(
    target: AiProtocol,
    value: &Value,
    issues: &mut Vec<LossyConversionIssue>,
) {
    check_openai_responses_lossy_item_array(target, value.get("input"), "/input", issues);
    check_openai_responses_lossy_item_array(target, value.get("output"), "/output", issues);
    check_openai_responses_lossy_tools(target, value.get("tools"), issues);
}

fn check_openai_responses_lossy_item_array(
    target: AiProtocol,
    value: Option<&Value>,
    path_prefix: &str,
    issues: &mut Vec<LossyConversionIssue>,
) {
    let Some(value) = value else {
        return;
    };
    let items = match value {
        Value::Array(items) => items.as_slice(),
        Value::Object(_) => std::slice::from_ref(value),
        _ => return,
    };
    for (index, item) in items.iter().enumerate() {
        let Some(item_type) = item.get("type").and_then(Value::as_str) else {
            continue;
        };
        if is_openai_responses_item_lossy_for_target(item_type, target) {
            issues.push(LossyConversionIssue {
                path: format!("{path_prefix}/{index}"),
                message: format!(
                    "OpenAI Responses item type '{item_type}' cannot be represented in {target:?}"
                ),
            });
        }
        if let Some(content) = item.get("content").and_then(Value::as_array) {
            for (part_index, part) in content.iter().enumerate() {
                let Some(part_type) = part.get("type").and_then(Value::as_str) else {
                    continue;
                };
                if is_openai_responses_item_lossy_for_target(part_type, target) {
                    issues.push(LossyConversionIssue {
                        path: format!("{path_prefix}/{index}/content/{part_index}"),
                        message: format!(
                            "OpenAI Responses content part type '{part_type}' cannot be represented in {target:?}"
                        ),
                    });
                }
            }
        }
    }
}

fn check_openai_responses_lossy_tools(
    target: AiProtocol,
    value: Option<&Value>,
    issues: &mut Vec<LossyConversionIssue>,
) {
    let Some(Value::Array(tools)) = value else {
        return;
    };
    for (index, tool) in tools.iter().enumerate() {
        let Some(tool_type) = tool.get("type").and_then(Value::as_str) else {
            continue;
        };
        if is_openai_responses_tool_lossy_for_target(tool_type, target) {
            issues.push(LossyConversionIssue {
                path: format!("/tools/{index}"),
                message: format!(
                    "OpenAI Responses tool type '{tool_type}' cannot be represented in {target:?}"
                ),
            });
        }
    }
}

fn is_openai_responses_item_lossy_for_target(item_type: &str, target: AiProtocol) -> bool {
    if target == AiProtocol::OpenAiResponses {
        return false;
    }
    match item_type {
        "code_interpreter_call"
        | "computer_call"
        | "local_shell_call"
        | "file_search_call"
        | "web_search_call"
        | "image_generation_call"
        | "mcp_call"
        | "mcp_list_tools"
        | "mcp_approval_request"
        | "compaction"
        | "compaction_summary" => true,
        _ => false,
    }
}

fn is_openai_responses_tool_lossy_for_target(tool_type: &str, target: AiProtocol) -> bool {
    if target == AiProtocol::OpenAiResponses {
        return false;
    }
    match tool_type {
        "code_interpreter"
        | "computer_use_preview"
        | "file_search"
        | "web_search"
        | "web_search_preview"
        | "image_generation"
        | "mcp" => true,
        _ => false,
    }
}

fn check_gemini_lossy_items(
    target: AiProtocol,
    value: &Value,
    issues: &mut Vec<LossyConversionIssue>,
) {
    if target == AiProtocol::GeminiNative {
        return;
    }
    check_gemini_generation_config_lossy(target, value.get("generationConfig"), issues);
    if value.get("cachedContent").is_some() {
        issues.push(LossyConversionIssue {
            path: "/cachedContent".to_string(),
            message: format!("Gemini cachedContent cannot be represented in {target:?}"),
        });
    }
    if value.get("safetySettings").is_some() {
        issues.push(LossyConversionIssue {
            path: "/safetySettings".to_string(),
            message: format!("Gemini safetySettings cannot be represented in {target:?}"),
        });
    }
    check_gemini_tools_lossy(target, value.get("tools"), issues);
    let Some(contents) = value.get("contents").and_then(Value::as_array) else {
        return;
    };
    for (content_index, content) in contents.iter().enumerate() {
        let Some(parts) = content.get("parts").and_then(Value::as_array) else {
            continue;
        };
        for (part_index, part) in parts.iter().enumerate() {
            if let Some(inline_data) = part.get("inlineData").or_else(|| part.get("inline_data")) {
                check_gemini_media_part_lossy(
                    target,
                    inline_data,
                    &format!("/contents/{content_index}/parts/{part_index}/inlineData"),
                    issues,
                );
            }
            if let Some(file_data) = part.get("fileData").or_else(|| part.get("file_data")) {
                check_gemini_media_part_lossy(
                    target,
                    file_data,
                    &format!("/contents/{content_index}/parts/{part_index}/fileData"),
                    issues,
                );
            }
        }
    }
}

fn check_gemini_generation_config_lossy(
    target: AiProtocol,
    value: Option<&Value>,
    issues: &mut Vec<LossyConversionIssue>,
) {
    let Some(config) = value.and_then(Value::as_object) else {
        return;
    };
    for key in [
        "topK",
        "responseLogprobs",
        "logprobs",
        "responseModalities",
        "imageConfig",
    ] {
        if config.contains_key(key) {
            issues.push(LossyConversionIssue {
                path: format!("/generationConfig/{key}"),
                message: format!(
                    "Gemini generationConfig.{key} cannot be represented in {target:?}"
                ),
            });
        }
    }
}

fn check_gemini_tools_lossy(
    target: AiProtocol,
    value: Option<&Value>,
    issues: &mut Vec<LossyConversionIssue>,
) {
    let Some(Value::Array(tools)) = value else {
        return;
    };
    for (index, tool) in tools.iter().enumerate() {
        for key in ["googleSearch", "codeExecution", "urlContext"] {
            if tool.get(key).is_some() {
                issues.push(LossyConversionIssue {
                    path: format!("/tools/{index}/{key}"),
                    message: format!(
                        "Gemini native tool '{key}' cannot be represented in {target:?}"
                    ),
                });
            }
        }
    }
}

fn check_gemini_media_part_lossy(
    target: AiProtocol,
    value: &Value,
    path: &str,
    issues: &mut Vec<LossyConversionIssue>,
) {
    let mime = value
        .get("mimeType")
        .or_else(|| value.get("mime_type"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if mime.starts_with("image/") {
        return;
    }
    issues.push(LossyConversionIssue {
        path: path.to_string(),
        message: format!(
            "Gemini media MIME '{}' cannot be represented in {target:?}",
            if mime.is_empty() { "<missing>" } else { &mime }
        ),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_responses_code_interpreter_to_anthropic_as_lossy() {
        let issues = check_lossy_conversion(
            ConversionRoute::new(AiProtocol::OpenAiResponses, AiProtocol::AnthropicMessages),
            &json!({
                "model": "gpt-5",
                "input": [{
                    "type": "code_interpreter_call",
                    "code": "print(1)"
                }]
            }),
        );
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].path, "/input/0");
    }

    #[test]
    fn detects_responses_compaction_output_and_tool_as_lossy() {
        let issues = check_lossy_conversion(
            ConversionRoute::new(AiProtocol::OpenAiResponses, AiProtocol::OpenAiChat),
            &json!({
                "model": "gpt-5",
                "input": [{
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "compaction",
                        "id": "cmp_1",
                        "encrypted_content": "encrypted"
                    }]
                }],
                "output": [{
                    "type": "compaction_summary",
                    "id": "cmp_summary_1",
                    "encrypted_content": "encrypted_summary"
                }],
                "tools": [{
                    "type": "web_search_preview"
                }]
            }),
        );
        assert_eq!(
            issues
                .iter()
                .map(|issue| issue.path.as_str())
                .collect::<Vec<_>>(),
            vec!["/input/0/content/0", "/output/0", "/tools/0"]
        );
    }

    #[test]
    fn responses_function_and_custom_items_are_not_lossy() {
        let issues = check_lossy_conversion(
            ConversionRoute::new(AiProtocol::OpenAiResponses, AiProtocol::OpenAiChat),
            &json!({
                "model": "gpt-5",
                "input": [
                    {"type": "function_call", "call_id": "call_1", "name": "tool", "arguments": "{}"},
                    {"type": "custom_tool_call", "call_id": "call_2", "name": "custom", "input": "raw"},
                    {"type": "function_call_output", "call_id": "call_1", "output": "ok"}
                ],
                "tools": [
                    {"type": "function", "name": "tool", "parameters": {"type": "object"}},
                    {"type": "custom", "name": "custom"}
                ]
            }),
        );
        assert!(issues.is_empty());
    }

    #[test]
    fn detects_anthropic_server_tool_blocks_to_chat_as_lossy() {
        let issues = check_lossy_conversion(
            ConversionRoute::new(AiProtocol::AnthropicMessages, AiProtocol::OpenAiChat),
            &json!({
                "model": "claude-sonnet",
                "max_tokens": 1024,
                "tools": [{
                    "type": "web_search_20250305",
                    "name": "web_search"
                }],
                "messages": [{
                    "role": "assistant",
                    "content": [{
                        "type": "server_tool_use",
                        "id": "srv_1",
                        "name": "web_search",
                        "input": {"query": "rust"}
                    }, {
                        "type": "text",
                        "text": "done"
                    }]
                }, {
                    "role": "user",
                    "content": [{
                        "type": "mcp_tool_result",
                        "tool_use_id": "mcp_1",
                        "content": "ok"
                    }]
                }]
            }),
        );
        assert_eq!(
            issues
                .iter()
                .map(|issue| issue.path.as_str())
                .collect::<Vec<_>>(),
            vec!["/tools/0", "/messages/0/content/0", "/messages/1/content/0"]
        );
    }

    #[test]
    fn anthropic_function_tools_and_normal_content_are_not_lossy() {
        let issues = check_lossy_conversion(
            ConversionRoute::new(AiProtocol::AnthropicMessages, AiProtocol::OpenAiChat),
            &json!({
                "model": "claude-sonnet",
                "max_tokens": 1024,
                "tools": [{
                    "name": "Read",
                    "input_schema": {"type": "object"}
                }],
                "messages": [{
                    "role": "user",
                    "content": [{
                        "type": "text",
                        "text": "hello"
                    }]
                }]
            }),
        );
        assert!(issues.is_empty());
    }

    #[test]
    fn detects_openai_chat_audio_unknown_parts_and_parallel_tools_as_lossy() {
        let issues = check_lossy_conversion(
            ConversionRoute::new(AiProtocol::OpenAiChat, AiProtocol::AnthropicMessages),
            &json!({
                "model": "gpt-4o-audio-preview",
                "modalities": ["text", "audio"],
                "audio": {"voice": "alloy", "format": "wav"},
                "parallel_tool_calls": true,
                "messages": [{
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "transcribe"},
                        {"type": "input_audio", "input_audio": {"data": "abc", "format": "wav"}}
                    ]
                }]
            }),
        );

        assert_eq!(
            issues
                .iter()
                .map(|issue| issue.path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "/modalities/1",
                "/audio",
                "/parallel_tool_calls",
                "/messages/0/content/1"
            ]
        );
    }

    #[test]
    fn openai_chat_text_and_image_are_not_lossy_to_gemini() {
        let issues = check_lossy_conversion(
            ConversionRoute::new(AiProtocol::OpenAiChat, AiProtocol::GeminiNative),
            &json!({
                "model": "gpt-4o",
                "messages": [{
                    "role": "user",
                    "content": [
                        {"type": "text", "text": "describe"},
                        {"type": "image_url", "image_url": {"url": "data:image/png;base64,abc"}}
                    ]
                }]
            }),
        );

        assert!(issues.is_empty());
    }

    #[test]
    fn detects_gemini_native_tools_config_and_document_media_as_lossy() {
        let issues = check_lossy_conversion(
            ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::OpenAiChat),
            &json!({
                "generationConfig": {
                    "topK": 40,
                    "responseModalities": ["TEXT", "IMAGE"]
                },
                "cachedContent": "cachedContents/abc",
                "safetySettings": [{"category": "HARM_CATEGORY_DANGEROUS_CONTENT"}],
                "tools": [{
                    "googleSearch": {}
                }, {
                    "functionDeclarations": [{"name": "lookup", "parameters": {"type": "object"}}]
                }],
                "contents": [{
                    "role": "user",
                    "parts": [{
                        "text": "read"
                    }, {
                        "inlineData": {
                            "mimeType": "application/pdf",
                            "data": "abc"
                        }
                    }]
                }]
            }),
        );

        assert_eq!(
            issues
                .iter()
                .map(|issue| issue.path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "/generationConfig/topK",
                "/generationConfig/responseModalities",
                "/cachedContent",
                "/safetySettings",
                "/tools/0/googleSearch",
                "/contents/0/parts/1/inlineData"
            ]
        );
    }

    #[test]
    fn gemini_text_function_tools_and_images_are_not_lossy() {
        let issues = check_lossy_conversion(
            ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::OpenAiChat),
            &json!({
                "tools": [{
                    "functionDeclarations": [{"name": "lookup", "parameters": {"type": "object"}}]
                }],
                "contents": [{
                    "role": "user",
                    "parts": [{
                        "text": "describe"
                    }, {
                        "inlineData": {
                            "mimeType": "image/png",
                            "data": "abc"
                        }
                    }]
                }]
            }),
        );

        assert!(issues.is_empty());
    }
}
