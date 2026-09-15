use serde_json::{json, Value};
use std::collections::BTreeSet;

pub(super) fn function_call(call_id: &str, name: &str, arguments: Value) -> Value {
    json!({
        "type": "function_call", "id": format!("fc_{call_id}"),
        "call_id": call_id, "name": name, "arguments": arguments.to_string()
    })
}

pub(super) fn tool_output(call_id: &str, output: Value) -> Value {
    json!({"type": "function_call_output", "call_id": call_id, "output": output})
}

pub(super) fn parallel_input() -> Vec<Value> {
    vec![
        function_call("tool_a", "read_file", json!({"path": "first.txt"})),
        function_call("tool_b", "read_file", json!({"path": "second.txt"})),
        tool_output("tool_a", json!("first result")),
        tool_output("tool_b", json!("second result")),
    ]
}

pub(super) fn responses_request(input: Vec<Value>) -> Value {
    json!({
        "model": "fixture-model",
        "input": input,
        "parallel_tool_calls": true,
        "tools": [{
            "type": "function", "name": "read_file",
            "parameters": {
                "type": "object", "properties": {"path": {"type": "string"}}
            }
        }]
    })
}

/// Enforce the Chat contract independently of the Responses converter:
/// every assistant batch is answered once, before another non-tool message.
pub(super) fn validate_chat_tool_history(body: &Value) -> Result<(), String> {
    let messages = body["messages"].as_array().ok_or("messages missing")?;
    let mut pending = BTreeSet::new();
    for (index, message) in messages.iter().enumerate() {
        if message["role"] == "tool" {
            let call_id = message["tool_call_id"]
                .as_str()
                .ok_or("tool_call_id missing")?;
            if !pending.remove(call_id) {
                return Err(format!("message {index}: unexpected result {call_id}"));
            }
            continue;
        }
        if !pending.is_empty() {
            return Err(format!(
                "message {index}: unanswered tool calls {pending:?}"
            ));
        }
        if let Some(calls) = message["tool_calls"].as_array() {
            if message["role"] != "assistant" {
                return Err("tool_calls outside assistant message".to_string());
            }
            for call in calls {
                let call_id = call["id"].as_str().ok_or("tool call id missing")?;
                if !pending.insert(call_id) {
                    return Err(format!("duplicate call id {call_id}"));
                }
            }
        }
    }
    if !pending.is_empty() {
        return Err(format!("unanswered tool calls {pending:?}"));
    }
    Ok(())
}

pub(super) fn assert_chat_tool_history(body: &Value) {
    assert_eq!(validate_chat_tool_history(body), Ok(()), "{body}");
}

pub(super) fn chat_tool_response() -> Value {
    json!({
        "id": "chatcmpl_parallel_fixture", "object": "chat.completion",
        "created": 1, "model": "fixture-model",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant", "content": null,
                "tool_calls": [
                    {"id": "tool_a", "type": "function", "function": {
                        "name": "read_file", "arguments": "{\"path\":\"first.txt\"}"}},
                    {"id": "tool_b", "type": "function", "function": {
                        "name": "read_file", "arguments": "{\"path\":\"second.txt\"}"}}
                ]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
    })
}

pub(super) fn chat_tool_stream(delayed_ids: bool) -> Vec<u8> {
    let mut first = json!({"index": 0, "type": "function", "function": {
        "name": "read_file", "arguments": "{\"path\":"}});
    let mut second = json!({"index": 1, "type": "function", "function": {
        "name": "read_file", "arguments": "{\"path\":"}});
    if !delayed_ids {
        first["id"] = json!("tool_a");
        second["id"] = json!("tool_b");
    }
    let mut chunks = vec![
        json!({"id": "chatcmpl_parallel_fixture", "model": "fixture-model", "choices": [
            {"index": 0, "delta": {"role": "assistant", "reasoning_content": "Read both files."}, "finish_reason": null}
        ]}),
        json!({"id": "chatcmpl_parallel_fixture", "choices": [
            {"index": 0, "delta": {"tool_calls": [first, second]}, "finish_reason": null}
        ]}),
    ];
    if delayed_ids {
        chunks.push(json!({"id": "chatcmpl_parallel_fixture", "choices": [
            {"index": 0, "delta": {"tool_calls": [
                {"index": 1, "id": "tool_b"},
                {"index": 0, "id": "tool_a"}
            ]}, "finish_reason": null}
        ]}));
    }
    chunks.extend([
        json!({"id": "chatcmpl_parallel_fixture", "choices": [
            {"index": 0, "delta": {"tool_calls": [
                {"index": 1, "function": {"arguments": "\"second.txt\"}"}},
                {"index": 0, "function": {"arguments": "\"first.txt\"}"}}
            ]}, "finish_reason": null}
        ]}),
        json!({"id": "chatcmpl_parallel_fixture", "choices": [
            {"index": 0, "delta": {}, "finish_reason": "tool_calls"}
        ]}),
        json!({"id": "chatcmpl_parallel_fixture", "choices": [],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}}),
    ]);
    let mut text = chunks
        .iter()
        .map(|chunk| format!("data: {chunk}\n\n"))
        .collect::<String>();
    text.push_str("data: [DONE]\n\n");
    text.into_bytes()
}

pub(super) fn sse_events(bytes: &[u8]) -> Vec<Value> {
    std::str::from_utf8(bytes)
        .unwrap()
        .split("\n\n")
        .filter_map(|block| {
            let data = block
                .lines()
                .filter_map(|line| line.strip_prefix("data: "))
                .collect::<Vec<_>>()
                .join("\n");
            serde_json::from_str(&data).ok()
        })
        .collect()
}

pub(super) fn chat_tool_stream_with_late_text(delayed_ids: bool) -> Vec<u8> {
    let mut chunks = sse_events(&chat_tool_stream(delayed_ids));
    let finish_index = chunks
        .iter()
        .position(|chunk| chunk["choices"][0]["finish_reason"] == "tool_calls")
        .unwrap();
    chunks.insert(
        finish_index,
        json!({"id": "chatcmpl_parallel_fixture", "choices": [
            {"index": 0, "delta": {"content": "Reading both files now."}, "finish_reason": null}
        ]}),
    );
    let mut wire = chunks
        .iter()
        .map(|chunk| format!("data: {chunk}\n\n"))
        .collect::<String>();
    wire.push_str("data: [DONE]\n\n");
    wire.into_bytes()
}

pub(super) fn completed_response(events: &[Value]) -> Value {
    let completed = events
        .iter()
        .filter(|event| event["type"] == "response.completed")
        .collect::<Vec<_>>();
    assert_eq!(completed.len(), 1, "{events:?}");
    assert_eq!(completed[0]["response"]["status"], "completed");
    completed[0]["response"].clone()
}

#[test]
fn chat_contract_rejects_the_original_split_parallel_history() {
    let response = chat_tool_response();
    let calls = &response["choices"][0]["message"]["tool_calls"];
    let body = json!({"messages": [
        {"role": "assistant", "tool_calls": [calls[0].clone()]},
        {"role": "assistant", "tool_calls": [calls[1].clone()]},
        {"role": "tool", "tool_call_id": "tool_a", "content": "a"},
        {"role": "tool", "tool_call_id": "tool_b", "content": "b"}
    ]});
    assert!(validate_chat_tool_history(&body)
        .unwrap_err()
        .contains("unanswered tool calls"));
}

#[test]
fn chat_contract_rejects_missing_and_duplicate_results() {
    let response = chat_tool_response();
    let calls = &response["choices"][0]["message"]["tool_calls"];
    let mut body = json!({"messages": [
        {"role": "assistant", "tool_calls": calls},
        {"role": "tool", "tool_call_id": "tool_a", "content": "a"}
    ]});
    assert!(validate_chat_tool_history(&body).is_err());
    body["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role": "tool", "tool_call_id": "tool_a", "content": "a again"}));
    assert!(validate_chat_tool_history(&body)
        .unwrap_err()
        .contains("unexpected result"));
}
