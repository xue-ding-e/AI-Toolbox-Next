use super::support::*;
use ai_toolbox_lib::coding::proxy_gateway::transformer::{
    convert_request_body, convert_request_body_with_context, convert_request_value,
    convert_response_body_with_context, convert_response_value,
    convert_responses_compact_request_body_to_target, convert_sse_stream, AiProtocol,
    ConversionRoute,
};
use futures_util::{stream, StreamExt};
use serde_json::{json, Value};

fn chat_route() -> ConversionRoute {
    ConversionRoute::new(AiProtocol::OpenAiResponses, AiProtocol::OpenAiChat)
}

fn to_chat(request: Value) -> Value {
    convert_request_value(chat_route(), request).expect("convert Responses request")
}

fn batch_ids(body: &Value) -> Vec<Vec<&str>> {
    body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|message| message["tool_calls"].as_array())
        .map(|calls| {
            calls
                .iter()
                .map(|call| call["id"].as_str().unwrap())
                .collect()
        })
        .collect()
}

fn followup_input(response: &Value) -> Vec<Value> {
    let mut input = response["output"].as_array().unwrap().clone();
    input.extend([
        tool_output("tool_a", json!("first result")),
        tool_output("tool_b", json!("second result")),
    ]);
    input
}

#[test]
fn parallel_same_name_calls_keep_every_id_argument_and_result() {
    let body = to_chat(responses_request(parallel_input()));
    assert_chat_tool_history(&body);
    assert_eq!(batch_ids(&body), [["tool_a", "tool_b"]]);
    assert_eq!(body["messages"].as_array().unwrap().len(), 3);
    let calls = &body["messages"][0]["tool_calls"];
    assert_eq!(calls[0]["function"]["name"], "read_file");
    assert_eq!(calls[1]["function"]["name"], "read_file");
    assert_eq!(
        calls[0]["function"]["arguments"],
        "{\"path\":\"first.txt\"}"
    );
    assert_eq!(
        calls[1]["function"]["arguments"],
        "{\"path\":\"second.txt\"}"
    );
    assert_eq!(body["messages"][1]["content"], "first result");
    assert_eq!(body["messages"][2]["content"], "second result");
}

#[test]
fn parallel_three_calls_keep_native_and_colon_ids_without_renumbering() {
    let ids = ["exec_command:14", "tool_native", "mcp__read_thread:0"];
    let mut input = ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            function_call(
                id,
                "exec_command",
                json!({"cmd": format!("command {index}")}),
            )
        })
        .collect::<Vec<_>>();
    input.extend(
        ids.iter()
            .map(|id| tool_output(id, json!(format!("output {id}")))),
    );
    let body = to_chat(responses_request(input));
    assert_chat_tool_history(&body);
    assert_eq!(batch_ids(&body), [ids]);
    assert_eq!(body["messages"].as_array().unwrap().len(), 4);
}

#[test]
fn parallel_results_preserve_all_completion_order_permutations() {
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let ids = ["call_a", "call_b", "call_c"];
        let mut input = ids
            .iter()
            .map(|id| function_call(id, "read_file", json!({})))
            .collect::<Vec<_>>();
        input.extend(
            order
                .iter()
                .map(|index| tool_output(ids[*index], json!(index))),
        );
        let body = to_chat(responses_request(input));
        assert_chat_tool_history(&body);
        assert_eq!(batch_ids(&body), [ids]);
        let result_ids = body["messages"].as_array().unwrap()[1..]
            .iter()
            .map(|message| message["tool_call_id"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(result_ids, order.map(|index| ids[index]));
    }
}

#[test]
fn serial_and_parallel_batches_remain_separate_across_multiple_turns() {
    let mut input = vec![
        function_call("serial", "read_file", json!({})),
        tool_output("serial", json!("serial result")),
    ];
    input.extend(parallel_input());
    input.extend([
        json!({"role": "assistant", "content": "Both files were read."}),
        json!({"role": "user", "content": "Read another file."}),
        function_call("next_turn", "read_file", json!({})),
        tool_output("next_turn", json!("next result")),
    ]);
    let body = to_chat(responses_request(input));
    assert_chat_tool_history(&body);
    assert_eq!(
        batch_ids(&body),
        [vec!["serial"], vec!["tool_a", "tool_b"], vec!["next_turn"]]
    );
}

#[test]
fn disabling_parallel_generation_does_not_destroy_existing_parallel_history() {
    let mut request = responses_request(parallel_input());
    request["parallel_tool_calls"] = json!(false);
    let body = to_chat(request);
    assert_chat_tool_history(&body);
    assert_eq!(batch_ids(&body), [["tool_a", "tool_b"]]);
    assert_eq!(body["parallel_tool_calls"], false);
}

#[test]
fn historical_parallel_calls_survive_without_current_tool_definitions() {
    let mut request = responses_request(parallel_input());
    request.as_object_mut().unwrap().remove("tools");
    let body = to_chat(request);
    assert_chat_tool_history(&body);
    assert_eq!(batch_ids(&body), [["tool_a", "tool_b"]]);
}

#[test]
fn reasoning_and_commentary_before_parallel_calls_keep_their_text() {
    let mut input = vec![
        json!({"type": "reasoning", "summary": [{"type": "summary_text", "text": "Inspect both."}]}),
        json!({"type": "message", "role": "assistant", "content": [
            {"type": "output_text", "text": "Reading both files now."}
        ]}),
    ];
    input.extend(parallel_input());
    let body = to_chat(responses_request(input));
    assert_chat_tool_history(&body);
    assert_eq!(batch_ids(&body), [["tool_a", "tool_b"]]);
    assert!(body["messages"][0]["content"]
        .to_string()
        .contains("Reading both files now."));
    assert_eq!(body["messages"][0]["reasoning_content"], "Inspect both.");
}

#[test]
fn parallel_namespace_calls_with_the_same_child_name_keep_distinct_names() {
    let mut request = responses_request(parallel_input());
    request["tools"] =
        json!(["left", "right"].into_iter().map(|namespace| json!({
        "type": "namespace", "name": namespace,
        "tools": [{"type": "function", "name": "read_file", "parameters": {"type": "object"}}]
    })).collect::<Vec<_>>());
    request["input"][0]["namespace"] = json!("left");
    request["input"][1]["namespace"] = json!("right");
    let body = to_chat(request);
    assert_chat_tool_history(&body);
    assert_eq!(batch_ids(&body), [["tool_a", "tool_b"]]);
    assert_eq!(
        body["messages"][0]["tool_calls"][0]["function"]["name"],
        "left__read_file"
    );
    assert_eq!(
        body["messages"][0]["tool_calls"][1]["function"]["name"],
        "right__read_file"
    );
}

#[test]
fn parallel_tool_search_and_function_calls_keep_their_result_pairing() {
    let mut request = responses_request(vec![
        json!({
            "type": "tool_search_call", "call_id": "tool_search_a",
            "execution": "client", "arguments": {"query": "read files"}
        }),
        function_call("tool_b", "read_file", json!({"path": "second.txt"})),
        json!({
            "type": "tool_search_output", "call_id": "tool_search_a",
            "execution": "client", "tools": []
        }),
        tool_output("tool_b", json!("second result")),
    ]);
    request["tools"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type": "tool_search"}));
    let body = to_chat(request);
    assert_chat_tool_history(&body);
    assert_eq!(batch_ids(&body), [["tool_search_a", "tool_b"]]);
    assert_eq!(
        body["messages"][0]["tool_calls"][0]["function"]["name"],
        "tool_search"
    );
    assert_eq!(body["messages"][1]["tool_call_id"], "tool_search_a");
}

#[test]
fn parallel_custom_and_namespace_tools_preserve_request_response_context() {
    let patch = "*** Begin Patch\n*** Add File: test.txt\n+hello\n*** End Patch";
    let tools = json!([
        {"type": "namespace", "name": "fs", "tools": [
            {"type": "function", "name": "read_file", "parameters": {"type": "object"}}
        ]},
        {"type": "custom", "name": "apply_patch"}
    ]);
    let mut request = responses_request(vec![
        function_call("tool_a", "read_file", json!({"path": "first.txt"})),
        json!({"type": "custom_tool_call", "call_id": "tool_b", "name": "apply_patch", "input": patch}),
        tool_output("tool_a", json!("first result")),
        json!({"type": "custom_tool_call_output", "call_id": "tool_b", "output": "patched"}),
    ]);
    request["input"][0]["namespace"] = json!("fs");
    request["tools"] = tools.clone();
    let prepared =
        convert_request_body_with_context(chat_route(), &serde_json::to_vec(&request).unwrap())
            .unwrap();
    let body: Value = serde_json::from_slice(&prepared.body).unwrap();
    assert_chat_tool_history(&body);
    assert_eq!(batch_ids(&body), [["tool_a", "tool_b"]]);
    let patch_arguments: Value = serde_json::from_str(
        body["messages"][0]["tool_calls"][1]["function"]["arguments"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(patch_arguments["input"], patch);

    let mut response = chat_tool_response();
    response["choices"][0]["message"]["tool_calls"][0]["function"]["name"] = json!("fs__read_file");
    response["choices"][0]["message"]["tool_calls"][1]["function"] = json!({
        "name": "apply_patch", "arguments": json!({"input": patch}).to_string()
    });
    let restored: Value = serde_json::from_slice(
        &convert_response_body_with_context(
            chat_route().reverse(),
            &serde_json::to_vec(&response).unwrap(),
            Some(&prepared.context),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(restored["output"][0]["namespace"], "fs");
    assert_eq!(restored["output"][0]["name"], "read_file");
    assert_eq!(restored["output"][1]["type"], "custom_tool_call");
    assert_eq!(restored["output"][1]["call_id"], "tool_b");
    assert_eq!(restored["output"][1]["input"], patch);
    let mut next = responses_request(followup_input(&restored));
    next["input"][3]["type"] = json!("custom_tool_call_output");
    next["tools"] = tools;
    assert_chat_tool_history(&to_chat(next));
}

#[test]
fn parallel_image_results_are_followed_by_one_synthetic_user_message() {
    let mut input = parallel_input();
    input[2]["output"] =
        json!([{"type": "input_image", "image_url": "data:image/png;base64,FIRST_IMAGE"}]);
    input[3]["output"] = json!({"type": "image", "mimeType": "image/png", "data": "SECOND_IMAGE"});
    let body = to_chat(responses_request(input));
    assert_chat_tool_history(&body);
    assert_eq!(batch_ids(&body), [["tool_a", "tool_b"]]);
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[1]["role"], "tool");
    assert_eq!(messages[2]["role"], "tool");
    assert_eq!(messages[3]["role"], "user");
    let images = messages[3]["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|part| part["type"] == "image_url")
        .collect::<Vec<_>>();
    assert_eq!(images.len(), 2);
    assert_eq!(
        images[0]["image_url"]["url"],
        "data:image/png;base64,FIRST_IMAGE"
    );
    assert_eq!(
        images[1]["image_url"]["url"],
        "data:image/png;base64,SECOND_IMAGE"
    );
}

#[test]
fn parallel_results_preserve_structured_json_and_empty_text() {
    let mut input = parallel_input();
    input[2]["output"] = json!({"records": [1, 2], "ok": true});
    input[3]["output"] = json!("");
    let body = to_chat(responses_request(input));
    assert_chat_tool_history(&body);
    let result: Value =
        serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(result, json!({"records": [1, 2], "ok": true}));
    assert_eq!(body["messages"][2]["content"], "");
}

#[test]
fn batching_does_not_invent_a_missing_tool_result() {
    let mut input = parallel_input();
    input.pop();
    let body = to_chat(responses_request(input));
    assert_eq!(batch_ids(&body), [["tool_a", "tool_b"]]);
    assert_eq!(body["messages"].as_array().unwrap().len(), 2);
    assert!(validate_chat_tool_history(&body)
        .unwrap_err()
        .contains("tool_b"));
}

#[test]
fn responses_identity_keeps_original_parallel_input_bytes() {
    let mut request = responses_request(parallel_input());
    request["input"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type": "future_raw_item", "value": 7}));
    let bytes = serde_json::to_vec_pretty(&request).unwrap();
    let result = convert_request_body(
        ConversionRoute::new(AiProtocol::OpenAiResponses, AiProtocol::OpenAiResponses),
        &bytes,
    )
    .unwrap();
    assert_eq!(result, bytes);
}

#[test]
fn compact_chat_fallback_keeps_the_complete_parallel_tool_history() {
    let prepared = convert_responses_compact_request_body_to_target(
        AiProtocol::OpenAiChat,
        &serde_json::to_vec(&responses_request(parallel_input())).unwrap(),
    )
    .unwrap();
    let body: Value = serde_json::from_slice(&prepared.body).unwrap();
    assert_chat_tool_history(&body);
    assert_eq!(batch_ids(&body), [["tool_a", "tool_b"]]);
    assert_eq!(body["stream"], false);
}

#[test]
fn parallel_responses_to_anthropic_keep_one_tool_use_batch_before_results() {
    let body = convert_request_value(
        ConversionRoute::new(AiProtocol::OpenAiResponses, AiProtocol::AnthropicMessages),
        responses_request(parallel_input()),
    )
    .unwrap();
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "assistant");
    let calls = messages[0]["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|block| block["type"] == "tool_use")
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["id"], "tool_a");
    assert_eq!(calls[1]["id"], "tool_b");
    assert_eq!(messages[1]["role"], "user");
    assert_eq!(messages[1]["content"][0]["tool_use_id"], "tool_a");
    assert_eq!(messages[1]["content"][1]["tool_use_id"], "tool_b");
}

#[test]
fn parallel_responses_to_gemini_keep_every_call_and_result_name() {
    let body = convert_request_value(
        ConversionRoute::new(AiProtocol::OpenAiResponses, AiProtocol::GeminiNative),
        responses_request(parallel_input()),
    )
    .unwrap();
    let contents = body["contents"].as_array().unwrap();
    assert_eq!(contents[0]["role"], "model");
    let calls = contents[0]["parts"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|part| part.get("functionCall"))
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["id"], "tool_a");
    assert_eq!(calls[1]["id"], "tool_b");
    let outputs = contents[1..]
        .iter()
        .flat_map(|content| content["parts"].as_array().unwrap())
        .filter_map(|part| part.get("functionResponse"))
        .collect::<Vec<_>>();
    assert_eq!(outputs.len(), 2);
    assert_eq!(outputs[0]["id"], "tool_a");
    assert_eq!(outputs[1]["id"], "tool_b");
    assert!(outputs.iter().all(|output| output["name"] == "read_file"));
}

#[test]
fn parallel_chat_json_response_can_be_replayed_as_a_complete_next_request() {
    let response = convert_response_value(chat_route().reverse(), chat_tool_response()).unwrap();
    let body = to_chat(responses_request(followup_input(&response)));
    assert_chat_tool_history(&body);
    assert_eq!(batch_ids(&body), [["tool_a", "tool_b"]]);
    assert_eq!(response["usage"]["input_tokens"], 10);
    assert_eq!(response["usage"]["output_tokens"], 5);
}

async fn converted_tool_stream(delayed_ids: bool, chunk_size: usize) -> Vec<Value> {
    let chunks = chat_tool_stream(delayed_ids)
        .chunks(chunk_size)
        .map(|chunk| Ok(chunk.to_vec()))
        .collect::<Vec<Result<Vec<u8>, String>>>();
    let mut converted = convert_sse_stream(chat_route().reverse(), Box::pin(stream::iter(chunks)));
    let mut bytes = Vec::new();
    while let Some(chunk) = converted.next().await {
        bytes.extend(chunk.unwrap());
    }
    sse_events(&bytes)
}

#[tokio::test]
async fn parallel_sse_history_survives_network_chunk_boundaries() {
    for chunk_size in [1, 7, 37, usize::MAX] {
        let response = completed_response(&converted_tool_stream(false, chunk_size).await);
        let body = to_chat(responses_request(followup_input(&response)));
        assert_chat_tool_history(&body);
        assert_eq!(batch_ids(&body), [["tool_a", "tool_b"]]);
        assert_eq!(
            body["messages"][0]["tool_calls"][0]["function"]["arguments"],
            "{\"path\":\"first.txt\"}"
        );
        assert_eq!(
            body["messages"][0]["tool_calls"][1]["function"]["arguments"],
            "{\"path\":\"second.txt\"}"
        );
        assert_eq!(response["usage"]["input_tokens"], 10);
        assert_eq!(response["usage"]["output_tokens"], 5);
    }
}

#[tokio::test]
async fn parallel_sse_delayed_ids_and_interleaved_arguments_replay_without_synthetic_ids() {
    let response = completed_response(&converted_tool_stream(true, 11).await);
    let body = to_chat(responses_request(followup_input(&response)));
    assert_chat_tool_history(&body);
    assert_eq!(batch_ids(&body), [["tool_a", "tool_b"]]);
}

#[tokio::test]
async fn parallel_sse_output_item_done_history_replays_without_terminal_snapshot() {
    let events = converted_tool_stream(false, 17).await;
    let mut input = events
        .iter()
        .filter(|event| event["type"] == "response.output_item.done")
        .map(|event| event["item"].clone())
        .collect::<Vec<_>>();
    input.extend([
        tool_output("tool_a", json!("a")),
        tool_output("tool_b", json!("b")),
    ]);
    let body = to_chat(responses_request(input));
    assert_chat_tool_history(&body);
    assert_eq!(batch_ids(&body), [["tool_a", "tool_b"]]);
}
