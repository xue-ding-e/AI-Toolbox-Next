use super::shared::{
    merge_raw_responses_fragments_with_signatures,
    RESPONSES_COMPACTION_ENCRYPTED_CONTENT_METADATA_KEY, RESPONSES_RAW_TOOLS_METADATA_KEY,
    RESPONSES_REQUEST_REASONING_CONTEXT_METADATA_KEY,
    RESPONSES_TOOL_SIGNATURES_COMPLETE_METADATA_KEY, RESPONSES_TOOL_SIGNATURES_METADATA_KEY,
};
use super::{
    llm_request_to_responses, llm_request_to_responses_compact, llm_response_to_responses,
    responses_compact_request_to_llm, responses_request_to_llm, responses_response_to_llm,
};
use crate::coding::proxy_gateway::transformer::llm::{
    Choice, Function, Message, MessageContent, MessageContentPart, Request, Response, Tool,
    TOOL_TYPE_FUNCTION, TOOL_TYPE_RESPONSES_CUSTOM_TOOL,
};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::coding::proxy_gateway::transformer::llm::ResponseCustomTool;

#[test]
fn responses_request_accepts_raw_message_and_input_shapes() {
    let llm = responses_request_to_llm(json!({
        "model": "gpt-5",
        "input": [
            {"type": "input_text", "text": "standalone text"},
            {"type": "message", "role": "user", "content": "string content"},
            {
                "type": "function_call",
                "call_id": "call_lookup",
                "name": "lookup",
                "arguments": {"query": "rust"}
            },
            {
                "type": "function_call_output",
                "call_id": "call_lookup",
                "output": [{"type": "output_text", "text": "tool output"}]
            }
        ]
    }));

    assert_eq!(llm.messages.len(), 4);
    let first_parts = match &llm.messages[0].content {
        MessageContent::Parts(parts) => parts,
        other => panic!("expected standalone text part, got {other:?}"),
    };
    assert_eq!(first_parts[0].text.as_deref(), Some("standalone text"));
    assert_eq!(
        llm.messages[1].content,
        MessageContent::Text("string content".to_string())
    );
    assert_eq!(
        llm.messages[2].tool_calls[0].function.arguments,
        r#"{"query":"rust"}"#
    );
    assert_eq!(
        llm.messages[3].content,
        MessageContent::Text("tool output".to_string())
    );
}

#[test]
fn responses_request_preserves_raw_json_array_tool_output() {
    let llm = responses_request_to_llm(json!({
        "model": "gpt-5",
        "input": [{
            "type": "function_call_output",
            "call_id": "call_lookup",
            "output": [{"value": 1}]
        }]
    }));

    assert_eq!(
        llm.messages[0].content,
        MessageContent::Text(r#"[{"value":1}]"#.to_string())
    );
}

#[test]
fn responses_response_accepts_top_level_output_text() {
    let llm = responses_response_to_llm(json!({
        "id": "resp_text",
        "object": "response",
        "created_at": 123,
        "status": "completed",
        "model": "gpt-5",
        "output": [
            {"type": "output_text", "text": "hello", "annotations": [{"type": "url_citation"}]}
        ],
        "usage": {"input_tokens": 1, "output_tokens": 1, "total_tokens": 2}
    }));

    let message = &llm.choices[0].message;
    let parts = match &message.content {
        MessageContent::Parts(parts) => parts,
        other => panic!("expected output text part, got {other:?}"),
    };
    assert_eq!(parts[0].text.as_deref(), Some("hello"));
    assert_eq!(message.annotations, vec![json!({"type": "url_citation"})]);
}

#[test]
fn responses_response_accumulates_multiple_reasoning_items() {
    let llm = responses_response_to_llm(json!({
        "id": "resp_reasoning",
        "status": "completed",
        "model": "gpt-5",
        "output": [
            {
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "first thought"}],
                "encrypted_content": "cipher-first"
            },
            {
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "second thought"}]
            },
            {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "answer"}]
            }
        ]
    }));

    let message = &llm.choices[0].message;
    assert_eq!(
        message.reasoning_content.as_deref(),
        Some("first thought\nsecond thought")
    );
    assert_eq!(
        message.reasoning.as_deref(),
        message.reasoning_content.as_deref()
    );
    assert_eq!(
        message.reasoning_signature.as_deref(),
        Some("ai-toolbox.sig.openai_responses:cipher-first")
    );
}

#[test]
fn responses_non_stream_cancellation_status_roundtrips_without_completed() {
    for status in ["cancelled", "canceled"] {
        let response = responses_response_to_llm(json!({
            "id": "resp_cancelled",
            "model": "model-a",
            "created_at": 123,
            "status": status,
            "output": []
        }));
        assert_eq!(
            response.choices[0].finish_reason.as_deref(),
            Some("cancelled")
        );

        let converted = llm_response_to_responses(response);
        assert_eq!(converted["status"], "canceled");
    }
}

#[test]
fn responses_response_accepts_message_content_object() {
    let llm = responses_response_to_llm(json!({
        "id": "resp_content_object",
        "object": "response",
        "created_at": 123,
        "status": "completed",
        "model": "gpt-5",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": {
                "type": "output_text",
                "text": "object text",
                "annotations": [{"type": "url_citation"}]
            }
        }]
    }));

    let message = &llm.choices[0].message;
    let parts = match &message.content {
        MessageContent::Parts(parts) => parts,
        other => panic!("expected output text part, got {other:?}"),
    };
    assert_eq!(parts[0].text.as_deref(), Some("object text"));
    assert_eq!(message.annotations, vec![json!({"type": "url_citation"})]);
}

#[test]
fn responses_tools_omit_empty_optional_fields() {
    let responses = llm_request_to_responses(Request {
        model: "gpt-5".to_string(),
        tools: vec![
            Tool {
                tool_type: TOOL_TYPE_RESPONSES_CUSTOM_TOOL.to_string(),
                response_custom_tool: Some(ResponseCustomTool {
                    name: "freeform".to_string(),
                    description: String::new(),
                    format: None,
                }),
                ..Default::default()
            },
            Tool {
                tool_type: TOOL_TYPE_FUNCTION.to_string(),
                function: Some(Function {
                    name: "lookup".to_string(),
                    description: String::new(),
                    parameters: Some(json!({"type": "object"})),
                    ..Default::default()
                }),
                ..Default::default()
            },
        ],
        ..Default::default()
    });

    assert!(responses["tools"][0].get("description").is_none());
    assert!(responses["tools"][0].get("format").is_none());
    assert!(responses["tools"][1].get("description").is_none());
}

#[test]
fn responses_request_roundtrip_preserves_compaction_items() {
    let llm = responses_request_to_llm(json!({
        "model": "gpt-5",
        "input": [
            {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "before"}]
            },
            {
                "type": "compaction",
                "id": "cmp_1",
                "encrypted_content": "encrypted_compaction",
                "created_by": "model"
            },
            {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "after"}]
            }
        ]
    }));

    assert_eq!(llm.messages.len(), 3);
    let compaction_part = match &llm.messages[1].content {
        MessageContent::Parts(parts) => &parts[0],
        other => panic!("expected compaction part, got {other:?}"),
    };
    assert_eq!(compaction_part.part_type, "compaction");
    assert_eq!(compaction_part.id, "cmp_1");
    assert_eq!(
        compaction_part
            .transformer_metadata
            .get(RESPONSES_COMPACTION_ENCRYPTED_CONTENT_METADATA_KEY),
        Some(&json!("encrypted_compaction"))
    );

    let converted = llm_request_to_responses(llm);
    let input = converted["input"].as_array().expect("responses input");
    assert_eq!(input.len(), 3);
    assert_eq!(input[0]["type"], "message");
    assert_eq!(input[0]["content"][0]["text"], "before");
    assert_eq!(input[1]["type"], "compaction");
    assert_eq!(input[1]["id"], "cmp_1");
    assert_eq!(input[1]["encrypted_content"], "encrypted_compaction");
    assert_eq!(input[1]["created_by"], "model");
    assert_eq!(input[2]["type"], "message");
    assert_eq!(input[2]["content"][0]["text"], "after");
}

#[test]
fn responses_compact_roundtrip_preserves_raw_only_request_fragments() {
    let raw_tool_choice = json!({
        "type": "allowed_tools",
        "mode": "auto",
        "tools": [{"type": "mcp", "server_label": "local"}]
    });
    let compact = responses_compact_request_to_llm(json!({
        "model": "gpt-5",
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "before"}]
            },
            {
                "type": "local_shell_call",
                "id": "shell_1",
                "call_id": "call_shell",
                "status": "completed",
                "action": {"command": "pwd"}
            },
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "after"}]
            }
        ],
        "tools": [
            {"type": "function", "name": "known", "parameters": {"type": "object"}},
            {"type": "mcp", "server_label": "local", "server_url": "http://localhost:3000"},
            {"type": "custom", "name": "freeform"}
        ],
        "tool_choice": raw_tool_choice
    }));

    let converted = llm_request_to_responses_compact(compact);
    let input = converted["input"].as_array().expect("responses input");
    assert_eq!(input.len(), 3);
    assert_eq!(input[0]["type"], "message");
    assert_eq!(input[1]["type"], "local_shell_call");
    assert_eq!(input[1]["action"]["command"], "pwd");
    assert_eq!(input[2]["type"], "message");

    let tools = converted["tools"].as_array().expect("responses tools");
    assert_eq!(tools.len(), 3);
    assert_eq!(tools[0]["type"], "function");
    assert_eq!(tools[1]["type"], "mcp");
    assert_eq!(tools[1]["server_url"], "http://localhost:3000");
    assert_eq!(tools[2]["type"], "custom");
    assert_eq!(converted["tool_choice"], raw_tool_choice);
    assert!(converted.get("stream").is_none());
}

#[test]
fn responses_response_roundtrip_preserves_compaction_summary() {
    let llm = responses_response_to_llm(json!({
        "id": "resp_1",
        "object": "response",
        "created_at": 123,
        "status": "completed",
        "model": "gpt-5",
        "output": [
            {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "summary before"}]
            },
            {
                "type": "compaction_summary",
                "id": "cmp_summary_1",
                "encrypted_content": "encrypted_summary",
                "created_by": "model"
            }
        ],
        "usage": {"input_tokens": 1, "output_tokens": 2, "total_tokens": 3}
    }));

    let message = &llm.choices[0].message;
    let parts = match &message.content {
        MessageContent::Parts(parts) => parts,
        other => panic!("expected response parts, got {other:?}"),
    };
    assert_eq!(parts[0].part_type, "text");
    assert_eq!(parts[1].part_type, "compaction_summary");

    let converted = llm_response_to_responses(llm);
    let output = converted["output"].as_array().expect("responses output");
    assert_eq!(output.len(), 2);
    assert_eq!(output[0]["type"], "message");
    assert_eq!(output[0]["content"][0]["text"], "summary before");
    assert_eq!(output[1]["type"], "compaction_summary");
    assert_eq!(output[1]["id"], "cmp_summary_1");
    assert_eq!(output[1]["encrypted_content"], "encrypted_summary");
    assert_eq!(output[1]["created_by"], "model");
}

#[test]
fn llm_response_to_responses_preserves_text_compaction_text_order() {
    let mut metadata = HashMap::new();
    metadata.insert(
        RESPONSES_COMPACTION_ENCRYPTED_CONTENT_METADATA_KEY.to_string(),
        json!("encrypted_mid"),
    );
    let response = Response {
        id: "resp_order".to_string(),
        model: "gpt-5".to_string(),
        choices: vec![Choice {
            index: 0,
            message: Message {
                role: "assistant".to_string(),
                content: MessageContent::Parts(vec![
                    MessageContentPart {
                        part_type: "text".to_string(),
                        text: Some("first".to_string()),
                        ..Default::default()
                    },
                    MessageContentPart {
                        id: "cmp_mid".to_string(),
                        part_type: "compaction".to_string(),
                        transformer_metadata: metadata,
                        ..Default::default()
                    },
                    MessageContentPart {
                        part_type: "text".to_string(),
                        text: Some("second".to_string()),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            },
            finish_reason: Some("stop".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    };

    let converted = llm_response_to_responses(response);
    let output = converted["output"].as_array().expect("responses output");
    assert_eq!(output.len(), 3);
    assert_eq!(output[0]["type"], "message");
    assert_eq!(output[0]["content"][0]["text"], "first");
    assert_eq!(output[1]["type"], "compaction");
    assert_eq!(output[1]["id"], "cmp_mid");
    assert_eq!(output[1]["encrypted_content"], "encrypted_mid");
    assert_eq!(output[2]["type"], "message");
    assert_eq!(output[2]["content"][0]["text"], "second");
}

#[test]
fn trailing_reasoning_attaches_to_previous_assistant_before_user() {
    let body = json!({
        "model": "gpt-5",
        "input": [
            {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "done"}]
            },
            {
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "trailing thought"}]
            },
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "next"}]
            }
        ]
    });
    let request = responses_request_to_llm(body);
    let assistant = request
        .messages
        .iter()
        .find(|message| message.role == "assistant")
        .expect("assistant");
    assert_eq!(
        assistant.reasoning_content.as_deref(),
        Some("trailing thought")
    );
    assert!(
        !request
            .messages
            .iter()
            .any(|message| message.role == "assistant"
                && message.content.is_empty()
                && message.reasoning_content.as_deref() == Some("trailing thought")
                && message.tool_calls.is_empty()),
        "trailing reasoning must not remain a standalone assistant message"
    );
    let user = request
        .messages
        .iter()
        .find(|message| message.role == "user")
        .expect("user");
    let user_has_next = match &user.content {
        MessageContent::Text(text) => text == "next",
        MessageContent::Parts(parts) => parts
            .iter()
            .any(|part| part.text.as_deref() == Some("next")),
        MessageContent::Empty => false,
    };
    assert!(user_has_next, "user message should carry next turn text");
}

#[test]
fn trailing_reasoning_at_input_end_attaches_to_previous_assistant() {
    let body = json!({
        "model": "gpt-5",
        "input": [
            {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "answer"}],
            },
            {
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "end trailing"}],
                "context": {"foo": "bar"}
            }
        ]
    });
    let request = responses_request_to_llm(body);
    assert_eq!(request.messages.len(), 1);
    assert_eq!(
        request.messages[0].reasoning_content.as_deref(),
        Some("end trailing")
    );
    assert_eq!(
        request.messages[0]
            .transformer_metadata
            .get("openai_responses_reasoning_context"),
        Some(&json!({"foo": "bar"}))
    );
}

#[test]
fn forward_merge_still_combines_reasoning_with_following_function_call() {
    let body = json!({
        "model": "gpt-5",
        "input": [
            {
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "plan"}]
            },
            {
                "type": "function_call",
                "call_id": "call_1",
                "name": "lookup",
                "arguments": "{}"
            }
        ]
    });
    let request = responses_request_to_llm(body);
    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.messages[0].role, "assistant");
    assert_eq!(
        request.messages[0].reasoning_content.as_deref(),
        Some("plan")
    );
    assert_eq!(request.messages[0].tool_calls.len(), 1);
    assert_eq!(request.messages[0].tool_calls[0].id, "call_1");
}

#[test]
fn trailing_reasoning_appends_to_assistant_that_already_has_tool_calls() {
    let body = json!({
        "model": "gpt-5",
        "input": [
            {
                "type": "function_call",
                "call_id": "call_1",
                "name": "lookup",
                "arguments": "{}"
            },
            {
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "after tools"}]
            }
        ]
    });
    let request = responses_request_to_llm(body);
    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.messages[0].role, "assistant");
    assert_eq!(request.messages[0].tool_calls.len(), 1);
    assert_eq!(
        request.messages[0].reasoning_content.as_deref(),
        Some("after tools")
    );
}

#[test]
fn trailing_reasoning_appends_after_embedded_forward_merge() {
    // reasoning + following assistant message (forward merge), then another
    // trailing reasoning before user must append to that same assistant.
    let body = json!({
        "model": "gpt-5",
        "input": [
            {
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "first"}]
            },
            {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "visible"}]
            },
            {
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "second trailing"}]
            },
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "next"}]
            }
        ]
    });
    let request = responses_request_to_llm(body);
    let assistant = request
        .messages
        .iter()
        .find(|message| message.role == "assistant")
        .expect("assistant");
    let reasoning = assistant.reasoning_content.as_deref().unwrap_or_default();
    assert!(
        reasoning.contains("first") && reasoning.contains("second trailing"),
        "expected both forward-merged and trailing reasoning, got {reasoning:?}"
    );
}

#[test]
fn consecutive_reasoning_items_are_preserved_when_followed_by_assistant() {
    let body = json!({
        "model": "gpt-5",
        "input": [
            {
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "first thought"}]
            },
            {
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "second thought"}]
            },
            {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "answer"}]
            }
        ]
    });
    let request = responses_request_to_llm(body);
    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.messages[0].role, "assistant");
    let reasoning = request.messages[0]
        .reasoning_content
        .as_deref()
        .unwrap_or_default();
    assert_eq!(reasoning, "first thought\nsecond thought");
    let answer = match &request.messages[0].content {
        MessageContent::Text(text) => text.clone(),
        MessageContent::Parts(parts) => parts
            .iter()
            .filter_map(|part| part.text.as_deref())
            .collect::<Vec<_>>()
            .join(""),
        MessageContent::Empty => String::new(),
    };
    assert_eq!(answer, "answer");
}

#[test]
fn reasoning_without_same_turn_assistant_is_kept_before_user_boundary() {
    let body = json!({
        "model": "gpt-5",
        "input": [
            {
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "orphan thought"}]
            },
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "next"}]
            }
        ]
    });
    let request = responses_request_to_llm(body);
    assert_eq!(request.messages.len(), 2);
    assert_eq!(request.messages[0].role, "assistant");
    assert_eq!(
        request.messages[0].reasoning_content.as_deref(),
        Some("orphan thought")
    );
    assert_eq!(request.messages[1].role, "user");
}

#[test]
fn trailing_reasoning_does_not_cross_user_boundary() {
    let body = json!({
        "model": "gpt-5",
        "input": [
            {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "previous answer"}]
            },
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "new turn"}]
            },
            {
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "new thought"}]
            },
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "following user"}]
            }
        ]
    });
    let request = responses_request_to_llm(body);
    let previous_assistant = request
        .messages
        .iter()
        .find(|message| message.role == "assistant")
        .expect("previous assistant");
    assert!(previous_assistant.reasoning_content.is_none());
    assert!(
        request
            .messages
            .iter()
            .any(|message| message.role == "assistant"
                && message.reasoning_content.as_deref() == Some("new thought")),
        "reasoning must remain in the current turn instead of attaching to the previous user turn"
    );
}

#[test]
fn request_top_level_reasoning_context_roundtrips_without_clobbering_effort() {
    // Audit T-1: preserve request-level reasoning.context (e.g. "all_turns"),
    // not just input-item context. Outbound must merge effort + context.
    let body = json!({
        "model": "gpt-5",
        "reasoning": {
            "effort": "high",
            "context": "all_turns"
        },
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "hi"}]
            }
        ]
    });
    let request = responses_request_to_llm(body);
    assert_eq!(request.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(
        request
            .transformer_metadata
            .get(RESPONSES_REQUEST_REASONING_CONTEXT_METADATA_KEY),
        Some(&json!("all_turns"))
    );

    let converted = llm_request_to_responses(request);
    assert_eq!(converted["reasoning"]["effort"], "high");
    assert_eq!(converted["reasoning"]["context"], "all_turns");
}

#[test]
fn request_reasoning_context_only_without_effort_is_preserved() {
    let body = json!({
        "model": "gpt-5",
        "reasoning": { "context": "all_turns" },
        "input": "hello"
    });
    let request = responses_request_to_llm(body);
    assert!(request.reasoning_effort.is_none());
    let converted = llm_request_to_responses(request);
    assert!(converted["reasoning"].get("effort").is_none());
    assert_eq!(converted["reasoning"]["context"], "all_turns");
}

#[test]
fn responses_namespace_tools_expand_into_ir_functions() {
    // AxonHub 7d095b63: namespace children become function tools with ns__name.
    let body = json!({
        "model": "gpt-5",
        "input": "hello",
        "tools": [
            {
                "type": "function",
                "name": "top_level",
                "parameters": {"type": "object"}
            },
            {
                "type": "namespace",
                "name": "codex_app",
                "tools": [
                    {
                        "type": "function",
                        "name": "automation_update",
                        "description": "update automation",
                        "parameters": {"type": "object", "properties": {}}
                    },
                    {
                        "type": "function",
                        "name": "list",
                        "parameters": {"type": "object"}
                    }
                ]
            },
            {
                "type": "web_search"
            }
        ]
    });
    let request = responses_request_to_llm(body);
    let names: Vec<&str> = request
        .tools
        .iter()
        .filter_map(|tool| {
            tool.function
                .as_ref()
                .map(|function| function.name.as_str())
        })
        .collect();
    assert_eq!(
        names,
        vec![
            "top_level",
            "codex_app__automation_update",
            "codex_app__list"
        ]
    );

    // Namespace envelope is preserved as a raw fragment with represented_tool_count=2.
    let raw_tools = request
        .transformer_metadata
        .get(RESPONSES_RAW_TOOLS_METADATA_KEY)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let namespace_fragment = raw_tools
        .iter()
        .find(|fragment| {
            fragment.pointer("/value/type").and_then(Value::as_str) == Some("namespace")
        })
        .expect("namespace raw fragment");
    assert_eq!(namespace_fragment["represented_tool_count"], 2);
    assert_eq!(namespace_fragment["index"], 1);
    assert_eq!(
        request
            .transformer_metadata
            .get(RESPONSES_TOOL_SIGNATURES_COMPLETE_METADATA_KEY)
            .and_then(Value::as_bool),
        Some(true),
        "raw tools should mark signature sidecar complete for merge"
    );
    let signatures = request
        .transformer_metadata
        .get(RESPONSES_TOOL_SIGNATURES_METADATA_KEY)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        signatures,
        vec![
            json!("function:top_level"),
            json!("function:codex_app__automation_update"),
            json!("function:codex_app__list"),
        ]
    );

    // Direct merge should reinsert namespace and consume the two expanded children.
    let mut structured = vec![
        json!({"type":"function","name":"top_level","parameters":{"type":"object"}}),
        json!({"type":"function","name":"codex_app__automation_update","parameters":{"type":"object"}}),
        json!({"type":"function","name":"codex_app__list","parameters":{"type":"object"}}),
    ];
    let expected = vec![
        "function:top_level".to_string(),
        "function:codex_app__automation_update".to_string(),
        "function:codex_app__list".to_string(),
    ];
    let merged = merge_raw_responses_fragments_with_signatures(
        &mut structured,
        Some(&Value::Array(raw_tools.clone())),
        Some(expected.as_slice()),
    );
    let merged_types: Vec<&str> = merged
        .iter()
        .filter_map(|tool| tool.get("type").and_then(Value::as_str))
        .collect();
    assert_eq!(
        merged_types,
        vec!["function", "namespace", "web_search"],
        "merge should restore namespace envelope, got {merged:?}"
    );

    let roundtrip = llm_request_to_responses(request);
    let tools = roundtrip["tools"].as_array().expect("tools");
    let types: Vec<&str> = tools
        .iter()
        .filter_map(|tool| tool.get("type").and_then(Value::as_str))
        .collect();
    assert!(
        types.iter().any(|tool_type| *tool_type == "namespace"),
        "namespace envelope should round-trip, got {types:?} full={tools:?}"
    );
    assert!(
        types.iter().any(|tool_type| *tool_type == "function"),
        "top-level function should remain, got {types:?}"
    );
    assert!(
        types.iter().any(|tool_type| *tool_type == "web_search"),
        "raw-only web_search should remain, got {types:?}"
    );
    // Expanded children must not be double-emitted as plain functions after merge.
    let function_names: Vec<&str> = tools
        .iter()
        .filter(|tool| tool.get("type").and_then(Value::as_str) == Some("function"))
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect();
    assert_eq!(function_names, vec!["top_level"]);
}

#[test]
fn responses_usage_roundtrip_preserves_cache_write_tokens() {
    use super::shared::{responses_usage_to_llm, usage_to_responses};
    use crate::coding::proxy_gateway::transformer::llm::Usage;

    let wire = json!({
        "input_tokens": 100,
        "output_tokens": 20,
        "total_tokens": 120,
        "input_tokens_details": {
            "cached_tokens": 40,
            "cache_write_tokens": 15
        },
        "output_tokens_details": {
            "reasoning_tokens": 3
        }
    });
    let usage = responses_usage_to_llm(Some(&wire));
    assert_eq!(usage.cached_tokens, 40);
    assert_eq!(usage.cache_write_tokens, 15);
    assert_eq!(usage.reasoning_tokens, 3);

    let back = usage_to_responses(Some(&usage));
    assert_eq!(back["input_tokens_details"]["cached_tokens"], 40);
    assert_eq!(back["input_tokens_details"]["cache_write_tokens"], 15);
    assert_eq!(back["output_tokens_details"]["reasoning_tokens"], 3);

    let zero_write = usage_to_responses(Some(&Usage {
        prompt_tokens: 1,
        completion_tokens: 1,
        total_tokens: 2,
        cached_tokens: 0,
        cache_write_tokens: 0,
        reasoning_tokens: 0,
    }));
    assert!(zero_write["input_tokens_details"]
        .get("cache_write_tokens")
        .is_none());
}

#[test]
fn raw_tools_merge_drops_raw_tool_colliding_with_structured_signature() {
    // Structured function tool signature is collected on inbound; a raw tool
    // fragment with the same type:name must be dropped (T-3 fail-closed).
    let mut structured = vec![json!({
        "type": "function",
        "name": "lookup",
        "parameters": {"type": "object"}
    })];
    let raw_tools = json!([
        {
            "index": 0,
            "value": {
                "type": "function",
                "name": "lookup",
                "description": "raw collision should drop"
            }
        },
        {
            "index": 1,
            "value": {
                "type": "web_search"
            }
        }
    ]);
    let signatures = vec!["function:lookup".to_string()];
    let merged = merge_raw_responses_fragments_with_signatures(
        &mut structured,
        Some(&raw_tools),
        Some(signatures.as_slice()),
    );
    let names: Vec<String> = merged
        .iter()
        .filter_map(|tool| {
            tool.get("name")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .or_else(|| {
                    tool.get("type")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                })
        })
        .collect();
    assert!(
        names
            .iter()
            .filter(|name| name.as_str() == "lookup")
            .count()
            == 1,
        "colliding raw function:lookup must be dropped, got {names:?}"
    );
    assert!(
        names.iter().any(|name| name == "web_search"),
        "non-colliding raw tool should remain, got {names:?}"
    );
}

#[test]
fn raw_tools_merge_skips_fragments_when_structured_signatures_are_reordered() {
    let mut structured = vec![
        json!({
            "type": "function",
            "name": "second",
            "parameters": {"type": "object"}
        }),
        json!({
            "type": "function",
            "name": "first",
            "parameters": {"type": "object"}
        }),
    ];
    let raw_tools = json!([
        {
            "index": 1,
            "value": {
                "type": "web_search"
            }
        }
    ]);
    let expected_signatures = vec!["function:first".to_string(), "function:second".to_string()];

    let merged = merge_raw_responses_fragments_with_signatures(
        &mut structured,
        Some(&raw_tools),
        Some(expected_signatures.as_slice()),
    );

    assert_eq!(merged.len(), 2);
    assert!(merged.iter().all(|tool| tool["type"] == "function"));
    assert_eq!(merged[0]["name"], "second");
    assert_eq!(merged[1]["name"], "first");
}

#[test]
fn raw_only_tools_preserve_an_empty_structured_signature_sidecar() {
    let request = responses_request_to_llm(json!({
        "model": "gpt-5",
        "input": "hello",
        "tools": [
            {
                "type": "web_search"
            }
        ]
    }));

    assert_eq!(
        request
            .transformer_metadata
            .get(RESPONSES_RAW_TOOLS_METADATA_KEY)
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        request
            .transformer_metadata
            .get(RESPONSES_TOOL_SIGNATURES_METADATA_KEY),
        Some(&json!([]))
    );
    assert_eq!(
        request
            .transformer_metadata
            .get(RESPONSES_TOOL_SIGNATURES_COMPLETE_METADATA_KEY),
        Some(&json!(true))
    );
}

#[test]
fn raw_tools_merge_skips_fragments_when_structured_tool_is_added_to_empty_signature_set() {
    let request = responses_request_to_llm(json!({
        "model": "gpt-5",
        "input": "hello",
        "tools": [
            {
                "type": "web_search"
            }
        ]
    }));
    let mut structured = vec![json!({
        "type": "function",
        "name": "lookup",
        "parameters": {"type": "object"}
    })];
    let raw_tools = request
        .transformer_metadata
        .get(RESPONSES_RAW_TOOLS_METADATA_KEY);
    let expected_signatures = request
        .transformer_metadata
        .get(RESPONSES_TOOL_SIGNATURES_METADATA_KEY)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .expect("raw-only tools should preserve an empty signature sidecar");

    let merged = merge_raw_responses_fragments_with_signatures(
        &mut structured,
        raw_tools,
        Some(expected_signatures.as_slice()),
    );

    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0]["type"], "function");
    assert_eq!(merged[0]["name"], "lookup");
}

#[test]
fn raw_tools_merge_fails_closed_when_any_structured_tool_has_no_signature() {
    let mut structured = vec![
        json!({
            "type": "function",
            "name": "lookup",
            "parameters": {"type": "object"}
        }),
        json!({
            "type": "function",
            "parameters": {"type": "object"}
        }),
    ];
    let raw_tools = json!([
        {
            "index": 1,
            "value": {
                "type": "web_search"
            }
        }
    ]);
    let expected_signatures = vec!["function:lookup".to_string()];

    let merged = merge_raw_responses_fragments_with_signatures(
        &mut structured,
        Some(&raw_tools),
        Some(expected_signatures.as_slice()),
    );

    assert_eq!(merged.len(), 2);
    assert!(merged.iter().all(|tool| tool["type"] == "function"));
    assert!(merged.iter().all(|tool| tool["type"] != "web_search"));
}

#[test]
fn responses_raw_tool_merge_fails_closed_for_unnameable_structured_tool() {
    let request = responses_request_to_llm(json!({
        "model": "gpt-5",
        "input": "hello",
        "tools": [
            {
                "type": "function",
                "name": "lookup",
                "parameters": {"type": "object"}
            },
            {
                "type": "function"
            },
            {
                "type": "web_search"
            }
        ]
    }));

    assert_eq!(
        request
            .transformer_metadata
            .get(RESPONSES_TOOL_SIGNATURES_COMPLETE_METADATA_KEY),
        Some(&json!(false))
    );

    let converted = llm_request_to_responses(request);
    let tools = converted["tools"]
        .as_array()
        .expect("the valid structured tool should remain");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["type"], "function");
    assert_eq!(tools[0]["name"], "lookup");
    assert!(
        !tools.iter().any(|tool| tool["type"] == "web_search"),
        "raw tools must not be reinserted when a structured tool has no signature"
    );
}
