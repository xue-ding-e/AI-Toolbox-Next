use super::{convert_request_value, AiProtocol, ConversionRoute};
use serde_json::{json, Value};

fn converted_request(source: AiProtocol, target: AiProtocol, body: Value) -> Value {
    convert_request_value(ConversionRoute::new(source, target), body).unwrap()
}

#[test]
fn anthropic_parallel_tool_result_images_batch_after_chat_tool_messages() {
    let converted = converted_request(
        AiProtocol::AnthropicMessages,
        AiProtocol::OpenAiChat,
        json!({
            "model": "claude-sonnet-4-5",
            "max_tokens": 128,
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {"type": "tool_use", "id": "call_1", "name": "inspect_a", "input": {}},
                        {"type": "tool_use", "id": "call_2", "name": "inspect_b", "input": {}}
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "call_1",
                            "content": [{
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": "image/png",
                                    "data": "PARALLEL_ONE"
                                },
                                "cache_control": {"type": "ephemeral"},
                                "prompt_cache_breakpoint": true
                            }]
                        },
                        {
                            "type": "tool_result",
                            "tool_use_id": "call_2",
                            "content": [{
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": "image/jpeg",
                                    "data": "PARALLEL_TWO"
                                }
                            }]
                        }
                    ]
                }
            ]
        }),
    );

    let messages = converted["messages"].as_array().unwrap();
    let roles = messages
        .iter()
        .map(|message| message["role"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(roles, vec!["assistant", "tool", "tool", "user"]);
    assert!(messages[1]["content"]
        .as_str()
        .unwrap()
        .contains("tool result media moved"));
    assert!(!messages[1]["content"]
        .as_str()
        .unwrap()
        .contains("PARALLEL_ONE"));
    assert!(!messages[2]["content"]
        .as_str()
        .unwrap()
        .contains("PARALLEL_TWO"));

    let media = messages[3]["content"].as_array().unwrap();
    assert_eq!(media.len(), 4);
    assert_eq!(
        media[1]["image_url"]["url"],
        "data:image/png;base64,PARALLEL_ONE"
    );
    assert_eq!(
        media[3]["image_url"]["url"],
        "data:image/jpeg;base64,PARALLEL_TWO"
    );
    assert!(media[1].get("cache_control").is_none());
    assert!(media[1].get("prompt_cache_breakpoint").is_none());
    assert!(media[1]["image_url"].get("cache_control").is_none());
    assert!(media[1]["image_url"]
        .get("prompt_cache_breakpoint")
        .is_none());
}

#[test]
fn responses_stringified_and_mcp_tool_images_move_out_of_chat_tool_text() {
    let nested_image = json!({
        "content": [{
            "type": "input_image",
            "image_url": {
                "url": "data:image/png;base64,NESTED_IMAGE",
                "detail": "high"
            }
        }]
    })
    .to_string();
    let converted = converted_request(
        AiProtocol::OpenAiResponses,
        AiProtocol::OpenAiChat,
        json!({
            "model": "gpt-5",
            "input": [
                {"type": "function_call", "call_id": "call_1", "name": "inspect_a", "arguments": "{}"},
                {"type": "function_call", "call_id": "call_2", "name": "inspect_b", "arguments": "{}"},
                {"type": "function_call_output", "call_id": "call_1", "output": nested_image},
                {
                    "type": "reasoning",
                    "summary": [{"type": "summary_text", "text": "keep tool outputs adjacent"}]
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_2",
                    "output": {
                        "type": "image",
                        "mimeType": "image/webp",
                        "data": "MCP_IMAGE"
                    }
                }
            ]
        }),
    );

    let messages = converted["messages"].as_array().unwrap();
    assert_eq!(
        messages
            .iter()
            .filter(|message| message["role"] == "user")
            .count(),
        1
    );
    let tool_messages = messages
        .iter()
        .filter(|message| message["role"] == "tool")
        .collect::<Vec<_>>();
    assert_eq!(tool_messages.len(), 2);
    assert!(tool_messages.iter().all(|message| {
        let content = message["content"].as_str().unwrap();
        !content.contains("NESTED_IMAGE") && !content.contains("MCP_IMAGE")
    }));

    let synthetic_user = messages.last().unwrap();
    assert_eq!(synthetic_user["role"], "user");
    let media = synthetic_user["content"].as_array().unwrap();
    assert_eq!(media.len(), 4);
    assert_eq!(
        media[1]["image_url"]["url"],
        "data:image/png;base64,NESTED_IMAGE"
    );
    assert_eq!(media[1]["image_url"]["detail"], "high");
    assert_eq!(
        media[3]["image_url"]["url"],
        "data:image/webp;base64,MCP_IMAGE"
    );
}

#[test]
fn anthropic_tool_image_becomes_responses_media_and_clamps_residual_base64() {
    let residual_base64 = "A".repeat(20_000);
    let tool_output = json!({
        "content": [
            {
                "type": "image_url",
                "image_url": "data:image/png;base64,RESPONSES_IMAGE"
            },
            {"type": "video", "data": residual_base64}
        ]
    })
    .to_string();
    let converted = converted_request(
        AiProtocol::AnthropicMessages,
        AiProtocol::OpenAiResponses,
        json!({
            "model": "gpt-5",
            "max_tokens": 128,
            "messages": [
                {
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": "call_image",
                        "name": "inspect",
                        "input": {}
                    }]
                },
                {
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": "call_image",
                        "content": tool_output
                    }]
                }
            ]
        }),
    );

    let output = converted["input"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "function_call_output")
        .unwrap()["output"]
        .as_array()
        .unwrap();
    let image = output
        .iter()
        .find(|part| part["type"] == "input_image")
        .unwrap();
    assert_eq!(image["image_url"], "data:image/png;base64,RESPONSES_IMAGE");
    let serialized = converted.to_string();
    assert!(serialized.contains("[ai-toolbox: omitted 20000 bytes]"));
    assert!(!serialized.contains(&"A".repeat(64)));
    assert!(output
        .iter()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .all(|text| !text.contains("RESPONSES_IMAGE")));
}

#[test]
fn anthropic_direct_mcp_tool_image_is_preserved_for_conversion() {
    let converted = converted_request(
        AiProtocol::AnthropicMessages,
        AiProtocol::OpenAiResponses,
        json!({
            "model": "gpt-5",
            "messages": [
                {
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": "call_mcp",
                        "name": "inspect",
                        "input": {}
                    }]
                },
                {
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": "call_mcp",
                        "content": [{
                            "type": "image",
                            "mimeType": "image/webp",
                            "data": "DIRECT_MCP_IMAGE"
                        }]
                    }]
                }
            ]
        }),
    );

    let output = converted["input"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "function_call_output")
        .unwrap()["output"]
        .as_array()
        .unwrap();
    assert_eq!(output[0]["type"], "input_text");
    assert!(!output[0]["text"]
        .as_str()
        .unwrap()
        .contains("DIRECT_MCP_IMAGE"));
    assert_eq!(output[1]["type"], "input_image");
    assert_eq!(
        output[1]["image_url"],
        "data:image/webp;base64,DIRECT_MCP_IMAGE"
    );
}

#[test]
fn responses_alternate_tool_images_restore_anthropic_native_blocks() {
    let nested_image = json!({
        "content": [{
            "type": "image_url",
            "image_url": "data:image/png;base64,STRINGIFIED_IMAGE"
        }]
    })
    .to_string();
    let converted = converted_request(
        AiProtocol::OpenAiResponses,
        AiProtocol::AnthropicMessages,
        json!({
            "model": "claude-sonnet-4-5",
            "input": [
                {"type": "function_call", "call_id": "call_1", "name": "inspect_a", "arguments": "{}"},
                {"type": "function_call", "call_id": "call_2", "name": "inspect_b", "arguments": "{}"},
                {"type": "function_call_output", "call_id": "call_1", "output": nested_image},
                {
                    "type": "function_call_output",
                    "call_id": "call_2",
                    "output": [{
                        "type": "image",
                        "mimeType": "image/webp",
                        "data": "MCP_ANTHROPIC_IMAGE"
                    }]
                }
            ]
        }),
    );

    let tool_results = converted["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|message| message["content"].as_array())
        .flatten()
        .filter(|block| block["type"] == "tool_result")
        .collect::<Vec<_>>();
    assert_eq!(tool_results.len(), 2);
    let images = tool_results
        .iter()
        .filter_map(|result| result["content"].as_array())
        .flatten()
        .filter(|block| block["type"] == "image")
        .collect::<Vec<_>>();
    assert_eq!(images.len(), 2);
    assert_eq!(images[0]["source"]["media_type"], "image/png");
    assert_eq!(images[0]["source"]["data"], "STRINGIFIED_IMAGE");
    assert_eq!(images[1]["source"]["media_type"], "image/webp");
    assert_eq!(images[1]["source"]["data"], "MCP_ANTHROPIC_IMAGE");
}

#[test]
fn gemini_2_and_3_use_their_supported_tool_media_shapes() {
    let source = |model: &str| {
        json!({
            "model": model,
            "max_tokens": 128,
            "messages": [
                {
                    "role": "assistant",
                    "content": [{
                        "type": "tool_use",
                        "id": "call_image",
                        "name": "inspect",
                        "input": {}
                    }]
                },
                {
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": "call_image",
                        "content": [{
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/jpeg",
                                "data": "GEMINI_TOOL_IMAGE"
                            }
                        }]
                    }]
                }
            ]
        })
    };

    let gemini_2 = converted_request(
        AiProtocol::AnthropicMessages,
        AiProtocol::GeminiNative,
        source("gemini-2.5-pro"),
    );
    let gemini_2_parts = gemini_2["contents"][1]["parts"].as_array().unwrap();
    assert_eq!(gemini_2_parts.len(), 3);
    assert!(gemini_2_parts[0]["functionResponse"].get("parts").is_none());
    assert_eq!(
        gemini_2_parts[1]["text"],
        "[ai-toolbox: media output of tool call call_image]"
    );
    assert_eq!(gemini_2_parts[2]["inlineData"]["data"], "GEMINI_TOOL_IMAGE");

    let gemini_3 = converted_request(
        AiProtocol::AnthropicMessages,
        AiProtocol::GeminiNative,
        source("gemini-3-pro-preview"),
    );
    let gemini_3_parts = gemini_3["contents"][1]["parts"].as_array().unwrap();
    assert_eq!(gemini_3_parts.len(), 1);
    assert_eq!(
        gemini_3_parts[0]["functionResponse"]["parts"][0]["inlineData"]["mimeType"],
        "image/jpeg"
    );
    assert_eq!(
        gemini_3_parts[0]["functionResponse"]["parts"][0]["inlineData"]["data"],
        "GEMINI_TOOL_IMAGE"
    );
}

#[test]
fn gemini_keeps_remote_and_malformed_tool_images_in_legacy_response() {
    for image_url in [
        "https://example.com/tool-image.png",
        "data:image/png,NOT_BASE64",
        "data:image/png;base64,",
    ] {
        let converted = converted_request(
            AiProtocol::OpenAiChat,
            AiProtocol::GeminiNative,
            json!({
                "model": "gemini-2.5-pro",
                "messages": [
                    {
                        "role": "assistant",
                        "tool_calls": [{
                            "id": "call_image",
                            "type": "function",
                            "function": {"name": "inspect", "arguments": "{}"}
                        }]
                    },
                    {
                        "role": "tool",
                        "tool_call_id": "call_image",
                        "content": json!({
                            "type": "image_url",
                            "image_url": {"url": image_url}
                        }).to_string()
                    }
                ]
            }),
        );

        let parts = converted["contents"][1]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 1);
        assert!(parts[0]["functionResponse"]["response"]
            .to_string()
            .contains(image_url));
        assert!(!converted.to_string().contains("\"inlineData\""));
        assert!(!converted.to_string().contains("\"fileData\""));
    }
}

#[test]
fn gemini_3_function_response_media_converts_to_all_native_target_shapes() {
    let source = json!({
        "model": "gemini-3-pro-preview",
        "contents": [
            {
                "role": "model",
                "parts": [{
                    "functionCall": {
                        "id": "call_image",
                        "name": "inspect",
                        "args": {}
                    }
                }]
            },
            {
                "role": "user",
                "parts": [{
                    "functionResponse": {
                        "id": "call_image",
                        "name": "inspect",
                        "response": {"content": "caption"},
                        "parts": [{
                            "inlineData": {
                                "mimeType": "image/png",
                                "data": "GEMINI_INBOUND_IMAGE"
                            }
                        }]
                    }
                }]
            }
        ]
    });

    let chat = converted_request(
        AiProtocol::GeminiNative,
        AiProtocol::OpenAiChat,
        source.clone(),
    );
    let chat_messages = chat["messages"].as_array().unwrap();
    assert_eq!(chat_messages.last().unwrap()["role"], "user");
    assert_eq!(
        chat_messages.last().unwrap()["content"][1]["image_url"]["url"],
        "data:image/png;base64,GEMINI_INBOUND_IMAGE"
    );

    let responses = converted_request(
        AiProtocol::GeminiNative,
        AiProtocol::OpenAiResponses,
        source.clone(),
    );
    let responses_output = responses["input"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["type"] == "function_call_output")
        .unwrap()["output"]
        .as_array()
        .unwrap();
    let responses_image = responses_output
        .iter()
        .find(|part| part["type"] == "input_image")
        .unwrap();
    assert_eq!(
        responses_image["image_url"],
        "data:image/png;base64,GEMINI_INBOUND_IMAGE"
    );

    let anthropic = converted_request(
        AiProtocol::GeminiNative,
        AiProtocol::AnthropicMessages,
        source,
    );
    let anthropic_tool_result = anthropic["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|message| message["content"].as_array())
        .flatten()
        .find(|block| block["type"] == "tool_result")
        .unwrap();
    let anthropic_image = anthropic_tool_result["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| block["type"] == "image")
        .unwrap();
    assert_eq!(anthropic_image["source"]["media_type"], "image/png");
    assert_eq!(anthropic_image["source"]["data"], "GEMINI_INBOUND_IMAGE");
}

#[test]
fn gemini_parallel_function_responses_preserve_every_tool_result() {
    let converted = converted_request(
        AiProtocol::GeminiNative,
        AiProtocol::OpenAiChat,
        json!({
            "model": "gemini-3-pro-preview",
            "contents": [
                {
                    "role": "model",
                    "parts": [
                        {
                            "functionCall": {
                                "id": "call_image",
                                "name": "inspect_image",
                                "args": {}
                            }
                        },
                        {
                            "functionCall": {
                                "id": "call_text",
                                "name": "inspect_text",
                                "args": {}
                            }
                        }
                    ]
                },
                {
                    "role": "user",
                    "parts": [
                        {
                            "functionResponse": {
                                "id": "call_image",
                                "name": "inspect_image",
                                "response": {"content": "image result"},
                                "parts": [{
                                    "inlineData": {
                                        "mimeType": "image/png",
                                        "data": "PARALLEL_GEMINI_IMAGE"
                                    }
                                }]
                            }
                        },
                        {
                            "functionResponse": {
                                "id": "call_text",
                                "name": "inspect_text",
                                "response": {"content": "text result"}
                            }
                        }
                    ]
                }
            ]
        }),
    );

    let messages = converted["messages"].as_array().unwrap();
    let roles = messages
        .iter()
        .map(|message| message["role"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(roles, vec!["assistant", "tool", "tool", "user"]);
    assert_eq!(messages[1]["tool_call_id"], "call_image");
    assert_eq!(messages[2]["tool_call_id"], "call_text");
    assert!(messages[2]["content"]
        .as_str()
        .unwrap()
        .contains("text result"));
    assert_eq!(
        messages[3]["content"][1]["image_url"]["url"],
        "data:image/png;base64,PARALLEL_GEMINI_IMAGE"
    );
}

#[test]
fn no_media_anthropic_tool_result_keeps_exact_chat_text_and_no_synthetic_turn() {
    let original = "{ \"status\": \"ok\", \"count\": 2 }";
    let converted = converted_request(
        AiProtocol::AnthropicMessages,
        AiProtocol::OpenAiChat,
        json!({
            "model": "claude-sonnet-4-5",
            "max_tokens": 128,
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": "call_plain",
                    "content": original
                }]
            }]
        }),
    );

    let messages = converted["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "tool");
    assert_eq!(messages[0]["content"], original);
}
