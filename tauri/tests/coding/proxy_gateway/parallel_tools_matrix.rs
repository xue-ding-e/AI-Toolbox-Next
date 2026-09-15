use super::support::*;
use ai_toolbox_lib::coding::proxy_gateway::transformer::{
    convert_request_body, convert_request_value, convert_response_value, convert_sse_stream,
    AiProtocol, ConversionRoute,
};
use futures_util::{stream, StreamExt};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const PROTOCOLS: [AiProtocol; 4] = [
    AiProtocol::OpenAiChat,
    AiProtocol::OpenAiResponses,
    AiProtocol::AnthropicMessages,
    AiProtocol::GeminiNative,
];

fn gemini_history_with_ids() -> Value {
    let mut request = gemini_history();
    for (index, id) in ["tool_a", "tool_b"].iter().enumerate() {
        request["contents"][0]["parts"][index]["functionCall"]["id"] = json!(id);
        request["contents"][1]["parts"][index]["functionResponse"]["id"] = json!(id);
    }
    request
}

fn protocol_history(protocol: AiProtocol) -> Value {
    match protocol {
        AiProtocol::OpenAiChat => chat_history(),
        AiProtocol::OpenAiResponses => responses_request(parallel_input()),
        AiProtocol::AnthropicMessages => anthropic_history(),
        AiProtocol::GeminiNative => gemini_history_with_ids(),
    }
}

fn protocol_reply(protocol: AiProtocol) -> Value {
    match protocol {
        AiProtocol::OpenAiChat => {
            let mut reply = chat_tool_response();
            reply["choices"][0]["message"]["content"] = json!("Reading both files now.");
            reply
        }
        AiProtocol::OpenAiResponses => json!({
            "id": "resp_fixture", "object": "response", "status": "completed", "model": "fixture-model",
            "output": [
                function_call("tool_a", "read_file", json!({"path": "first.txt"})),
                function_call("tool_b", "read_file", json!({"path": "second.txt"})),
                {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "Reading both files now."}]}
            ],
            "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15}
        }),
        AiProtocol::AnthropicMessages => {
            let mut content = anthropic_history()["messages"][0]["content"].clone();
            content
                .as_array_mut()
                .unwrap()
                .push(json!({"type": "text", "text": "Reading both files now."}));
            json!({"id": "msg_fixture", "type": "message", "role": "assistant", "model": "fixture-model",
                "content": content, "stop_reason": "tool_use", "usage": {"input_tokens": 10, "output_tokens": 5}})
        }
        AiProtocol::GeminiNative => {
            let mut content = gemini_history_with_ids()["contents"][0].clone();
            content["parts"]
                .as_array_mut()
                .unwrap()
                .push(json!({"text": "Reading both files now."}));
            json!({"responseId": "gemini_fixture", "modelVersion": "gemini-2.5-pro",
                "candidates": [{"index": 0, "content": content, "finishReason": "STOP"}],
                "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 5, "totalTokenCount": 15}})
        }
    }
}

fn followup_for_reply(protocol: AiProtocol, reply: &Value) -> Value {
    let mut request = protocol_history(protocol);
    match protocol {
        AiProtocol::OpenAiChat => request["messages"][0] = reply["choices"][0]["message"].clone(),
        AiProtocol::OpenAiResponses => {
            let mut input = reply["output"].as_array().unwrap().clone();
            input.extend([
                tool_output("tool_a", json!("first result")),
                tool_output("tool_b", json!("second result")),
            ]);
            request["input"] = json!(input);
        }
        AiProtocol::AnthropicMessages => {
            request["messages"][0]["content"] = reply["content"].clone()
        }
        AiProtocol::GeminiNative => {
            request["contents"][0] = reply["candidates"][0]["content"].clone()
        }
    }
    request
}

fn gemini_history() -> Value {
    json!({
        "model": "gemini-2.5-pro",
        "contents": [
            {"role": "model", "parts": [
                {"functionCall": {"name": "read_file", "args": {"path": "first.txt"}}},
                {"functionCall": {"name": "read_file", "args": {"path": "second.txt"}}}
            ]},
            {"role": "user", "parts": [
                {"functionResponse": {"name": "read_file", "response": {"result": "first result"}}},
                {"functionResponse": {"name": "read_file", "response": {"result": "second result"}}}
            ]}
        ]
    })
}

fn chat_history() -> Value {
    json!({"model": "fixture-model", "messages": [
        chat_tool_response()["choices"][0]["message"].clone(),
        {"role": "tool", "tool_call_id": "tool_a", "content": "first result"},
        {"role": "tool", "tool_call_id": "tool_b", "content": "second result"}
    ]})
}

fn anthropic_history() -> Value {
    json!({"model": "fixture-model", "max_tokens": 128, "messages": [
        {"role": "assistant", "content": [
            {"type": "tool_use", "id": "tool_a", "name": "read_file", "input": {"path": "first.txt"}},
            {"type": "tool_use", "id": "tool_b", "name": "read_file", "input": {"path": "second.txt"}}
        ]},
        {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": "tool_a", "content": "first result"},
            {"type": "tool_result", "tool_use_id": "tool_b", "content": "second result"}
        ]}
    ]})
}

/// Inspect the actual target wire shape, without converting it through another
/// production transformer that could hide an invalid turn boundary.
fn single_batch_ids(protocol: AiProtocol, body: &Value) -> (Vec<String>, Vec<String>) {
    let (calls, results, call_id_key, result_id_key) = match protocol {
        AiProtocol::OpenAiChat => {
            assert_chat_tool_history(body);
            let messages = body["messages"].as_array().unwrap();
            assert_eq!(messages.len(), 3, "{body}");
            (
                messages[0]["tool_calls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .collect::<Vec<_>>(),
                messages[1..].iter().collect::<Vec<_>>(),
                "id",
                "tool_call_id",
            )
        }
        AiProtocol::AnthropicMessages => {
            let messages = body["messages"].as_array().unwrap();
            assert_eq!(messages.len(), 2, "{body}");
            assert_eq!(messages[0]["role"], "assistant");
            assert_eq!(messages[1]["role"], "user");
            (
                messages[0]["content"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|part| part["type"] == "tool_use")
                    .collect(),
                messages[1]["content"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|part| part["type"] == "tool_result")
                    .collect(),
                "id",
                "tool_use_id",
            )
        }
        AiProtocol::GeminiNative => {
            let contents = body["contents"].as_array().unwrap();
            assert_eq!(
                contents.len(),
                2,
                "parallel results must share one user turn: {body}"
            );
            assert_eq!(contents[0]["role"], "model");
            assert_eq!(contents[1]["role"], "user");
            (
                contents[0]["parts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|part| part.get("functionCall"))
                    .collect(),
                contents[1]["parts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|part| part.get("functionResponse"))
                    .collect(),
                "id",
                "id",
            )
        }
        AiProtocol::OpenAiResponses => {
            let input = body["input"].as_array().unwrap();
            let first_result = input
                .iter()
                .position(|item| item["type"] == "function_call_output")
                .unwrap();
            assert!(
                input[first_result..]
                    .iter()
                    .all(|item| item["type"] == "function_call_output"),
                "{body}"
            );
            (
                input[..first_result]
                    .iter()
                    .filter(|item| item["type"] == "function_call")
                    .collect(),
                input[first_result..].iter().collect(),
                "call_id",
                "call_id",
            )
        }
    };
    assert_eq!(calls.len(), 2, "{body}");
    assert_eq!(results.len(), 2, "{body}");
    (
        calls
            .iter()
            .map(|call| call[call_id_key].as_str().unwrap().to_string())
            .collect(),
        results
            .iter()
            .map(|result| result[result_id_key].as_str().unwrap().to_string())
            .collect(),
    )
}

#[test]
fn parallel_results_share_one_gemini_user_turn_from_every_source() {
    for (source, request) in [
        (
            AiProtocol::OpenAiResponses,
            responses_request(parallel_input()),
        ),
        (AiProtocol::OpenAiChat, chat_history()),
        (AiProtocol::AnthropicMessages, anthropic_history()),
    ] {
        let body = convert_request_value(
            ConversionRoute::new(source, AiProtocol::GeminiNative),
            request,
        )
        .unwrap();
        let (calls, results) = single_batch_ids(AiProtocol::GeminiNative, &body);
        assert_eq!(calls, ["tool_a", "tool_b"]);
        assert_eq!(results, calls);
    }
}

#[test]
fn gemini_same_name_calls_without_ids_pair_each_result_once_for_every_target() {
    for target in [
        AiProtocol::OpenAiChat,
        AiProtocol::OpenAiResponses,
        AiProtocol::AnthropicMessages,
    ] {
        let body = convert_request_value(
            ConversionRoute::new(AiProtocol::GeminiNative, target),
            gemini_history(),
        )
        .unwrap();
        let (calls, results) = single_batch_ids(target, &body);
        assert_ne!(calls[0], calls[1]);
        assert_eq!(results, calls, "{target:?}: {body}");
    }
}

#[test]
fn gemini_missing_call_ids_do_not_collide_across_model_turns() {
    let mut request = gemini_history();
    let second_turn = request["contents"].as_array().unwrap().clone();
    request["contents"]
        .as_array_mut()
        .unwrap()
        .extend(second_turn);
    let body = convert_request_value(
        ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::OpenAiChat),
        request,
    )
    .unwrap();
    let ids = body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|message| message["tool_calls"].as_array())
        .flatten()
        .map(|call| call["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 4);
    assert_eq!(
        ids.iter().copied().collect::<BTreeSet<_>>().len(),
        4,
        "{body}"
    );
    assert_chat_tool_history(&body);
}

#[test]
fn gemini_roundtrip_does_not_send_synthetic_result_ids_upstream() {
    let chat = convert_request_value(
        ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::OpenAiChat),
        gemini_history(),
    )
    .unwrap();
    let body = convert_request_value(
        ConversionRoute::new(AiProtocol::OpenAiChat, AiProtocol::GeminiNative),
        chat,
    )
    .unwrap();
    for part in body["contents"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|content| content["parts"].as_array().unwrap())
    {
        for key in ["functionCall", "functionResponse"] {
            if let Some(function) = part.get(key) {
                assert!(
                    function.get("id").is_none(),
                    "synthetic IDs are local pairing handles: {body}"
                );
            }
        }
    }
}

async fn responses_stream_with_late_text() -> Vec<Value> {
    let mut chunks = sse_events(&chat_tool_stream(false));
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
    let wire = chunks
        .iter()
        .map(|chunk| format!("data: {chunk}\n\n"))
        .collect::<String>()
        + "data: [DONE]\n\n";
    let source = stream::iter(
        wire.as_bytes()
            .chunks(7)
            .map(|chunk| Ok(chunk.to_vec()))
            .collect::<Vec<Result<Vec<u8>, String>>>(),
    );
    let mut converted = convert_sse_stream(
        ConversionRoute::new(AiProtocol::OpenAiChat, AiProtocol::OpenAiResponses),
        Box::pin(source),
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = converted.next().await {
        bytes.extend(chunk.unwrap());
    }
    sse_events(&bytes)
}

#[tokio::test]
async fn parallel_sse_text_after_calls_replays_as_one_assistant_turn() {
    let response = completed_response(&responses_stream_with_late_text().await);
    let mut input = response["output"].as_array().unwrap().clone();
    assert_eq!(
        input.last().unwrap()["type"],
        "message",
        "fixture must exercise text after calls"
    );
    input.extend([
        tool_output("tool_a", json!("first result")),
        tool_output("tool_b", json!("second result")),
    ]);
    for target in [
        AiProtocol::OpenAiChat,
        AiProtocol::AnthropicMessages,
        AiProtocol::GeminiNative,
    ] {
        let body = convert_request_value(
            ConversionRoute::new(AiProtocol::OpenAiResponses, target),
            responses_request(input.clone()),
        )
        .unwrap();
        let (calls, results) = single_batch_ids(target, &body);
        assert_eq!(calls, ["tool_a", "tool_b"]);
        assert_eq!(results, calls);
        assert_eq!(
            body.to_string().matches("Reading both files now.").count(),
            1,
            "{body}"
        );
    }
}

#[test]
fn all_twelve_request_routes_preserve_parallel_call_result_batches() {
    let mut routes = 0;
    for source in PROTOCOLS {
        for target in PROTOCOLS.into_iter().filter(|target| *target != source) {
            let body = convert_request_value(
                ConversionRoute::new(source, target),
                protocol_history(source),
            )
            .unwrap();
            let (calls, results) = single_batch_ids(target, &body);
            assert_eq!(
                calls,
                ["tool_a", "tool_b"],
                "{source:?} -> {target:?}: {body}"
            );
            assert_eq!(results, calls);
            assert_eq!(body.to_string().matches("first result").count(), 1);
            assert_eq!(body.to_string().matches("second result").count(), 1);
            routes += 1;
        }
    }
    assert_eq!(routes, 12);
}

#[test]
fn all_twelve_json_response_routes_replay_parallel_calls_and_commentary() {
    let mut routes = 0;
    for upstream in PROTOCOLS {
        for client in PROTOCOLS.into_iter().filter(|client| *client != upstream) {
            let route = ConversionRoute::new(upstream, client);
            let reply = convert_response_value(route, protocol_reply(upstream)).unwrap();
            let followup = followup_for_reply(client, &reply);
            let body = convert_request_value(route.reverse(), followup).unwrap();
            let (calls, results) = single_batch_ids(upstream, &body);
            assert_eq!(
                calls,
                ["tool_a", "tool_b"],
                "{upstream:?} -> {client:?} -> {upstream:?}: {body}"
            );
            assert_eq!(results, calls);
            assert_eq!(
                body.to_string().matches("Reading both files now.").count(),
                1,
                "{body}"
            );
            routes += 1;
        }
    }
    assert_eq!(routes, 12);
}

#[test]
fn responses_commentary_before_between_and_after_calls_keeps_one_assistant_turn() {
    let input = vec![
        json!({"role": "assistant", "content": "before"}),
        function_call("tool_a", "read_file", json!({"path": "first.txt"})),
        json!({"type": "reasoning", "summary": [{"type": "summary_text", "text": "inspect both"}]}),
        json!({"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "between"}]}),
        function_call("tool_b", "read_file", json!({"path": "second.txt"})),
        json!({"type": "output_text", "text": "after"}),
        tool_output("tool_a", json!("first result")),
        tool_output("tool_b", json!("second result")),
    ];
    for target in [
        AiProtocol::OpenAiChat,
        AiProtocol::AnthropicMessages,
        AiProtocol::GeminiNative,
    ] {
        let body = convert_request_value(
            ConversionRoute::new(AiProtocol::OpenAiResponses, target),
            responses_request(input.clone()),
        )
        .unwrap();
        let (calls, results) = single_batch_ids(target, &body);
        assert_eq!(calls, ["tool_a", "tool_b"]);
        assert_eq!(results, calls);
        for text in ["before", "between", "after", "inspect both"] {
            assert_eq!(body.to_string().matches(text).count(), 1, "{body}");
        }
    }
}

#[test]
fn gemini_explicit_result_ids_preserve_reverse_completion_order() {
    let mut request = gemini_history_with_ids();
    request["contents"][1]["parts"]
        .as_array_mut()
        .unwrap()
        .reverse();
    for target in [
        AiProtocol::OpenAiChat,
        AiProtocol::OpenAiResponses,
        AiProtocol::AnthropicMessages,
    ] {
        let body = convert_request_value(
            ConversionRoute::new(AiProtocol::GeminiNative, target),
            request.clone(),
        )
        .unwrap();
        let (calls, results) = single_batch_ids(target, &body);
        assert_eq!(calls, ["tool_a", "tool_b"]);
        assert_eq!(results, ["tool_b", "tool_a"]);
    }
}

#[test]
fn gemini_mixed_result_ids_reserve_later_explicit_matches() {
    let mut request = gemini_history_with_ids();
    request["contents"][1]["parts"]
        .as_array_mut()
        .unwrap()
        .reverse();
    request["contents"][1]["parts"][0]["functionResponse"]
        .as_object_mut()
        .unwrap()
        .remove("id");
    for target in [
        AiProtocol::OpenAiChat,
        AiProtocol::OpenAiResponses,
        AiProtocol::AnthropicMessages,
    ] {
        let body = convert_request_value(
            ConversionRoute::new(AiProtocol::GeminiNative, target),
            request.clone(),
        )
        .unwrap();
        let (calls, results) = single_batch_ids(target, &body);
        assert_eq!(calls, ["tool_a", "tool_b"]);
        assert_eq!(results, ["tool_b", "tool_a"], "{body}");
    }
}

#[test]
fn gemini_anonymous_results_in_separate_contents_consume_pending_calls_once() {
    let mut request = gemini_history();
    let second_result = request["contents"][1]["parts"]
        .as_array_mut()
        .unwrap()
        .pop()
        .unwrap();
    request["contents"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role": "user", "parts": [second_result]}));
    let body = convert_request_value(
        ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::OpenAiChat),
        request,
    )
    .unwrap();
    let (calls, results) = single_batch_ids(AiProtocol::OpenAiChat, &body);
    assert_eq!(results, calls);
}

#[test]
fn gemini_name_fallback_matches_different_functions_independently() {
    let mut request = gemini_history();
    request["contents"][0]["parts"][1]["functionCall"]["name"] = json!("list_files");
    request["contents"][1]["parts"][1]["functionResponse"]["name"] = json!("list_files");
    request["contents"][1]["parts"]
        .as_array_mut()
        .unwrap()
        .reverse();
    let body = convert_request_value(
        ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::OpenAiChat),
        request,
    )
    .unwrap();
    let (calls, results) = single_batch_ids(AiProtocol::OpenAiChat, &body);
    assert_eq!(results, [calls[1].clone(), calls[0].clone()]);
}

#[test]
fn gemini_missing_call_ids_never_replace_existing_native_ids() {
    let mut request = gemini_history_with_ids();
    request["contents"][0]["parts"][1]["functionCall"]
        .as_object_mut()
        .unwrap()
        .remove("id");
    request["contents"][1]["parts"][1]["functionResponse"]
        .as_object_mut()
        .unwrap()
        .remove("id");
    let body = convert_request_value(
        ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::OpenAiChat),
        request,
    )
    .unwrap();
    let (calls, results) = single_batch_ids(AiProtocol::OpenAiChat, &body);
    assert_eq!(calls[0], "tool_a");
    assert!(calls[1].starts_with("gemini_synth_"));
    assert_eq!(results, calls);
}

#[test]
fn gemini_json_responses_generate_unique_ids_across_separate_turns() {
    let mut reply = protocol_reply(AiProtocol::GeminiNative);
    for part in reply["candidates"][0]["content"]["parts"]
        .as_array_mut()
        .unwrap()
    {
        if let Some(call) = part.get_mut("functionCall") {
            call.as_object_mut().unwrap().remove("id");
        }
    }
    let mut ids = BTreeSet::new();
    for _ in 0..2 {
        let response = convert_response_value(
            ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::OpenAiResponses),
            reply.clone(),
        )
        .unwrap();
        for call in response["output"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["type"] == "function_call")
        {
            assert!(
                ids.insert(call["call_id"].as_str().unwrap().to_string()),
                "tool identity was reused across responses: {response}"
            );
        }
    }
    assert_eq!(ids.len(), 4);
}

async fn convert_stream_events(
    source: AiProtocol,
    target: AiProtocol,
    chunks: &[Value],
) -> Vec<Value> {
    let wire = chunks
        .iter()
        .map(|chunk| {
            let event = chunk["type"]
                .as_str()
                .map(|kind| format!("event: {kind}\n"))
                .unwrap_or_default();
            format!("{event}data: {chunk}\n\n")
        })
        .collect::<String>();
    let stream = stream::iter(
        wire.as_bytes()
            .chunks(11)
            .map(|chunk| Ok(chunk.to_vec()))
            .collect::<Vec<Result<Vec<u8>, String>>>(),
    );
    let mut converted = convert_sse_stream(ConversionRoute::new(source, target), Box::pin(stream));
    let mut bytes = Vec::new();
    while let Some(chunk) = converted.next().await {
        bytes.extend(chunk.unwrap());
    }
    sse_events(&bytes)
}

fn gemini_stream_chunks(include_ids: bool) -> Vec<Value> {
    let mut chunks = Vec::new();
    for (index, (id, path)) in [("tool_a", "first.txt"), ("tool_b", "second.txt")]
        .into_iter()
        .enumerate()
    {
        let mut call = json!({"name": "read_file", "args": {"path": path}});
        if include_ids {
            call["id"] = json!(id);
        }
        chunks.push(json!({"responseId": "gemini_fixture", "modelVersion": "gemini-2.5-pro", "candidates": [
            {"index": 0, "content": {"role": "model", "parts": [{"functionCall": call, "thoughtSignature": format!("fixture-signature-{index}")}]}}
        ]}));
    }
    chunks.push(json!({"responseId": "gemini_fixture", "candidates": [{"index": 0,
        "content": {"role": "model", "parts": [{"text": "Reading both files now."}]}, "finishReason": "STOP"}],
        "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 5, "totalTokenCount": 15}}));
    chunks
}

#[tokio::test]
async fn gemini_parallel_sse_calls_in_separate_events_keep_ids_and_arguments() {
    let events = convert_stream_events(
        AiProtocol::GeminiNative,
        AiProtocol::OpenAiResponses,
        &gemini_stream_chunks(true),
    )
    .await;
    let reply = completed_response(&events);
    let calls = reply["output"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["type"] == "function_call")
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2, "{reply}");
    for (call, (id, path)) in calls
        .iter()
        .zip([("tool_a", "first.txt"), ("tool_b", "second.txt")])
    {
        assert_eq!(call["call_id"], id);
        assert_eq!(
            serde_json::from_str::<Value>(call["arguments"].as_str().unwrap()).unwrap(),
            json!({"path": path})
        );
    }
    let followup = followup_for_reply(AiProtocol::OpenAiResponses, &reply);
    for target in [
        AiProtocol::OpenAiChat,
        AiProtocol::AnthropicMessages,
        AiProtocol::GeminiNative,
    ] {
        let body = convert_request_value(
            ConversionRoute::new(AiProtocol::OpenAiResponses, target),
            followup.clone(),
        )
        .unwrap();
        let (call_ids, results) = single_batch_ids(target, &body);
        assert_eq!(call_ids, ["tool_a", "tool_b"]);
        assert_eq!(results, call_ids);
    }
}

#[tokio::test]
async fn gemini_anonymous_sse_calls_have_distinct_ids_within_and_across_streams() {
    let mut ids = BTreeSet::new();
    for _ in 0..2 {
        let reply = completed_response(
            &convert_stream_events(
                AiProtocol::GeminiNative,
                AiProtocol::OpenAiResponses,
                &gemini_stream_chunks(false),
            )
            .await,
        );
        let calls = reply["output"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["type"] == "function_call")
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2, "{reply}");
        for call in calls {
            assert!(
                ids.insert(call["call_id"].as_str().unwrap().to_string()),
                "{reply}"
            );
        }
    }
    assert_eq!(ids.len(), 4);
}

#[tokio::test]
async fn gemini_parallel_sse_to_chat_keeps_distinct_indices_and_complete_arguments() {
    let events = convert_stream_events(
        AiProtocol::GeminiNative,
        AiProtocol::OpenAiChat,
        &gemini_stream_chunks(true),
    )
    .await;
    let mut calls = BTreeMap::<u64, (String, String, String)>::new();
    for event in &events {
        for call in event
            .pointer("/choices/0/delta/tool_calls")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let entry = calls.entry(call["index"].as_u64().unwrap()).or_default();
            if let Some(id) = call["id"].as_str() {
                entry.0.push_str(id);
            }
            if let Some(name) = call["function"]["name"].as_str() {
                entry.1.push_str(name);
            }
            if let Some(arguments) = call["function"]["arguments"].as_str() {
                entry.2.push_str(arguments);
            }
        }
    }
    assert_eq!(calls.len(), 2, "{events:?}");
    for (index, (id, path)) in [("tool_a", "first.txt"), ("tool_b", "second.txt")]
        .into_iter()
        .enumerate()
    {
        let call = &calls[&(index as u64)];
        assert_eq!(call.0, id);
        assert_eq!(call.1, "read_file");
        assert_eq!(
            serde_json::from_str::<Value>(&call.2).unwrap(),
            json!({"path": path})
        );
    }
    assert_eq!(
        events
            .iter()
            .filter(|event| event
                .pointer("/choices/0/finish_reason")
                .is_some_and(|reason| reason == "tool_calls"))
            .count(),
        1
    );
}

#[tokio::test]
async fn gemini_parallel_sse_to_anthropic_closes_each_tool_before_the_next_block() {
    let events = convert_stream_events(
        AiProtocol::GeminiNative,
        AiProtocol::AnthropicMessages,
        &gemini_stream_chunks(true),
    )
    .await;
    let mut open_block = None;
    let mut calls = BTreeMap::<u64, (String, String)>::new();
    for event in &events {
        let index = event["index"].as_u64();
        match event["type"].as_str() {
            Some("content_block_start") => {
                assert!(open_block.is_none(), "overlapping blocks: {events:?}");
                open_block = index;
                if event["content_block"]["type"] == "tool_use" {
                    calls.insert(
                        index.unwrap(),
                        (
                            event["content_block"]["id"].as_str().unwrap().to_string(),
                            String::new(),
                        ),
                    );
                }
            }
            Some("content_block_delta") if event["delta"]["type"] == "input_json_delta" => {
                assert_eq!(open_block, index);
                calls
                    .get_mut(&index.unwrap())
                    .unwrap()
                    .1
                    .push_str(event["delta"]["partial_json"].as_str().unwrap());
            }
            Some("content_block_stop") => {
                assert_eq!(open_block.take(), index);
            }
            _ => {}
        }
    }
    assert!(open_block.is_none());
    assert_eq!(calls.len(), 2);
    for (call, (id, path)) in calls
        .values()
        .zip([("tool_a", "first.txt"), ("tool_b", "second.txt")])
    {
        assert_eq!(call.0, id);
        assert_eq!(
            serde_json::from_str::<Value>(&call.1).unwrap(),
            json!({"path": path})
        );
    }
    assert_eq!(
        events
            .iter()
            .filter(|event| event["type"] == "message_stop")
            .count(),
        1
    );
}

#[tokio::test]
async fn anthropic_parallel_sse_with_trailing_text_replays_to_all_targets() {
    let mut chunks = vec![json!({"type": "message_start", "message": {
        "id": "msg_fixture", "type": "message", "role": "assistant", "model": "fixture-model",
        "content": [], "usage": {"input_tokens": 10, "output_tokens": 0}
    }})];
    for (index, block) in protocol_reply(AiProtocol::AnthropicMessages)["content"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let mut start = block.clone();
        let delta = if block["type"] == "tool_use" {
            start["input"] = json!({});
            json!({"type": "input_json_delta", "partial_json": block["input"].to_string()})
        } else {
            start["text"] = json!("");
            json!({"type": "text_delta", "text": block["text"]})
        };
        chunks.extend([
            json!({"type": "content_block_start", "index": index, "content_block": start}),
            json!({"type": "content_block_delta", "index": index, "delta": delta}),
            json!({"type": "content_block_stop", "index": index}),
        ]);
    }
    chunks.extend([
        json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 5}}),
        json!({"type": "message_stop"}),
    ]);
    let reply = completed_response(
        &convert_stream_events(
            AiProtocol::AnthropicMessages,
            AiProtocol::OpenAiResponses,
            &chunks,
        )
        .await,
    );
    assert_eq!(reply["usage"]["input_tokens"], 10);
    assert_eq!(reply["usage"]["output_tokens"], 5);
    for target in [
        AiProtocol::OpenAiChat,
        AiProtocol::AnthropicMessages,
        AiProtocol::GeminiNative,
    ] {
        let body = convert_request_value(
            ConversionRoute::new(AiProtocol::OpenAiResponses, target),
            followup_for_reply(AiProtocol::OpenAiResponses, &reply),
        )
        .unwrap();
        let (calls, results) = single_batch_ids(target, &body);
        assert_eq!(calls, ["tool_a", "tool_b"]);
        assert_eq!(results, calls);
        assert_eq!(
            body.to_string().matches("Reading both files now.").count(),
            1
        );
    }
}

#[tokio::test]
async fn late_commentary_output_item_done_history_replays_without_completed_snapshot() {
    let events = responses_stream_with_late_text().await;
    let mut input = events
        .iter()
        .filter(|event| event["type"] == "response.output_item.done")
        .map(|event| event["item"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        input
            .iter()
            .filter(|item| item["type"] == "function_call")
            .count(),
        2
    );
    input.extend([
        tool_output("tool_a", json!("first result")),
        tool_output("tool_b", json!("second result")),
    ]);
    let body = convert_request_value(
        ConversionRoute::new(AiProtocol::OpenAiResponses, AiProtocol::OpenAiChat),
        responses_request(input),
    )
    .unwrap();
    let (calls, results) = single_batch_ids(AiProtocol::OpenAiChat, &body);
    assert_eq!(calls, ["tool_a", "tool_b"]);
    assert_eq!(results, calls);
    assert_eq!(
        body.to_string().matches("Reading both files now.").count(),
        1
    );
}

#[test]
fn parallel_tool_images_share_a_gemini_turn_in_both_media_dialects() {
    for source in [
        AiProtocol::OpenAiResponses,
        AiProtocol::OpenAiChat,
        AiProtocol::AnthropicMessages,
    ] {
        for model in ["gemini-2.5-pro", "gemini-3-pro-preview"] {
            let mut request = protocol_history(source);
            request["model"] = json!(model);
            for (index, image) in ["FIRST_IMAGE", "SECOND_IMAGE"].iter().enumerate() {
                match source {
                    AiProtocol::OpenAiResponses => {
                        request["input"][index + 2]["output"] = json!([
                            {"type": "input_text", "text": format!("result {index}")},
                            {"type": "input_image", "image_url": format!("data:image/png;base64,{image}")}
                        ])
                    }
                    AiProtocol::OpenAiChat => {
                        request["messages"][index + 1]["content"] = json!(json!({"content": [
                            {"type": "text", "text": format!("result {index}")},
                            {"type": "image", "mimeType": "image/png", "data": image}
                        ]})
                        .to_string())
                    }
                    AiProtocol::AnthropicMessages => {
                        request["messages"][1]["content"][index]["content"] = json!([
                            {"type": "text", "text": format!("result {index}")},
                            {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": image}}
                        ])
                    }
                    _ => unreachable!(),
                }
            }
            let body = convert_request_value(
                ConversionRoute::new(source, AiProtocol::GeminiNative),
                request,
            )
            .unwrap();
            let (calls, results) = single_batch_ids(AiProtocol::GeminiNative, &body);
            assert_eq!(calls, ["tool_a", "tool_b"]);
            assert_eq!(results, calls);
            let parts = body["contents"][1]["parts"].as_array().unwrap();
            if model.starts_with("gemini-3") {
                assert_eq!(parts.len(), 2, "{body}");
                assert_eq!(
                    parts[0]["functionResponse"]["parts"][0]["inlineData"]["data"],
                    "FIRST_IMAGE"
                );
                assert_eq!(
                    parts[1]["functionResponse"]["parts"][0]["inlineData"]["data"],
                    "SECOND_IMAGE"
                );
            } else {
                assert_eq!(parts.len(), 6, "{body}");
                assert_eq!(parts[2]["inlineData"]["data"], "FIRST_IMAGE");
                assert_eq!(parts[5]["inlineData"]["data"], "SECOND_IMAGE");
            }
            let chat = convert_request_value(
                ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::OpenAiChat),
                body,
            )
            .unwrap();
            assert_chat_tool_history(&chat);
            let messages = chat["messages"].as_array().unwrap();
            assert_eq!(messages.len(), 4);
            assert_eq!(messages[1]["tool_call_id"], "tool_a");
            assert_eq!(messages[2]["tool_call_id"], "tool_b");
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
    }
}

#[test]
fn gemini_anonymous_parallel_image_results_keep_their_call_pairing() {
    let mut request = gemini_history();
    for (index, data) in ["FIRST_IMAGE", "SECOND_IMAGE"].iter().enumerate() {
        request["contents"][1]["parts"][index]["functionResponse"]["parts"] = json!([
            {"inlineData": {"mimeType": "image/png", "data": data}}
        ]);
    }
    let chat = convert_request_value(
        ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::OpenAiChat),
        request,
    )
    .unwrap();
    assert_chat_tool_history(&chat);
    let messages = chat["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 4);
    assert_ne!(
        messages[0]["tool_calls"][0]["id"],
        messages[0]["tool_calls"][1]["id"]
    );
    assert_eq!(
        messages[1]["tool_call_id"],
        messages[0]["tool_calls"][0]["id"]
    );
    assert_eq!(
        messages[2]["tool_call_id"],
        messages[0]["tool_calls"][1]["id"]
    );
    assert_eq!(messages[3]["role"], "user");
}

#[test]
fn gemini_result_groups_do_not_cross_ordinary_user_or_new_model_turns() {
    let mut request = chat_history();
    let mut second = chat_history()["messages"].as_array().unwrap().clone();
    second[0]["tool_calls"][0]["id"] = json!("tool_c");
    second[0]["tool_calls"][1]["id"] = json!("tool_d");
    second[1]["tool_call_id"] = json!("tool_c");
    second[2]["tool_call_id"] = json!("tool_d");
    let messages = request["messages"].as_array_mut().unwrap();
    messages.push(json!({"role": "user", "content": "Next task"}));
    messages.extend(second);
    let body = convert_request_value(
        ConversionRoute::new(AiProtocol::OpenAiChat, AiProtocol::GeminiNative),
        request,
    )
    .unwrap();
    let contents = body["contents"].as_array().unwrap();
    assert_eq!(contents.len(), 5, "{body}");
    assert_eq!(contents[1]["parts"][0]["functionResponse"]["id"], "tool_a");
    assert_eq!(contents[1]["parts"][1]["functionResponse"]["id"], "tool_b");
    assert_eq!(contents[2]["parts"][0]["text"], "Next task");
    assert_eq!(contents[4]["parts"][0]["functionResponse"]["id"], "tool_c");
    assert_eq!(contents[4]["parts"][1]["functionResponse"]["id"], "tool_d");
}

#[test]
fn gemini_unmatched_result_does_not_reuse_a_consumed_call_id() {
    let mut request = gemini_history();
    request["contents"][1]["parts"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "functionResponse": {"name": "read_file", "response": {"result": "unmatched"}}
        }));
    let body = convert_request_value(
        ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::OpenAiChat),
        request,
    )
    .unwrap();
    assert_eq!(body["messages"][3]["tool_call_id"], "read_file");
    assert!(validate_chat_tool_history(&body)
        .unwrap_err()
        .contains("unexpected result"));
}

#[test]
fn identity_requests_for_all_protocols_keep_the_original_wire_bytes() {
    for protocol in PROTOCOLS {
        let mut request = if protocol == AiProtocol::GeminiNative {
            gemini_history()
        } else {
            protocol_history(protocol)
        };
        request["future_extension"] = json!({"keep": true});
        let bytes = serde_json::to_vec_pretty(&request).unwrap();
        assert_eq!(
            convert_request_body(ConversionRoute::new(protocol, protocol), &bytes).unwrap(),
            bytes
        );
    }
}

#[test]
fn gemini_anonymous_results_return_to_call_order_before_synthetic_ids_are_removed() {
    for client in [
        AiProtocol::OpenAiChat,
        AiProtocol::OpenAiResponses,
        AiProtocol::AnthropicMessages,
    ] {
        let mut request = convert_request_value(
            ConversionRoute::new(AiProtocol::GeminiNative, client),
            gemini_history(),
        )
        .unwrap();
        match client {
            AiProtocol::OpenAiChat => request["messages"].as_array_mut().unwrap().swap(1, 2),
            AiProtocol::OpenAiResponses => request["input"].as_array_mut().unwrap().swap(2, 3),
            AiProtocol::AnthropicMessages => request["messages"][1]["content"]
                .as_array_mut()
                .unwrap()
                .reverse(),
            _ => unreachable!(),
        }
        let body = convert_request_value(
            ConversionRoute::new(client, AiProtocol::GeminiNative),
            request,
        )
        .unwrap();
        assert_eq!(
            body["contents"][1]["parts"][0]["functionResponse"]["response"]["result"],
            "first result",
            "{body}"
        );
        assert_eq!(
            body["contents"][1]["parts"][1]["functionResponse"]["response"]["result"],
            "second result"
        );
        assert!(body["contents"][1]["parts"][0]["functionResponse"]
            .get("id")
            .is_none());
        assert!(body["contents"][1]["parts"][1]["functionResponse"]
            .get("id")
            .is_none());
    }
}

#[test]
fn gemini_anonymous_result_reordering_keeps_each_nested_image_with_its_call() {
    let mut native = gemini_history();
    for (index, data) in ["FIRST_IMAGE", "SECOND_IMAGE"].iter().enumerate() {
        native["contents"][1]["parts"][index]["functionResponse"]["parts"] = json!([
            {"inlineData": {"mimeType": "image/png", "data": data}}
        ]);
    }
    let mut responses = convert_request_value(
        ConversionRoute::new(AiProtocol::GeminiNative, AiProtocol::OpenAiResponses),
        native,
    )
    .unwrap();
    responses["input"].as_array_mut().unwrap().swap(2, 3);
    responses["model"] = json!("gemini-3-pro-preview");
    let body = convert_request_value(
        ConversionRoute::new(AiProtocol::OpenAiResponses, AiProtocol::GeminiNative),
        responses,
    )
    .unwrap();
    let results = body["contents"][1]["parts"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(
        results[0]["functionResponse"]["parts"][0]["inlineData"]["data"],
        "FIRST_IMAGE"
    );
    assert_eq!(
        results[1]["functionResponse"]["parts"][0]["inlineData"]["data"],
        "SECOND_IMAGE"
    );
    assert!(results[0]["functionResponse"].get("id").is_none());
    assert!(results[1]["functionResponse"].get("id").is_none());
}

#[tokio::test]
async fn gemini_sse_same_id_updates_emit_one_complete_tool_call() {
    let mut chunks = gemini_stream_chunks(true);
    chunks.insert(2, json!({"responseId": "gemini_fixture", "candidates": [{"index": 0, "content": {"role": "model", "parts": [
        {"functionCall": {"id": "tool_a", "name": "read_file", "args": {"encoding": "utf8"}}}
    ]}}]}));
    let reply = completed_response(
        &convert_stream_events(
            AiProtocol::GeminiNative,
            AiProtocol::OpenAiResponses,
            &chunks,
        )
        .await,
    );
    let calls = reply["output"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["type"] == "function_call")
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2, "{reply}");
    assert_eq!(calls[0]["call_id"], "tool_a");
    assert_eq!(
        serde_json::from_str::<Value>(calls[0]["arguments"].as_str().unwrap()).unwrap(),
        json!({"path": "first.txt", "encoding": "utf8"})
    );
    assert_eq!(calls[1]["call_id"], "tool_b");
    assert_eq!(
        serde_json::from_str::<Value>(calls[1]["arguments"].as_str().unwrap()).unwrap(),
        json!({"path": "second.txt"})
    );
}
