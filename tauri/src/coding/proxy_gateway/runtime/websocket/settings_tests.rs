use super::*;
use crate::coding::proxy_gateway::{runtime::ProxyGatewayManager, settings};

#[tokio::test]
async fn websocket_is_disabled_by_default_before_provider_loading() {
    let context = GatewayRuntimeContext::new(ProxyGatewaySettings::default(), None, None);
    let (address, gateway) = start_gateway(context, 1).await;
    let response = rejected_upgrade(address, "/openai/v1/responses").await;
    assert_eq!(response.status(), 426);
    let body = String::from_utf8(response.body().as_ref().unwrap().clone()).unwrap();
    assert!(body.contains("disabled in gateway settings"));
    gateway.await.unwrap();
}

#[tokio::test]
async fn saved_websocket_toggle_updates_live_gateway_without_restart() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let (_directory, context, _) = test_context(
        &format!("http://{}/v1", listener.local_addr().unwrap()),
        "openai_responses",
        true,
    );
    let db = context.db.as_ref().unwrap();
    let mut configured = context.settings_snapshot();
    configured.codex_websocket_enabled = false;
    let port_reservation = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    configured.listen_port = port_reservation.local_addr().unwrap().port();
    configured.port_auto_select = true;
    drop(port_reservation);
    let saved = settings::save_settings(db, configured).unwrap();
    let mut manager = ProxyGatewayManager::default();
    let status = manager
        .start_with_context(saved, db.clone(), context.paths.as_ref().unwrap().clone())
        .unwrap();
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], status.listen_port.unwrap()));
    assert_eq!(
        rejected_upgrade(address, "/openai/v1/responses")
            .await
            .status(),
        426
    );
    assert!(timeout(Duration::from_millis(30), listener.accept())
        .await
        .is_err());

    let mut enabled = settings::load_settings_from_sqlite_state(db).unwrap();
    enabled.codex_websocket_enabled = true;
    let saved = settings::save_settings(db, enabled).unwrap();
    manager.update_runtime_settings(saved).unwrap();
    let upstream = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_async(socket).await.unwrap();
        receive_json(&mut socket).await;
        send_json(&mut socket, json!({"type":"response.completed","response":{"id":"toggle-response","usage":{"input_tokens":10,"output_tokens":2}}})).await;
        let _ = timeout(Duration::from_secs(5), socket.next())
            .await
            .unwrap();
    });
    let socket = TcpStream::connect(address).await.unwrap();
    let (mut client, _) =
        tokio_tungstenite::client_async(format!("ws://{address}/openai/v1/responses"), socket)
            .await
            .unwrap();
    send_json(
        &mut client,
        json!({"type":"response.create","model":"test-model","input":"hello"}),
    )
    .await;
    assert_eq!(
        receive_json(&mut client).await["type"],
        "response.completed"
    );

    let mut disabled = settings::load_settings_from_sqlite_state(db).unwrap();
    disabled.codex_websocket_enabled = false;
    manager
        .update_runtime_settings(settings::save_settings(db, disabled).unwrap())
        .unwrap();
    assert!(matches!(
        timeout(Duration::from_secs(5), client.next())
            .await
            .unwrap(),
        Some(Ok(Message::Close(_)))
    ));
    assert_eq!(
        rejected_upgrade(address, "/openai/v1/responses")
            .await
            .status(),
        426
    );
    assert!(
        !settings::load_settings_from_sqlite_state(db)
            .unwrap()
            .codex_websocket_enabled
    );
    upstream.await.unwrap();
    let summary = usage_stats::usage_summary(db, None, None, None, true).unwrap();
    assert_eq!(summary.total_requests, 1);
    assert_eq!(summary.total_tokens, 12);
    assert_eq!(summary.success_rate, 100.0);
    assert_eq!(manager.status().requests_per_minute, 1);
    manager.stop().unwrap();
}

#[tokio::test]
async fn disabling_idle_connection_does_not_forward_next_response_create() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let (_directory, context, _) = test_context(
        &format!("http://{}/v1", listener.local_addr().unwrap()),
        "openai_responses",
        true,
    );
    let upstream = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_async(socket).await.unwrap();
        assert!(matches!(
            timeout(Duration::from_secs(5), socket.next())
                .await
                .unwrap(),
            Some(Ok(Message::Close(_)))
        ));
    });
    let (mut client, gateway) = gateway_connection(context.clone()).await;
    // Let the relay enter its idle read before changing the setting.
    tokio::time::sleep(Duration::from_millis(30)).await;
    context.settings.write().unwrap().codex_websocket_enabled = false;
    send_json(
        &mut client,
        json!({"type":"response.create","model":"test-model","input":"hello"}),
    )
    .await;
    assert!(matches!(
        timeout(Duration::from_secs(5), client.next())
            .await
            .unwrap(),
        Some(Ok(Message::Close(_)))
    ));
    gateway.await.unwrap();
    upstream.await.unwrap();
    assert!(recorded_details(&context).is_empty());
    assert_eq!(context.requests_per_minute(), 0);
}

#[tokio::test]
async fn disabling_websocket_keeps_in_flight_usage_and_closes_after_terminal() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let (_directory, context, _) = test_context(
        &format!("http://{}/v1", listener.local_addr().unwrap()),
        "openai_responses",
        true,
    );
    let (release_terminal, terminal_ready) = tokio::sync::oneshot::channel();
    let upstream = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_async(socket).await.unwrap();
        receive_json(&mut socket).await;
        send_json(
            &mut socket,
            json!({"type":"response.created","response":{"id":"draining-response"}}),
        )
        .await;
        terminal_ready.await.unwrap();
        send_json(&mut socket, json!({"type":"response.completed","response":{"id":"draining-response","usage":{"input_tokens":20,"output_tokens":3}}})).await;
        let _ = timeout(Duration::from_secs(5), socket.next())
            .await
            .unwrap();
    });
    let (mut client, gateway) = gateway_connection(context.clone()).await;
    send_json(
        &mut client,
        json!({"type":"response.create","model":"test-model","input":"hello"}),
    )
    .await;
    receive_json(&mut client).await;
    context.settings.write().unwrap().codex_websocket_enabled = false;
    let (address, rejected_gateway) = start_gateway(context.clone(), 1).await;
    assert_eq!(
        rejected_upgrade(address, "/openai/v1/responses")
            .await
            .status(),
        426
    );
    rejected_gateway.await.unwrap();
    assert!(timeout(Duration::from_millis(150), client.next())
        .await
        .is_err());
    release_terminal.send(()).unwrap();
    assert_eq!(
        receive_json(&mut client).await["type"],
        "response.completed"
    );
    assert!(matches!(
        timeout(Duration::from_secs(5), client.next())
            .await
            .unwrap(),
        Some(Ok(Message::Close(_)))
    ));
    gateway.await.unwrap();
    upstream.await.unwrap();
    let summary =
        usage_stats::usage_summary(context.db.as_ref().unwrap(), None, None, None, true).unwrap();
    assert_eq!(summary.total_requests, 1);
    assert_eq!(summary.total_tokens, 23);
    assert_eq!(summary.success_rate, 100.0);
}

#[tokio::test]
async fn disabling_during_upstream_handshake_returns_426_before_downstream_upgrade() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let (_directory, context, _) = test_context(
        &format!("http://{}/v1", listener.local_addr().unwrap()),
        "openai_responses",
        true,
    );
    let changing_context = context.clone();
    let upstream = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_hdr_async(
            socket,
            move |_: &server::Request, response: server::Response| {
                changing_context
                    .settings
                    .write()
                    .unwrap()
                    .codex_websocket_enabled = false;
                Ok(response)
            },
        )
        .await
        .unwrap();
        let _ = timeout(Duration::from_secs(5), socket.next())
            .await
            .unwrap();
    });
    let (address, gateway) = start_gateway(context.clone(), 1).await;
    assert_eq!(
        rejected_upgrade(address, "/openai/v1/responses")
            .await
            .status(),
        426
    );
    gateway.await.unwrap();
    upstream.await.unwrap();
    let detail = recorded_details(&context).pop().unwrap();
    assert_eq!(detail.summary.status_code, Some(426));
    assert_eq!(
        detail.summary.request_kind,
        GatewayRequestKind::WebsocketHandshake
    );
    assert_eq!(
        detail.websocket.unwrap().upstream_handshake_status,
        Some(101)
    );
    assert_eq!(context.requests_per_minute(), 0);
}
