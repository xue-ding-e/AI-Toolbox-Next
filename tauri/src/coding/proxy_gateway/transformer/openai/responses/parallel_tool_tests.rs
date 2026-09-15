use super::{llm_request_to_responses, responses_request_to_llm};
use crate::coding::proxy_gateway::transformer::llm::{
    Message, TOOL_TYPE_FUNCTION, TOOL_TYPE_RESPONSES_CUSTOM_TOOL,
};
use serde_json::{json, Value};

fn function_call(call_id: &str, name: &str, arguments: &str) -> Value {
    json!({
        "type": "function_call",
        "id": format!("fc_{call_id}"),
        "call_id": call_id,
        "name": name,
        "arguments": arguments
    })
}

fn custom_call(call_id: &str) -> Value {
    json!({
        "type": "custom_tool_call",
        "call_id": call_id,
        "name": "apply_patch",
        "input": "*** Begin Patch\n*** End Patch"
    })
}

fn output(call_id: &str) -> Value {
    json!({"type": "function_call_output", "call_id": call_id, "output": format!("result {call_id}")})
}

fn reasoning(text: &str) -> Value {
    json!({"type": "reasoning", "summary": [{"type": "summary_text", "text": text}]})
}

fn messages(input: Vec<Value>) -> Vec<Message> {
    responses_request_to_llm(json!({"model": "fixture-model", "input": input})).messages
}

fn call_ids(message: &Message) -> Vec<&str> {
    message
        .tool_calls
        .iter()
        .map(|call| call.id.as_str())
        .collect()
}

#[test]
fn parallel_tools_share_one_assistant_message() {
    let messages = messages(vec![
        function_call("call_a", "read_file", r#"{"path":"a"}"#),
        function_call("call_b", "list_files", r#"{"path":"b"}"#),
        output("call_a"),
        output("call_b"),
    ]);
    assert_eq!(messages.len(), 3);
    assert_eq!(call_ids(&messages[0]), ["call_a", "call_b"]);
    assert_eq!(
        messages[0].tool_calls[0].function.arguments,
        r#"{"path":"a"}"#
    );
    assert_eq!(
        messages[0].tool_calls[1].function.arguments,
        r#"{"path":"b"}"#
    );
    assert_eq!(messages[1].tool_call_id.as_deref(), Some("call_a"));
    assert_eq!(messages[2].tool_call_id.as_deref(), Some("call_b"));
}

#[test]
fn parallel_same_name_tools_keep_distinct_ids_and_indices() {
    let messages = messages(vec![
        function_call("exec_command:14", "exec_command", r#"{"cmd":"first"}"#),
        function_call("exec_command:15", "exec_command", r#"{"cmd":"second"}"#),
        function_call("tool_native", "exec_command", "{}"),
    ]);
    assert_eq!(messages.len(), 1);
    assert_eq!(
        call_ids(&messages[0]),
        ["exec_command:14", "exec_command:15", "tool_native"]
    );
    assert_eq!(
        messages[0]
            .tool_calls
            .iter()
            .map(|call| call.index)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
}

#[test]
fn parallel_mixed_function_and_custom_tools_keep_native_fields() {
    let messages = messages(vec![
        function_call("call_a", "read_file", "{}"),
        custom_call("call_patch"),
        function_call("call_b", "read_file", r#"{"path":"b"}"#),
    ]);
    assert_eq!(messages.len(), 1);
    assert_eq!(call_ids(&messages[0]), ["call_a", "call_patch", "call_b"]);
    assert_eq!(messages[0].tool_calls[0].tool_type, TOOL_TYPE_FUNCTION);
    let custom = &messages[0].tool_calls[1];
    assert_eq!(custom.tool_type, TOOL_TYPE_RESPONSES_CUSTOM_TOOL);
    let native = custom.response_custom_tool_call.as_ref().unwrap();
    assert_eq!(native.call_id, "call_patch");
    assert_eq!(native.name, "apply_patch");
    assert_eq!(native.input, "*** Begin Patch\n*** End Patch");
}

#[test]
fn parallel_custom_tools_form_one_batch() {
    let messages = messages(vec![custom_call("patch_a"), custom_call("patch_b")]);
    assert_eq!(messages.len(), 1);
    assert_eq!(call_ids(&messages[0]), ["patch_a", "patch_b"]);
    assert!(messages[0]
        .tool_calls
        .iter()
        .all(|call| call.response_custom_tool_call.is_some()));
}

#[test]
fn leading_reasoning_belongs_to_the_entire_parallel_batch() {
    let messages = messages(vec![
        reasoning("Inspect both files"),
        function_call("call_a", "read_file", "{}"),
        function_call("call_b", "read_file", "{}"),
        output("call_a"),
        output("call_b"),
    ]);
    assert_eq!(messages.len(), 3);
    assert_eq!(call_ids(&messages[0]), ["call_a", "call_b"]);
    assert_eq!(
        messages[0].reasoning_content.as_deref(),
        Some("Inspect both files")
    );
    assert_eq!(messages[0].reasoning, messages[0].reasoning_content);
}

#[test]
fn consecutive_reasoning_keeps_signature_and_context_on_the_batch() {
    let mut first = reasoning("First");
    first["encrypted_content"] = json!("fixture-signature");
    first["context"] = json!({"turn": "current"});
    let request = responses_request_to_llm(json!({
        "model": "fixture-model",
        "input": [
            first, reasoning("Second"),
            function_call("call_a", "read_file", "{}"), custom_call("call_patch")
        ]
    }));
    assert_eq!(request.messages.len(), 1);
    assert_eq!(
        request.messages[0].reasoning_content.as_deref(),
        Some("First\nSecond")
    );
    assert_eq!(call_ids(&request.messages[0]), ["call_a", "call_patch"]);
    let restored = llm_request_to_responses(request);
    assert_eq!(
        restored["input"][0]["encrypted_content"],
        "fixture-signature"
    );
    assert_eq!(restored["input"][0]["context"], json!({"turn": "current"}));
    assert_eq!(restored["input"][1]["call_id"], "call_a");
    assert_eq!(restored["input"][2]["call_id"], "call_patch");
}

#[test]
fn reasoning_between_parallel_calls_does_not_split_the_batch() {
    let messages = messages(vec![
        function_call("call_a", "read_file", "{}"),
        reasoning("Also inspect the second file"),
        function_call("call_b", "read_file", "{}"),
        output("call_a"),
        output("call_b"),
    ]);
    assert_eq!(messages.len(), 3);
    assert_eq!(call_ids(&messages[0]), ["call_a", "call_b"]);
    assert_eq!(
        messages[0].reasoning_content.as_deref(),
        Some("Also inspect the second file")
    );
}

#[test]
fn reasoning_before_between_and_after_calls_is_appended_once() {
    let messages = messages(vec![
        reasoning("before"),
        function_call("call_a", "read_file", "{}"),
        reasoning("between"),
        custom_call("call_patch"),
        reasoning("after"),
    ]);
    assert_eq!(messages.len(), 1);
    assert_eq!(call_ids(&messages[0]), ["call_a", "call_patch"]);
    assert_eq!(
        messages[0].reasoning_content.as_deref(),
        Some("before\nbetween\nafter")
    );
}

#[test]
fn reasoning_between_parallel_results_keeps_results_adjacent() {
    let messages = messages(vec![
        function_call("call_a", "read_file", "{}"),
        function_call("call_b", "read_file", "{}"),
        output("call_a"),
        reasoning("trailing"),
        output("call_b"),
    ]);
    assert_eq!(messages.len(), 3);
    assert_eq!(call_ids(&messages[0]), ["call_a", "call_b"]);
    assert_eq!(messages[0].reasoning_content.as_deref(), Some("trailing"));
    assert_eq!(messages[1].role, "tool");
    assert_eq!(messages[2].role, "tool");
}

#[test]
fn serial_calls_are_not_merged_across_results() {
    let messages = messages(vec![
        function_call("call_a", "read_file", "{}"),
        output("call_a"),
        reasoning("next generation"),
        function_call("call_b", "read_file", "{}"),
        output("call_b"),
    ]);
    assert_eq!(messages.len(), 4);
    assert_eq!(call_ids(&messages[0]), ["call_a"]);
    assert_eq!(call_ids(&messages[2]), ["call_b"]);
    assert!(messages[0].reasoning_content.is_none());
    assert_eq!(
        messages[2].reasoning_content.as_deref(),
        Some("next generation")
    );
    assert_eq!(messages[2].tool_calls[0].index, 0);
}

#[test]
fn parallel_batches_stop_at_non_assistant_message_and_raw_item_boundaries() {
    for boundary in [
        json!({"role": "user", "content": "new turn"}),
        json!({"role": "system", "content": "new instructions"}),
        json!({"role": "developer", "content": "new instructions"}),
        json!({"type": "input_text", "text": "new input"}),
        json!({"type": "input_image", "image_url": "https://example.com/image.png"}),
        json!({"type": "web_search_call", "id": "search_fixture", "status": "completed"}),
        json!({"type": "compaction", "id": "cmp_fixture", "encrypted_content": "fixture-compaction"}),
    ] {
        let messages = messages(vec![
            function_call("call_a", "read_file", "{}"),
            boundary.clone(),
            function_call("call_b", "read_file", "{}"),
        ]);
        let batches = messages
            .iter()
            .filter(|message| !message.tool_calls.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(batches.len(), 2, "boundary: {boundary}");
        assert_eq!(call_ids(batches[0]), ["call_a"], "boundary: {boundary}");
        assert_eq!(call_ids(batches[1]), ["call_b"], "boundary: {boundary}");
    }
}

#[test]
fn parallel_batches_after_user_boundary_keep_reasoning_in_the_new_turn() {
    let messages = messages(vec![
        function_call("old", "read_file", "{}"),
        output("old"),
        json!({"role": "user", "content": "new turn"}),
        reasoning("new reasoning"),
        function_call("new_a", "read_file", "{}"),
        function_call("new_b", "read_file", "{}"),
    ]);
    assert_eq!(messages.len(), 4);
    assert!(messages[0].reasoning_content.is_none());
    assert_eq!(call_ids(&messages[3]), ["new_a", "new_b"]);
    assert_eq!(
        messages[3].reasoning_content.as_deref(),
        Some("new reasoning")
    );
}

#[test]
fn parallel_batch_roundtrip_preserves_call_output_types_and_order() {
    let request = responses_request_to_llm(json!({
        "model": "fixture-model",
        "input": [
            function_call("call_a", "read_file", r#"{"path":"a"}"#),
            custom_call("call_patch"),
            {"type": "custom_tool_call_output", "call_id": "call_patch", "output": "patched"},
            output("call_a")
        ]
    }));
    let restored = llm_request_to_responses(request);
    let input = restored["input"].as_array().unwrap();
    assert_eq!(input.len(), 4);
    assert_eq!(
        input
            .iter()
            .map(|item| item["call_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["call_a", "call_patch", "call_patch", "call_a"]
    );
    assert_eq!(input[0]["arguments"], r#"{"path":"a"}"#);
    assert_eq!(input[1]["type"], "custom_tool_call");
    assert_eq!(input[2]["type"], "custom_tool_call_output");
    assert_eq!(input[3]["type"], "function_call_output");
}

#[test]
fn parallel_batch_roundtrip_keeps_raw_fragment_between_batches() {
    let raw = json!({"type": "web_search_call", "id": "search_fixture", "status": "completed"});
    let request = responses_request_to_llm(json!({
        "model": "fixture-model",
        "input": [
            function_call("call_a", "read_file", "{}"),
            function_call("call_b", "read_file", "{}"),
            output("call_a"), output("call_b"),
            raw.clone(),
            function_call("call_c", "read_file", "{}"),
            function_call("call_d", "read_file", "{}")
        ]
    }));
    let restored = llm_request_to_responses(request);
    assert_eq!(restored["input"].as_array().unwrap().len(), 7);
    assert_eq!(restored["input"][4], raw);
    assert_eq!(restored["input"][5]["call_id"], "call_c");
    assert_eq!(restored["input"][6]["call_id"], "call_d");
}

#[test]
fn tool_batch_state_does_not_escape_a_request() {
    let first = messages(vec![
        function_call("call_a", "read_file", "{}"),
        function_call("call_b", "read_file", "{}"),
    ]);
    let second = messages(vec![function_call("call_c", "read_file", "{}")]);
    assert_eq!(call_ids(&first[0]), ["call_a", "call_b"]);
    assert_eq!(second.len(), 1);
    assert_eq!(call_ids(&second[0]), ["call_c"]);
    assert_eq!(second[0].tool_calls[0].index, 0);
}

#[test]
fn merged_assistant_items_keep_annotations_refusals_and_reasoning_context() {
    let mut thought = reasoning("inspect");
    thought["encrypted_content"] = json!("fixture-signature");
    thought["context"] = json!({"turn": "current"});
    let request = responses_request_to_llm(json!({"input": [
        {"role": "assistant", "id": "msg_commentary", "content": "before"},
        function_call("call_a", "read_file", "{}"), thought,
        {"role": "assistant", "content": [{"type": "output_text", "text": "between", "annotations": [{"type": "url_citation", "url": "https://example.com/reference"}]}]},
        function_call("call_b", "read_file", "{}"),
        {"role": "assistant", "content": [{"type": "refusal", "refusal": "first refusal"}]},
        {"role": "assistant", "content": [{"type": "refusal", "refusal": "second refusal"}]},
        output("call_a"), output("call_b")
    ]}));
    assert_eq!(request.messages.len(), 3);
    let message = &request.messages[0];
    assert_eq!(message.id, "msg_commentary");
    assert_eq!(call_ids(message), ["call_a", "call_b"]);
    assert_eq!(message.reasoning_content.as_deref(), Some("inspect"));
    assert_eq!(message.refusal, "first refusal\nsecond refusal");
    assert_eq!(message.annotations.len(), 1);
    let restored = llm_request_to_responses(request);
    assert_eq!(
        restored["input"][0]["encrypted_content"],
        "fixture-signature"
    );
    assert_eq!(restored["input"][0]["context"], json!({"turn": "current"}));
    assert!(restored.to_string().contains("before"));
    assert!(restored.to_string().contains("between"));
}

#[test]
fn raw_fragments_stay_between_turns_after_assistant_items_are_merged() {
    let raw = json!({"type": "web_search_call", "id": "search_fixture", "status": "completed"});
    let request = responses_request_to_llm(json!({"input": [
        function_call("call_a", "read_file", "{}"),
        {"role": "assistant", "content": "first commentary"},
        {"role": "assistant", "content": "second commentary"},
        output("call_a"), raw.clone(),
        function_call("call_b", "read_file", "{}"), output("call_b")
    ]}));
    let restored = llm_request_to_responses(request);
    let input = restored["input"].as_array().unwrap();
    let raw_index = input.iter().position(|item| *item == raw).unwrap();
    assert_eq!(
        input[raw_index - 1]["type"],
        "function_call_output",
        "{restored}"
    );
    assert_eq!(input[raw_index - 1]["call_id"], "call_a");
    assert_eq!(input[raw_index + 1]["type"], "function_call", "{restored}");
    assert_eq!(input[raw_index + 1]["call_id"], "call_b");
}

#[test]
fn raw_fragments_keep_head_tail_and_adjacent_order_after_reasoning_merge() {
    let raw = |id: &str| json!({"type": "future_raw_item", "id": id});
    let request = responses_request_to_llm(json!({"instructions": "system", "input": [
        raw("head_a"), raw("head_b"), reasoning("first"), reasoning("second"),
        function_call("call_a", "read_file", "{}"), output("call_a"), raw("tail_a"), raw("tail_b")
    ]}));
    let restored = llm_request_to_responses(request);
    let input = restored["input"].as_array().unwrap();
    assert_eq!(input[0], raw("head_a"));
    assert_eq!(input[1], raw("head_b"));
    assert_eq!(input[input.len() - 2], raw("tail_a"));
    assert_eq!(input[input.len() - 1], raw("tail_b"));
    assert_eq!(
        input
            .iter()
            .filter(|item| item["type"] == "reasoning")
            .count(),
        1
    );
}

#[test]
fn raw_boundary_does_not_attach_prior_commentary_to_a_later_tool_call() {
    let messages = messages(vec![
        json!({"role": "assistant", "content": "earlier commentary"}),
        json!({"type": "web_search_call", "id": "search_fixture"}),
        function_call("call_a", "read_file", "{}"),
        output("call_a"),
    ]);
    assert_eq!(messages.len(), 3);
    assert!(messages[0].tool_calls.is_empty());
    assert_eq!(call_ids(&messages[1]), ["call_a"]);
}

#[test]
fn top_level_assistant_text_is_not_duplicated_as_a_raw_fragment() {
    let request = responses_request_to_llm(json!({"input": [
        function_call("call_a", "read_file", "{}"),
        {"type": "output_text", "text": "single commentary"},
        output("call_a")
    ]}));
    let restored = llm_request_to_responses(request);
    assert_eq!(restored.to_string().matches("single commentary").count(), 1);
    assert_eq!(restored["input"].as_array().unwrap().len(), 3);
}
