//! Native-tool and rewrite markers over the real HTTP route.
//!
//! The unit tests in `runtime::observations` cover the pure scanners. These
//! drive the gateway end to end to prove the markers are *mounted*: declared
//! from the client body, forwarded from the body actually sent upstream, used
//! from the upstream reply, and persisted in the JSONL detail (never in the
//! SQLite summary, which is what the request list reads).

use super::support::*;
use ai_toolbox_lib::coding::proxy_gateway::request_log;
use ai_toolbox_lib::coding::proxy_gateway::types::{
    GatewayRequestLogDetail, GatewayStreamOutcome, ProxyGatewayRequestLogListInput,
    ProxyGatewaySettings,
};
use ai_toolbox_lib::db::{helpers::db_put, schema::DbTable};
use ai_toolbox_lib::http_client;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};

async fn accept_upstream(listener: &TcpListener) -> TcpStream {
    tokio::time::timeout(Duration::from_secs(10), listener.accept())
        .await
        .expect("gateway must send an upstream request")
        .unwrap()
        .0
}

fn responses_request(tools: Value) -> Value {
    json!({
        "model": "fixture-model",
        "stream": false,
        "input": [
            { "type": "message", "role": "user", "content": [
                { "type": "input_text", "text": "search for the answer" }
            ] }
        ],
        "tools": tools
    })
}

fn responses_search_reply() -> Value {
    json!({
        "id": "resp_search", "object": "response", "status": "completed",
        "model": "fixture-model",
        "output": [
            { "type": "web_search_call", "id": "ws_1", "status": "completed" },
            { "type": "message", "role": "assistant", "content": [
                { "type": "output_text", "text": "found it" }
            ] }
        ],
        "usage": { "input_tokens": 11, "output_tokens": 4, "total_tokens": 15 }
    })
}

fn chat_reply() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "id": "chatcmpl_fixture", "object": "chat.completion", "model": "fixture-model",
        "choices": [{ "index": 0, "message": { "role": "assistant", "content": "found it" },
                      "finish_reason": "stop" }],
        "usage": { "prompt_tokens": 7, "completion_tokens": 2, "total_tokens": 9 }
    }))
    .unwrap()
}

/// Observability is recorded after the client response is written, so the
/// JSONL detail appears asynchronously. Poll instead of assuming ordering.
fn only_recorded_detail(gateway: &RunningGateway) -> GatewayRequestLogDetail {
    for _ in 0..100 {
        let summaries = request_log::list_request_logs(
            &gateway.paths(),
            ProxyGatewayRequestLogListInput { limit: Some(10) },
        )
        .expect("list request logs");
        if let Some(trace_id) = summaries.first().map(|summary| summary.trace_id.clone()) {
            if let Some(detail) = request_log::get_request_log_detail(&gateway.paths(), &trace_id)
                .expect("read request log")
            {
                return detail;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("one recorded request with a JSONL detail");
}

#[tokio::test]
async fn a_native_search_is_marked_declared_forwarded_and_used() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let upstream_url = format!("http://{}", listener.local_addr().unwrap());
    let gateway = RunningGateway::new_for_api_format(
        &upstream_url,
        "custom",
        BodyLogging::Enabled,
        "openai_responses",
    );

    let upstream = tokio::spawn(async move {
        let mut socket = accept_upstream(&listener).await;
        let forwarded = read_mock_request(&mut socket, "/v1/responses").await;
        assert_eq!(
            forwarded["tools"][0]["type"], "web_search",
            "the declaration must reach the upstream unchanged: {forwarded}"
        );
        write_mock_reply(
            &mut socket,
            200,
            &serde_json::to_vec(&responses_search_reply()).unwrap(),
            false,
        )
        .await;
    });

    let client = http_client::create_client_no_proxy(10).unwrap();
    let response = client
        .post(&gateway.url)
        .json(&responses_request(json!([{ "type": "web_search" }])))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    upstream.await.unwrap();

    let detail = only_recorded_detail(&gateway);
    let search = detail
        .native_tools
        .iter()
        .find(|tool| tool.kind == "web_search")
        .expect("web search marker");
    assert!(search.declared);
    assert!(search.forwarded);
    assert!(search.used);
    assert!(
        !detail
            .rewrites
            .iter()
            .any(|rewrite| rewrite.kind == "native_tool_filtered"),
        "nothing was filtered, so no filter marker: {:?}",
        detail.rewrites
    );
}

#[tokio::test]
async fn a_tool_dropped_by_protocol_conversion_is_marked_filtered() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let upstream_url = format!("http://{}", listener.local_addr().unwrap());
    // Responses source → Chat target: the hosted tool has no Chat representation
    // and the outbound adapter filters it.
    let gateway = RunningGateway::new_for_api_format(
        &upstream_url,
        "custom",
        BodyLogging::Enabled,
        "openai_chat",
    );

    let upstream = tokio::spawn(async move {
        let mut socket = accept_upstream(&listener).await;
        let forwarded = read_mock_request(&mut socket, "/v1/chat/completions").await;
        let forwarded_tools = forwarded["tools"].as_array().cloned().unwrap_or_default();
        assert!(
            forwarded_tools
                .iter()
                .all(|tool| tool["type"].as_str() != Some("web_search")),
            "the hosted tool must not survive into a Chat body: {forwarded}"
        );
        write_mock_reply(&mut socket, 200, &chat_reply(), false).await;
    });

    let client = http_client::create_client_no_proxy(10).unwrap();
    let response = client
        .post(&gateway.url)
        .json(&responses_request(json!([
            { "type": "function", "name": "shell", "parameters": {"type": "object", "properties": {}} },
            { "type": "web_search" }
        ])))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    upstream.await.unwrap();

    let detail = only_recorded_detail(&gateway);
    let search = detail
        .native_tools
        .iter()
        .find(|tool| tool.kind == "web_search")
        .expect("web search marker");
    assert!(search.declared);
    assert!(!search.forwarded, "the declaration was dropped");
    assert!(
        detail.rewrites.iter().any(|rewrite| {
            rewrite.kind == "native_tool_filtered"
                && rewrite.detail.as_deref() == Some("web_search")
        }),
        "the filter must leave a marker: {:?}",
        detail.rewrites
    );
}

#[tokio::test]
async fn detail_only_markers_do_not_exist_without_request_logging() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let upstream_url = format!("http://{}", listener.local_addr().unwrap());
    // Request logging off: the markers live only in the JSONL detail, so no
    // detail file may be written at all (and the list stays empty).
    let gateway = RunningGateway::new_for_api_format(
        &upstream_url,
        "custom",
        BodyLogging::Disabled,
        "openai_responses",
    );

    let upstream = tokio::spawn(async move {
        let mut socket = accept_upstream(&listener).await;
        let _ = read_mock_request(&mut socket, "/v1/responses").await;
        write_mock_reply(
            &mut socket,
            200,
            &serde_json::to_vec(&responses_search_reply()).unwrap(),
            false,
        )
        .await;
    });

    let client = http_client::create_client_no_proxy(10).unwrap();
    let response = client
        .post(&gateway.url)
        .json(&responses_request(json!([{ "type": "web_search" }])))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    upstream.await.unwrap();

    assert!(
        request_log::list_request_logs(
            &gateway.paths(),
            ProxyGatewayRequestLogListInput { limit: Some(10) },
        )
        .unwrap()
        .is_empty(),
        "request logging is off, so no JSONL detail exists"
    );
}

fn metadata_only(settings: &mut ProxyGatewaySettings) {
    settings.request_log_enabled = true;
    settings.store_request_body = false;
    settings.store_response_body = false;
    settings.store_headers = false;
}

fn assert_no_stored_payloads(detail: &GatewayRequestLogDetail) {
    assert!(detail.request_body.is_none());
    assert!(detail.upstream_request_body.is_none());
    assert!(detail.response_body.is_none());
    assert!(detail.upstream_response_body.is_none());
    assert!(detail.request_headers.is_none());
    assert!(detail.response_headers.is_none());
}

fn assert_completed_stream(detail: &GatewayRequestLogDetail) {
    assert!(detail.summary.is_streaming);
    assert!(detail.summary.success);
    assert_eq!(detail.summary.status_code, Some(200));
    assert_eq!(
        detail.summary.stream_outcome,
        Some(GatewayStreamOutcome::Completed)
    );
    assert_no_stored_payloads(detail);
}

fn chat_text_request(streaming: bool) -> Value {
    json!({
        "model": "fixture-model", "stream": streaming,
        "messages": [{"role": "user", "content": "give a short answer"}]
    })
}

fn chat_text_stream(text: &str) -> Vec<u8> {
    let chunks = [
        json!({"id": "chatcmpl_observations", "object": "chat.completion.chunk",
            "model": "fixture-model", "choices": [{"index": 0,
                "delta": {"role": "assistant", "content": text}, "finish_reason": null}]}),
        json!({"id": "chatcmpl_observations", "object": "chat.completion.chunk",
            "model": "fixture-model", "choices": [{"index": 0,
                "delta": {}, "finish_reason": "stop"}]}),
        json!({"id": "chatcmpl_observations", "object": "chat.completion.chunk",
            "model": "fixture-model", "choices": [],
            "usage": {"prompt_tokens": 7, "completion_tokens": 2, "total_tokens": 9}}),
    ];
    let mut wire = chunks
        .iter()
        .map(|chunk| format!("data: {chunk}\n\n"))
        .collect::<String>();
    wire.push_str("data: [DONE]\n\n");
    wire.into_bytes()
}

fn responses_search_stream() -> Vec<u8> {
    let mut reply = responses_search_reply();
    reply["output"][1]["id"] = json!("msg_search");
    reply["output"][1]["status"] = json!("completed");
    reply["output"][1]["content"][0]["annotations"] = json!([]);
    let message = &reply["output"][1];
    let events = [
        json!({"type": "response.created", "response": {
            "id": "resp_search", "object": "response", "status": "in_progress",
            "model": "fixture-model", "output": []}}),
        json!({"type": "response.output_item.added", "output_index": 0,
            "item": {"type": "web_search_call", "id": "ws_1", "status": "in_progress"}}),
        json!({"type": "response.output_item.done", "output_index": 0,
            "item": reply["output"][0]}),
        json!({"type": "response.output_item.added", "output_index": 1,
            "item": {"id": "msg_search", "type": "message", "role": "assistant",
                "status": "in_progress", "content": []}}),
        json!({"type": "response.content_part.added", "item_id": "msg_search",
            "output_index": 1, "content_index": 0,
            "part": {"type": "output_text", "text": "", "annotations": []}}),
        json!({"type": "response.output_text.delta", "item_id": "msg_search",
            "output_index": 1, "content_index": 0, "delta": "found it"}),
        json!({"type": "response.output_text.done", "item_id": "msg_search",
            "output_index": 1, "content_index": 0, "text": "found it"}),
        json!({"type": "response.content_part.done", "item_id": "msg_search",
            "output_index": 1, "content_index": 0, "part": message["content"][0]}),
        json!({"type": "response.output_item.done", "output_index": 1, "item": message}),
        json!({"type": "response.completed", "response": reply}),
    ];
    events
        .iter()
        .map(|event| format!("event: {}\ndata: {event}\n\n", event["type"].as_str().unwrap()))
        .collect::<String>()
        .into_bytes()
}

fn assert_responses_stream(bytes: &[u8], expected_text: &str) {
    let events = sse_events(bytes);
    let completed = events
        .iter()
        .filter(|event| event["type"] == "response.completed")
        .collect::<Vec<_>>();
    assert_eq!(completed.len(), 1, "one Responses terminal: {events:?}");
    assert_eq!(completed[0]["response"]["status"], "completed");
    assert!(events
        .iter()
        .all(|event| event["type"] != "response.failed" && event["type"] != "error"));
    let text = events
        .iter()
        .filter(|event| event["type"] == "response.output_text.delta")
        .filter_map(|event| event["delta"].as_str())
        .collect::<String>();
    assert_eq!(text, expected_text);
}

fn assert_chat_stream(bytes: &[u8], expected_text: &str) {
    let events = sse_events(bytes);
    assert!(events.iter().all(|event| event.get("error").is_none()));
    let text = events
        .iter()
        .filter_map(|event| event["choices"][0]["delta"]["content"].as_str())
        .collect::<String>();
    assert_eq!(text, expected_text);
    assert_eq!(
        events
            .iter()
            .filter(|event| event["choices"][0]["finish_reason"] == "stop")
            .count(),
        1
    );
    assert_eq!(
        std::str::from_utf8(bytes)
            .unwrap()
            .matches("data: [DONE]")
            .count(),
        1
    );
}

#[tokio::test]
async fn streaming_native_search_keeps_declared_forwarded_and_used_without_bodies() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let gateway = RunningGateway::new_for_api_format_configured(
        &format!("http://{}", listener.local_addr().unwrap()),
        "custom",
        BodyLogging::Enabled,
        "openai_responses",
        metadata_only,
    );
    let upstream = tokio::spawn(async move {
        let mut socket = accept_upstream(&listener).await;
        let forwarded = read_mock_request(&mut socket, "/v1/responses").await;
        assert_eq!(forwarded["stream"], true);
        assert_eq!(forwarded["tools"][0]["type"], "web_search");
        write_mock_reply(&mut socket, 200, &responses_search_stream(), true).await;
    });
    let mut request = responses_request(json!([{"type": "web_search"}]));
    request["stream"] = json!(true);
    let response = http_client::create_client_no_proxy(10)
        .unwrap()
        .post(&gateway.url)
        .json(&request)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_responses_stream(&response.bytes().await.unwrap(), "found it");
    upstream.await.unwrap();

    let detail = only_recorded_detail(&gateway);
    assert_completed_stream(&detail);
    let search = detail
        .native_tools
        .iter()
        .find(|tool| tool.kind == "web_search")
        .expect("upstream native search marker");
    assert!(
        search.declared,
        "the streaming response must retain request observations"
    );
    assert!(search.forwarded);
    assert!(search.used);
    assert!(!detail
        .rewrites
        .iter()
        .any(|rewrite| rewrite.kind == "native_tool_filtered"));
}

#[tokio::test]
async fn streaming_filtered_search_keeps_the_request_rewrite_without_bodies() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let gateway = RunningGateway::new_for_api_format_configured(
        &format!("http://{}", listener.local_addr().unwrap()),
        "custom",
        BodyLogging::Enabled,
        "openai_chat",
        metadata_only,
    );
    let upstream = tokio::spawn(async move {
        let mut socket = accept_upstream(&listener).await;
        let forwarded = read_mock_request(&mut socket, "/v1/chat/completions").await;
        assert_eq!(forwarded["stream"], true);
        assert!(!forwarded.to_string().contains("web_search"));
        write_mock_reply(&mut socket, 200, &chat_text_stream("found it"), true).await;
    });
    let mut request = responses_request(json!([{"type": "web_search"}]));
    request["stream"] = json!(true);
    let response = http_client::create_client_no_proxy(10)
        .unwrap()
        .post(&gateway.url)
        .json(&request)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_responses_stream(&response.bytes().await.unwrap(), "found it");
    upstream.await.unwrap();

    let detail = only_recorded_detail(&gateway);
    assert_completed_stream(&detail);
    let search = detail
        .native_tools
        .iter()
        .find(|tool| tool.kind == "web_search")
        .expect("filtered declaration must survive the streaming response");
    assert!(search.declared);
    assert!(!search.forwarded);
    assert!(!search.used);
    assert!(detail.rewrites.iter().any(|rewrite| {
        rewrite.kind == "native_tool_filtered"
            && rewrite.detail.as_deref() == Some("web_search")
    }));
}

#[tokio::test]
async fn streaming_used_search_is_observed_before_response_conversion_without_bodies() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let gateway = RunningGateway::new_for_api_format_configured(
        &format!("http://{}", listener.local_addr().unwrap()),
        "custom",
        BodyLogging::Enabled,
        "openai_responses",
        metadata_only,
    );
    let upstream = tokio::spawn(async move {
        let mut socket = accept_upstream(&listener).await;
        let forwarded = read_mock_request(&mut socket, "/v1/responses").await;
        assert_eq!(forwarded["stream"], true);
        assert!(!forwarded.to_string().contains("web_search"));
        write_mock_reply(&mut socket, 200, &responses_search_stream(), true).await;
    });
    let response = http_client::create_client_no_proxy(10)
        .unwrap()
        .post(gateway.url.replace("/responses", "/chat/completions"))
        .json(&chat_text_request(true))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let bytes = response.bytes().await.unwrap();
    assert_chat_stream(&bytes, "found it");
    assert!(
        !std::str::from_utf8(&bytes).unwrap().contains("web_search_call"),
        "Chat has no hosted-call item; observing the client stream cannot prove upstream use"
    );
    upstream.await.unwrap();

    let detail = only_recorded_detail(&gateway);
    assert_completed_stream(&detail);
    let search = detail
        .native_tools
        .iter()
        .find(|tool| tool.kind == "web_search")
        .expect("upstream use must be recorded before conversion drops the hosted-call item");
    assert!(!search.declared);
    assert!(!search.forwarded);
    assert!(search.used);
}

#[tokio::test]
async fn streaming_text_marker_is_not_tool_usage() {
    // The JSON reply is a control for the same literal text in a legal SSE
    // delta. Neither reply contains a tool call or even a tool declaration.
    for streaming in [false, true] {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let gateway = RunningGateway::new_for_api_format_configured(
            &format!("http://{}", listener.local_addr().unwrap()),
            "custom",
            BodyLogging::Enabled,
            "openai_chat",
            metadata_only,
        );
        let upstream = tokio::spawn(async move {
            let mut socket = accept_upstream(&listener).await;
            let forwarded = read_mock_request(&mut socket, "/v1/chat/completions").await;
            assert_eq!(forwarded["stream"], streaming);
            let body = if streaming {
                chat_text_stream("web_search_call")
            } else {
                let mut reply: Value = serde_json::from_slice(&chat_reply()).unwrap();
                reply["choices"][0]["message"]["content"] = json!("web_search_call");
                serde_json::to_vec(&reply).unwrap()
            };
            write_mock_reply(&mut socket, 200, &body, streaming).await;
        });
        let response = http_client::create_client_no_proxy(10)
        .unwrap()
            .post(gateway.url.replace("/responses", "/chat/completions"))
            .json(&chat_text_request(streaming))
        .send()
        .await
        .unwrap();
        assert_eq!(response.status(), 200);
        let bytes = response.bytes().await.unwrap();
        if streaming {
            assert_chat_stream(&bytes, "web_search_call");
        } else {
            let reply: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(reply["choices"][0]["message"]["content"], "web_search_call");
            assert_eq!(reply["choices"][0]["finish_reason"], "stop");
        }
        upstream.await.unwrap();
        let detail = only_recorded_detail(&gateway);
        assert_no_stored_payloads(&detail);
        if streaming {
            assert_completed_stream(&detail);
        }
        assert!(
            detail.native_tools.is_empty(),
            "ordinary text is not native-tool use (streaming={streaming}): {:?}",
            detail.native_tools
        );
    }
}

#[tokio::test]
async fn privacy_lossy_observations_do_not_persist_secrets_without_bodies() {
    // A unique synthetic password-shaped value, never a real credential.
    const SECRET: &str = "fixture_observation_private_7b3e";
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let gateway = RunningGateway::new_for_api_format_configured_with_database(
        &format!("http://{}", listener.local_addr().unwrap()),
        "custom",
        BodyLogging::Enabled,
        "openai_responses",
        metadata_only,
        |db| {
            db.with_conn(|connection| {
                db_put(
                    connection,
                    DbTable::ProxyGatewaySettings,
                    "privacy",
                    &json!({"enabled": true}),
                )
            })
            .unwrap()
        },
    );
    let upstream = tokio::spawn(async move {
        let mut socket = accept_upstream(&listener).await;
        let forwarded = read_mock_request(&mut socket, "/v1/responses").await;
        let forwarded_text = forwarded.to_string();
        assert!(
            !forwarded_text.contains(SECRET),
            "privacy must be active before send"
        );
        assert!(
            forwarded_text.contains("__AITB_PRIV_"),
            "the retained text must really be protected"
        );
        assert!(forwarded.get("modalities").is_none());
        write_mock_reply(
            &mut socket,
            200,
            &serde_json::to_vec(&responses_search_reply()).unwrap(),
            false,
        )
        .await;
    });
    let mut request = chat_text_request(false);
    request["modalities"] = json!(["text", format!("api_key={SECRET}")]);
    request["messages"][0]["content"] = json!(format!("Please protect api_key={SECRET}"));
    let response = http_client::create_client_no_proxy(10)
        .unwrap()
        .post(gateway.url.replace("/responses", "/chat/completions"))
        .json(&request)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let reply: Value = response.json().await.unwrap();
    // Responses text becomes IR parts; the Chat reverse adapter preserves
    // those as typed content parts even when there is only one text part.
    assert_eq!(
        reply["choices"][0]["message"]["content"],
        json!([{"type": "text", "text": "found it"}])
    );
    assert_eq!(reply["choices"][0]["finish_reason"], "stop");
    upstream.await.unwrap();

    let detail = only_recorded_detail(&gateway);
    assert_no_stored_payloads(&detail);
    assert!(detail.summary.success);
    let privacy = detail
        .privacy
        .as_ref()
        .expect("the runtime loaded privacy settings");
    assert!(privacy.matched_values > 0);
    assert!(!privacy.failed);
    assert!(
        detail.rewrites.iter().any(|rewrite| rewrite.kind == "lossy_conversion"),
        "retain a useful diagnostic instead of suppressing all observations"
    );
    let raw_jsonl = std::fs::read_dir(gateway.paths().request_log_root())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("jsonl"))
        .map(|path| std::fs::read_to_string(path).unwrap())
        .collect::<String>();
    assert!(
        raw_jsonl.contains(&detail.summary.trace_id),
        "verify the persisted record, not an empty log"
    );
    assert!(
        !raw_jsonl.contains(SECRET),
        "lossy diagnostics must not bypass privacy in JSONL"
    );
}
