use super::*;
use crate::coding::proxy_gateway::privacy::{
    CompiledPolicy, PrivacyCustomRule, PrivacyRuleKind, PrivacySettings,
};

pub(super) const SECRET: &str = "secret\"line\nvalue";

pub(super) fn enable_privacy(context: &GatewayRuntimeContext) {
    let mut settings = PrivacySettings::default();
    settings.enabled = true;
    settings.rules.custom.push(PrivacyCustomRule {
        id: "integration".into(),
        name: "Integration".into(),
        enabled: true,
        kind: PrivacyRuleKind::Literal,
        pattern: SECRET.into(),
        priority: 200,
    });
    context
        .privacy
        .publish(CompiledPolicy::compile(&settings).unwrap());
    context.settings.write().unwrap().log_max_body_size_kb = 64;
}

pub(super) fn token(body: &[u8]) -> String {
    let text = std::str::from_utf8(body).unwrap();
    assert!(
        !text.contains("secret"),
        "outbound request leaked original text"
    );
    regex::Regex::new(r"__AITB_PRIV_[a-z0-9]+_[a-f0-9]+__")
        .unwrap()
        .find(text)
        .expect("outbound placeholder")
        .as_str()
        .to_string()
}

pub(super) fn upstream_json(protocol: &str, token: &str) -> Value {
    match protocol {
        "openai_chat" => {
            json!({"id":"chat_privacy","model":"test-model","choices":[{"index":0,"message":{"role":"assistant","content":token,"tool_calls":[{"id":"tool_1","type":"function","function":{"name":"write_file","arguments":json!({"value":token}).to_string()}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":12,"completion_tokens":3}})
        }
        "anthropic_messages" => {
            json!({"id":"msg_privacy","type":"message","role":"assistant","model":"test-model","content":[{"type":"text","text":token},{"type":"tool_use","id":"tool_1","name":"write_file","input":{"value":token}}],"stop_reason":"tool_use","usage":{"input_tokens":12,"output_tokens":3}})
        }
        "gemini_native" => {
            json!({"responseId":"gemini_privacy","candidates":[{"index":0,"content":{"role":"model","parts":[{"text":token},{"functionCall":{"name":"write_file","args":{"value":token}}}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":12,"candidatesTokenCount":3,"totalTokenCount":15}})
        }
        _ => {
            json!({"id":"resp_privacy","object":"response","model":"test-model","status":"completed","output":[{"type":"message","id":"message_1","role":"assistant","status":"completed","content":[{"type":"output_text","text":token,"annotations":[]}]},{"type":"function_call","id":"function_1","call_id":"tool_1","name":"write_file","arguments":json!({"value":token}).to_string(),"status":"completed"}],"usage":{"input_tokens":12,"output_tokens":3}})
        }
    }
}

#[tokio::test]
async fn privacy_http_converts_requests_before_redacting_and_restores_json_tools_after_conversion()
{
    for protocol in [
        "openai_responses",
        "openai_chat",
        "anthropic_messages",
        "gemini_native",
    ] {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let (_directory, context, _) = test_context(
            &format!("http://{}/v1", listener.local_addr().unwrap()),
            protocol,
            true,
        );
        enable_privacy(&context);
        let upstream = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = super::super::super::http_io::read_http_request(&mut socket, 0)
                .await
                .unwrap();
            let placeholder = token(&request.body);
            let payload: Value = serde_json::from_slice(&request.body).unwrap();
            assert_eq!(
                payload
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or("test-model"),
                "test-model"
            );
            let body = upstream_json(protocol, &placeholder).to_string();
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        });
        let (address, gateway) = start_gateway(context.clone(), 1).await;
        let response = crate::http_client::create_client_no_proxy(10).unwrap()
            .post(format!("http://{address}/openai/v1/responses"))
            .json(&json!({"model":"test-model","input":SECRET,"tools":[{"type":"function","name":"write_file","parameters":{"type":"object","properties":{"value":{"type":"string"}}}}]}))
            .send().await.unwrap();
        assert_eq!(response.status(), 200, "{protocol}");
        let body: Value = response.json().await.unwrap();
        let output = body["output"].as_array().unwrap();
        let call = output
            .iter()
            .find(|item| item["type"] == "function_call")
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(call["arguments"].as_str().unwrap()).unwrap()["value"],
            SECRET,
            "{protocol}"
        );
        assert!(!body.to_string().contains("__AITB_PRIV_"), "{protocol}");
        gateway.await.unwrap();
        upstream.await.unwrap();
        let details = recorded_details(&context);
        assert_eq!(details.len(), 1, "{protocol}");
        let detail = &details[0];
        assert!(detail.summary.success, "{protocol}");
        assert_eq!(detail.summary.total_tokens, Some(15), "{protocol}");
        assert_eq!(detail.privacy.as_ref().unwrap().matched_values, 1);
        assert!(detail.privacy.as_ref().unwrap().log_redacted);
        assert!(
            detail.upstream_response_body.is_some(),
            "{protocol}: keep pre-restoration upstream detail"
        );
        assert!(
            !serde_json::to_string(detail).unwrap().contains("secret"),
            "{protocol}: log leaked text"
        );
    }
}

#[tokio::test]
async fn privacy_websocket_restores_split_arguments_tracks_usage_and_continues_previous_response() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let (_directory, context, _) = test_context(
        &format!("http://{}/v1", listener.local_addr().unwrap()),
        "openai_responses",
        true,
    );
    enable_privacy(&context);
    let upstream = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_async(socket).await.unwrap();
        let request = receive_json(&mut socket).await;
        let placeholder = token(request.to_string().as_bytes());
        send_json(
            &mut socket,
            json!({"type":"response.created","response":{"id":"resp_ws_first"}}),
        )
        .await;
        let arguments = json!({"value":placeholder}).to_string();
        for fragment in arguments.as_bytes().chunks(3) {
            send_json(&mut socket, json!({"type":"response.function_call_arguments.delta","response_id":"resp_ws_first","item_id":"function_1","output_index":0,"delta":std::str::from_utf8(fragment).unwrap()})).await;
        }
        send_json(&mut socket, json!({"type":"response.function_call_arguments.done","response_id":"resp_ws_first","item_id":"function_1","arguments":arguments})).await;
        send_json(&mut socket, json!({"type":"response.completed","response":{"id":"resp_ws_first","status":"completed","usage":{"input_tokens":10,"output_tokens":2}}})).await;
        let previous = receive_json(&mut socket).await;
        assert_eq!(previous["previous_response_id"], "resp_ws_first");
        send_json(&mut socket, json!({"type":"response.output_text.delta","response_id":"resp_ws_second","delta":placeholder})).await;
        send_json(&mut socket, json!({"type":"response.completed","response":{"id":"resp_ws_second","status":"completed","usage":{"input_tokens":11,"output_tokens":3}}})).await;
        let _ = socket.next().await;
    });
    let (mut client, gateway) = gateway_connection(context.clone()).await;
    send_json(
        &mut client,
        json!({"type":"response.create","model":"test-model","input":SECRET}),
    )
    .await;
    let mut arguments = String::new();
    loop {
        let event = receive_json(&mut client).await;
        match event["type"].as_str().unwrap() {
            "response.function_call_arguments.delta" => {
                arguments.push_str(event["delta"].as_str().unwrap())
            }
            "response.function_call_arguments.done" => assert_eq!(
                serde_json::from_str::<Value>(event["arguments"].as_str().unwrap()).unwrap()
                    ["value"],
                SECRET
            ),
            "response.completed" => break,
            _ => {}
        }
    }
    assert_eq!(
        serde_json::from_str::<Value>(&arguments).unwrap()["value"],
        SECRET
    );
    send_json(&mut client, json!({"type":"response.create","model":"test-model","previous_response_id":"resp_ws_first","input":"continue"})).await;
    assert_eq!(receive_json(&mut client).await["delta"], SECRET);
    assert_eq!(
        receive_json(&mut client).await["type"],
        "response.completed"
    );
    let _ = client.close(None).await;
    gateway.await.unwrap();
    upstream.await.unwrap();
    let details = recorded_details(&context);
    assert_eq!(details.len(), 2);
    assert!(details.iter().all(|detail| detail.summary.success));
    assert_eq!(
        details
            .iter()
            .filter_map(|detail| detail.summary.total_tokens)
            .sum::<u64>(),
        26
    );
    assert!(!serde_json::to_string(&details).unwrap().contains("secret"));
    assert!(details
        .iter()
        .all(|detail| detail.privacy.as_ref().unwrap().restored_values > 0));
}

#[tokio::test]
async fn privacy_websocket_bad_restoration_is_health_neutral_and_does_not_deliver_success() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let (_directory, context, _) = test_context(
        &format!("http://{}/v1", listener.local_addr().unwrap()),
        "openai_responses",
        false,
    );
    enable_privacy(&context);
    let upstream = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_async(socket).await.unwrap();
        receive_json(&mut socket).await;
        send_json(
            &mut socket,
            json!({"type":"response.created","response":{"id":"resp_ws_bad"}}),
        )
        .await;
        send_json(&mut socket, json!({"type":"response.output_text.delta","response_id":"resp_ws_bad","delta":"__AITB_PRIV_lost"})).await;
        send_json(&mut socket, json!({"type":"response.completed","response":{"id":"resp_ws_bad","status":"completed","usage":{"input_tokens":8,"output_tokens":1}}})).await;
        let _ = socket.next().await;
    });
    let (mut client, gateway) = gateway_connection(context.clone()).await;
    send_json(
        &mut client,
        json!({"type":"response.create","model":"test-model","input":SECRET}),
    )
    .await;
    assert_eq!(receive_json(&mut client).await["type"], "response.created");
    assert_eq!(receive_json(&mut client).await["type"], "response.failed");
    let _ = client.close(None).await;
    gateway.await.unwrap();
    upstream.await.unwrap();
    let details = recorded_details(&context);
    assert_eq!(details.len(), 1);
    assert!(!details[0].summary.success);
    assert_eq!(details[0].summary.total_tokens, Some(9));
    assert_eq!(
        details[0].summary.error_category.as_deref(),
        Some("privacy_restore_failed")
    );
    assert!(details[0].privacy.as_ref().unwrap().failed);
    assert!(context
        .health_items()
        .unwrap_or_default()
        .iter()
        .all(|item| item.failure_score == 0));
}

#[tokio::test]
async fn privacy_http_sse_conversion_keeps_tool_json_usage_and_redacted_logs() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let (_directory, context, _) = test_context(
        &format!("http://{}/v1", listener.local_addr().unwrap()),
        "openai_chat",
        true,
    );
    enable_privacy(&context);
    let upstream = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = super::super::super::http_io::read_http_request(&mut socket, 0)
            .await
            .unwrap();
        let token = token(&request.body);
        let arguments = json!({"value":token}).to_string();
        let mut events = vec![
            json!({"id":"chat_stream_privacy","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"tool_1","type":"function","function":{"name":"write_file","arguments":""}}]},"finish_reason":null}]}),
        ];
        for piece in arguments.as_bytes().chunks(2) {
            events.push(json!({"id":"chat_stream_privacy","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":std::str::from_utf8(piece).unwrap()}}]},"finish_reason":null}]}));
        }
        events.push(json!({"id":"chat_stream_privacy","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":12,"completion_tokens":3}}));
        let mut body = events
            .into_iter()
            .map(|event| format!("data: {event}\n\n"))
            .collect::<String>();
        body.push_str("data: [DONE]\n\n");
        socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
    });
    let (address, gateway) = start_gateway(context.clone(), 1).await;
    let response = crate::http_client::create_client_no_proxy(10)
        .unwrap()
        .post(format!("http://{address}/openai/v1/responses"))
        .json(&json!({"model":"test-model","input":SECRET,"stream":true}))
        .send()
        .await
        .unwrap();
    let body = response.text().await.unwrap();
    assert!(!body.contains("__AITB_PRIV_"));
    let events = body
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    let arguments = events
        .iter()
        .filter(|event| event["type"] == "response.function_call_arguments.delta")
        .filter_map(|event| event["delta"].as_str())
        .collect::<String>();
    assert_eq!(
        serde_json::from_str::<Value>(&arguments).unwrap()["value"],
        SECRET
    );
    assert!(events
        .iter()
        .any(|event| event["type"] == "response.completed"));
    gateway.await.unwrap();
    upstream.await.unwrap();
    let details = recorded_details(&context);
    assert_eq!(
        details[0].summary.stream_outcome,
        Some(GatewayStreamOutcome::Completed)
    );
    assert_eq!(details[0].summary.total_tokens, Some(15));
    assert!(!serde_json::to_string(&details).unwrap().contains("secret"));
    assert!(details[0]
        .response_body
        .as_deref()
        .unwrap()
        .contains("redacted_stream"));
}

#[tokio::test]
async fn privacy_decode_failure_never_logs_unprocessed_request_text() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let (_directory, context, _) = test_context(
        &format!("http://{}/v1", listener.local_addr().unwrap()),
        "openai_responses",
        true,
    );
    enable_privacy(&context);
    let (address, gateway) = start_gateway(context.clone(), 1).await;
    let response = crate::http_client::create_client_no_proxy(10)
        .unwrap()
        .post(format!("http://{address}/openai/v1/responses"))
        .header("Content-Encoding", "invalid-codec")
        .json(&json!({"model":"test-model","input":SECRET}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    gateway.await.unwrap();
    assert!(timeout(Duration::from_millis(30), listener.accept())
        .await
        .is_err());
    let details = recorded_details(&context);
    // Decode errors precede model routing and do not create a model usage/detail record.
    assert!(details.is_empty());
    assert_eq!(context.requests_per_minute(), 0);
}

#[tokio::test]
async fn privacy_websocket_toggle_finishes_active_turn_and_drops_its_late_duplicate() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let (_directory, context, _) = test_context(
        &format!("http://{}/v1", listener.local_addr().unwrap()),
        "openai_responses",
        true,
    );
    enable_privacy(&context);
    let (continue_tx, continue_rx) = tokio::sync::oneshot::channel();
    let upstream = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_async(socket).await.unwrap();
        let first = receive_json(&mut socket).await;
        let placeholder = token(first.to_string().as_bytes());
        send_json(
            &mut socket,
            json!({"type":"response.created","response":{"id":"protected-first"}}),
        )
        .await;
        continue_rx.await.unwrap();
        send_json(&mut socket, json!({"type":"response.output_text.delta","response_id":"protected-first","delta":placeholder})).await;
        send_json(&mut socket, json!({"type":"response.completed","response":{"id":"protected-first","status":"completed"}})).await;
        send_json(&mut socket, json!({"type":"response.completed","response":{"id":"protected-first","status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":placeholder}]}]}})).await;
        let second = receive_json(&mut socket).await;
        assert_eq!(second["input"], SECRET);
        send_json(&mut socket, json!({"type":"response.output_text.delta","response_id":"unprotected-second","delta":"second"})).await;
        send_json(&mut socket, json!({"type":"response.completed","response":{"id":"unprotected-second","status":"completed"}})).await;
        let _ = socket.next().await;
    });
    let (mut client, gateway) = gateway_connection(context.clone()).await;
    send_json(
        &mut client,
        json!({"type":"response.create","model":"test-model","input":SECRET}),
    )
    .await;
    assert_eq!(receive_json(&mut client).await["type"], "response.created");
    context
        .privacy
        .publish(CompiledPolicy::compile(&PrivacySettings::default()).unwrap());
    continue_tx.send(()).unwrap();
    assert_eq!(receive_json(&mut client).await["delta"], SECRET);
    assert_eq!(
        receive_json(&mut client).await["type"],
        "response.completed"
    );
    send_json(
        &mut client,
        json!({"type":"response.create","model":"test-model","input":SECRET}),
    )
    .await;
    assert_eq!(receive_json(&mut client).await["delta"], "second");
    assert_eq!(
        receive_json(&mut client).await["type"],
        "response.completed"
    );
    let _ = client.close(None).await;
    gateway.await.unwrap();
    upstream.await.unwrap();
    let details = recorded_details(&context);
    assert_eq!(details.len(), 2);
    assert!(details.iter().all(|detail| detail.summary.success));
    assert_eq!(
        details
            .iter()
            .filter(|detail| detail.privacy.is_some())
            .count(),
        1
    );
}

#[tokio::test]
async fn privacy_websocket_rejects_unscoped_data_with_no_pending_turns() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let (_directory, context, _) = test_context(
        &format!("http://{}/v1", listener.local_addr().unwrap()),
        "openai_responses",
        false,
    );
    enable_privacy(&context);
    let upstream = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_async(socket).await.unwrap();
        send_json(&mut socket, json!({"type":"response.output_text.delta","response_id":"orphan","delta":"__AITB_PRIV_unknown__"})).await;
        let _ = socket.next().await;
    });
    let (mut client, gateway) = gateway_connection(context.clone()).await;
    let event = receive_json(&mut client).await;
    assert_eq!(event["error"]["code"], "privacy_unscoped_event");
    assert!(!event.to_string().contains("__AITB_PRIV_"));
    let _ = client.close(None).await;
    gateway.await.unwrap();
    upstream.await.unwrap();
}
