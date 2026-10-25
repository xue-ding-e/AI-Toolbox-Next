use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

tokio::task_local! {
    static HTTP_OVERRIDE: (reqwest::Client, String, String);
}

pub(super) fn http_override() -> Option<(reqwest::Client, String, String)> {
    HTTP_OVERRIDE.try_with(Clone::clone).ok()
}

#[derive(Clone)]
struct Request {
    path: String,
    authorization: String,
    account_id: String,
    body: String,
}

struct MockServer {
    url: String,
    requests: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MockServer {
    fn new(mut handler: impl FnMut(&Request) -> (u16, Value) + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let recorded = requests.clone();
        let stopping = stop.clone();
        let thread = std::thread::spawn(move || {
            while !stopping.load(Ordering::SeqCst) {
                let (mut stream, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("mock accept: {error}"),
                };
                stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                let mut data = Vec::new();
                let header_end = loop {
                    let mut byte = [0; 1];
                    if stream.read_exact(&mut byte).is_err() {
                        break None;
                    }
                    data.push(byte[0]);
                    if data.ends_with(b"\r\n\r\n") {
                        break Some(data.len());
                    }
                    assert!(data.len() < 32_768, "bounded synthetic request");
                };
                let Some(header_end) = header_end else { continue };
                let header = String::from_utf8(data[..header_end].to_vec()).unwrap();
                let header_value = |name: &str| header.lines().find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.eq_ignore_ascii_case(name).then(|| value.trim().to_string())
                }).unwrap_or_default();
                let content_length = header_value("content-length").parse::<usize>().unwrap_or(0);
                assert!(content_length < 32_768);
                let mut body = vec![0; content_length];
                stream.read_exact(&mut body).unwrap();
                let request = Request {
                    path: header.lines().next().unwrap().split_whitespace().nth(1).unwrap().to_string(),
                    authorization: header_value("authorization"),
                    account_id: header_value("chatgpt-account-id"),
                    body: String::from_utf8(body).unwrap(),
                };
                recorded.lock().unwrap().push(request.clone());
                let (status, body) = handler(&request);
                let body = body.get("__raw").and_then(Value::as_str).map(str::to_string)
                    .unwrap_or_else(|| body.to_string());
                let response = format!("HTTP/1.1 {status} Mock\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                let _ = stream.write_all(response.as_bytes());
            }
        });
        Self { url, requests, stop, thread: Some(thread) }
    }

    async fn run<T>(&self, task: impl std::future::Future<Output = T>) -> T {
        HTTP_OVERRIDE.scope((
            crate::http_client::create_client_no_proxy(3).unwrap(),
            format!("{}/oauth/token", self.url),
            format!("{}/usage", self.url),
        ), task).await
    }

    fn paths(&self) -> Vec<String> {
        self.requests.lock().unwrap().iter().map(|request| request.path.clone()).collect()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let result = thread.join();
            if !std::thread::panicking() {
                assert!(result.is_ok(), "synthetic HTTP handler failed");
            }
        }
    }
}

fn synthetic_auth(account_id: &str, generation: &str, expires_in: i64) -> Value {
    let payload = serde_json::json!({
        "exp": chrono::Utc::now().timestamp() + expires_in,
        "sub": format!("synthetic-user-{account_id}"),
        "https://api.openai.com/auth": {
            "chatgpt_account_id": account_id,
            "chatgpt_user_id": format!("synthetic-user-{account_id}"),
            "chatgpt_plan_type": "plus"
        },
        "generation": generation
    });
    let token = format!("e30.{}.synthetic", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string()));
    serde_json::json!({
        "auth_mode": "chatgpt",
        "tokens": {
            "access_token": token, "id_token": token,
            "refresh_token": format!("synthetic-refresh-{account_id}-{generation}"),
            "account_id": account_id
        },
        "last_refresh": "2026-01-01T00:00:00Z"
    })
}

fn token(auth: &Value) -> &str {
    auth.pointer("/tokens/access_token").unwrap().as_str().unwrap()
}

fn token_response(auth: &Value) -> Value {
    serde_json::json!({
        "access_token": auth["tokens"]["access_token"],
        "refresh_token": auth["tokens"]["refresh_token"],
        "id_token": auth["tokens"]["id_token"]
    })
}

fn usage_response() -> Value {
    serde_json::json!({"credits": {"has_credits": true, "balance": "42.5"}})
}

async fn fixture(auth: &Value, applied: bool) -> (SqliteDbState, CodexOfficialAccount, tempfile::TempDir) {
    let db = SqliteDbState::in_memory_for_test().unwrap();
    let root = tempfile::Builder::new().prefix("ai-toolbox-issue440-").tempdir().unwrap();
    fs::write(root.path().join(".issue440-synthetic"), b"synthetic fixture only").unwrap();
    db.with_conn(|conn| {
        db_put(conn, DbTable::CodexProvider, "provider-a", &serde_json::json!({
            "name": "Synthetic", "category": "official", "is_applied": applied,
            "settings_config": "{}"
        }))?;
        db_put(conn, DbTable::CodexCommonConfig, "common", &serde_json::json!({
            "root_dir": root.path().to_str().unwrap(), "config": ""
        }))
    }).unwrap();
    fs::write(root.path().join("auth.json"), auth.to_string()).unwrap();
    let mut content = build_account_content_from_auth_snapshot("provider-a", auth, None, None).unwrap();
    content.is_applied = applied;
    let account = save_official_account(&db, &content).await.unwrap();
    (db, account, root)
}

#[tokio::test]
async fn issue440_healthy_account_uses_own_token_without_oauth() {
    let auth = synthetic_auth("a", "healthy", 10 * 86400);
    let (db, account, _root) = fixture(&auth, false).await;
    let expected = format!("Bearer {}", token(&auth));
    let server = MockServer::new(move |request| {
        assert_eq!(request.path, "/usage");
        assert_eq!(request.authorization, expected);
        assert_eq!(request.account_id, "a");
        (200, usage_response())
    });
    let result = server.run(refresh_persisted_account_usage(&db, &account)).await.unwrap();
    assert_eq!(result.credits.unwrap().balance, Some(42.5));
    assert_eq!(server.paths(), ["/usage"]);
}

#[tokio::test]
async fn issue440_expired_token_rotates_and_persists_before_usage() {
    let old = synthetic_auth("a", "expired", -3600);
    let new = synthetic_auth("a", "rotated", 10 * 86400);
    let (db, account, _root) = fixture(&old, false).await;
    let observed_db = db.clone();
    let id = account.id.clone();
    let expected = new.clone();
    let server = MockServer::new(move |request| {
        if request.path == "/oauth/token" {
            assert!(request.body.contains("synthetic-refresh-a-expired"));
            return (200, token_response(&expected));
        }
        if request.authorization != format!("Bearer {}", token(&expected)) {
            return (401, serde_json::json!({"error": "synthetic_old_token"}));
        }
        let stored = observed_db.with_conn(|conn| db_get(conn, DbTable::CodexOfficialAccount, &id)).unwrap().unwrap();
        let snapshot: Value = serde_json::from_str(stored["auth_snapshot"].as_str().unwrap()).unwrap();
        assert_eq!(snapshot["tokens"], expected["tokens"], "rotation must be durable before usage");
        (200, usage_response())
    });
    let result = server.run(refresh_persisted_account_usage(&db, &account)).await.unwrap();
    assert_eq!(result.credits.unwrap().balance, Some(42.5));
    assert_eq!(server.paths(), ["/oauth/token", "/usage"]);
}

#[tokio::test]
async fn issue440_applied_account_adopts_newer_same_identity_runtime() {
    let old = synthetic_auth("a", "expired-live-test", -3600);
    let new = synthetic_auth("a", "runtime-new", 10 * 86400);
    let (db, account, root) = fixture(&old, true).await;
    fs::write(root.path().join("auth.json"), new.to_string()).unwrap();
    let expected = format!("Bearer {}", token(&new));
    let server = MockServer::new(move |request| {
        if request.path == "/usage" && request.authorization == expected {
            (200, usage_response())
        } else {
            (401, serde_json::json!({"error": "synthetic_old_token"}))
        }
    });
    server.run(refresh_persisted_account_usage(&db, &account)).await.unwrap();
    let stored = load_official_account(&db, &account.id).await.unwrap();
    assert_eq!(auth_json_from_snapshot(stored.auth_snapshot.as_deref().unwrap()).unwrap()["tokens"], new["tokens"]);
    assert_eq!(server.paths(), ["/usage"]);
}

#[tokio::test]
async fn issue440_unexpired_401_forces_one_recovery_and_retry() {
    let old = synthetic_auth("a", "rejected", 10 * 86400);
    let new = synthetic_auth("a", "recovered", 11 * 86400);
    let (db, account, _root) = fixture(&old, false).await;
    let server = MockServer::new(move |request| {
        if request.path == "/oauth/token" { return (200, token_response(&new)); }
        if request.authorization == format!("Bearer {}", token(&new)) { (200, usage_response()) }
        else { (401, serde_json::json!({"error": "synthetic_rejected"})) }
    });
    server.run(refresh_persisted_account_usage(&db, &account)).await.unwrap();
    assert_eq!(server.paths(), ["/usage", "/oauth/token", "/usage"]);
}

#[tokio::test]
async fn issue440_rotation_survives_usage_failure() {
    let old = synthetic_auth("a", "before-500", -3600);
    let new = synthetic_auth("a", "after-500", 10 * 86400);
    let (db, account, _root) = fixture(&old, false).await;
    let response = new.clone();
    let server = MockServer::new(move |request| {
        if request.path == "/oauth/token" { (200, token_response(&response)) }
        else { (500, serde_json::json!({"error": "synthetic_500"})) }
    });
    assert!(server.run(refresh_persisted_account_usage(&db, &account)).await.is_err());
    let stored = load_official_account(&db, &account.id).await.unwrap();
    assert_eq!(auth_json_from_snapshot(stored.auth_snapshot.as_deref().unwrap()).unwrap()["tokens"], new["tokens"]);
    assert!(stored.last_error.is_some());
    assert!(stored.credits.is_none());
    assert_eq!(server.paths(), ["/oauth/token", "/usage"]);
}

#[tokio::test]
async fn issue440_non_auth_failures_do_not_exchange_credentials() {
    for status in [429, 500] {
        let auth = synthetic_auth("a", &format!("http-{status}"), 10 * 86400);
        let (db, account, _root) = fixture(&auth, false).await;
        let server = MockServer::new(move |_| (status, serde_json::json!({"error": "synthetic"})));
        assert!(server.run(refresh_persisted_account_usage(&db, &account)).await.is_err());
        assert_eq!(server.paths(), ["/usage"]);
        let stored = load_official_account(&db, &account.id).await.unwrap();
        assert_eq!(stored.auth_snapshot, account.auth_snapshot);
    }
}

#[tokio::test]
async fn issue440_second_401_stops_and_preserves_new_credentials() {
    let old = synthetic_auth("a", "twice-rejected", 10 * 86400);
    let new = synthetic_auth("a", "twice-recovered", 11 * 86400);
    let (db, account, _root) = fixture(&old, false).await;
    let response = new.clone();
    let server = MockServer::new(move |request| {
        if request.path == "/oauth/token" { (200, token_response(&response)) }
        else { (401, serde_json::json!({"error": "synthetic_rejected"})) }
    });
    let error = server.run(refresh_persisted_account_usage(&db, &account)).await.unwrap_err();
    assert!(error.contains("CODEX_REAUTH_REQUIRED"));
    assert_eq!(server.paths(), ["/usage", "/oauth/token", "/usage"]);
    let stored = load_official_account(&db, &account.id).await.unwrap();
    assert_eq!(auth_json_from_snapshot(stored.auth_snapshot.as_deref().unwrap()).unwrap()["tokens"], new["tokens"]);
}

#[tokio::test]
async fn issue440_missing_or_revoked_refresh_token_is_actionable_and_redacted() {
    for code in ["missing", "invalid_grant", "refresh_token_reused"] {
        let mut auth = synthetic_auth("a", &format!("revoked-{code}"), -3600);
        if code == "missing" { auth["tokens"]["refresh_token"] = Value::Null; }
        let (db, account, _root) = fixture(&auth, false).await;
        let server = MockServer::new(move |_| (400, serde_json::json!({
            "error": code, "description": "synthetic-secret-must-not-escape"
        })));
        let error = server.run(refresh_persisted_account_usage(&db, &account)).await.unwrap_err();
        assert!(error.contains("CODEX_REAUTH_REQUIRED"));
        assert!(!error.contains("synthetic-secret-must-not-escape"));
        assert_eq!(server.paths().len(), usize::from(code != "missing"));
        assert_eq!(load_official_account(&db, &account.id).await.unwrap().auth_snapshot, account.auth_snapshot);
    }
}

#[tokio::test]
async fn issue440_same_access_after_forced_exchange_is_not_replayed() {
    let auth = synthetic_auth("a", "unchanged-access", 10 * 86400);
    let (db, account, _root) = fixture(&auth, false).await;
    let mut refreshed = auth.clone();
    refreshed["tokens"]["refresh_token"] = Value::String("synthetic-rotated-even-with-same-access".to_string());
    let response = refreshed.clone();
    let server = MockServer::new(move |request| {
        if request.path == "/oauth/token" { (200, token_response(&response)) }
        else { (401, serde_json::json!({})) }
    });
    assert!(server.run(refresh_persisted_account_usage(&db, &account)).await.unwrap_err().contains("CODEX_REAUTH_REQUIRED"));
    assert_eq!(server.paths(), ["/usage", "/oauth/token"]);
    let stored = load_official_account(&db, &account.id).await.unwrap();
    assert_eq!(auth_json_from_snapshot(stored.auth_snapshot.as_deref().unwrap()).unwrap()["tokens"], refreshed["tokens"]);
}

#[tokio::test]
async fn issue440_other_runtime_account_and_inactive_target_remain_untouched() {
    for applied in [false, true] {
        let auth = synthetic_auth("a", &format!("isolated-{applied}"), 10 * 86400);
        let other = synthetic_auth("b", &format!("live-other-{applied}"), 15 * 86400);
        let (db, account, root) = fixture(&auth, applied).await;
        fs::write(root.path().join("auth.json"), other.to_string()).unwrap();
        let before = fs::read(root.path().join("auth.json")).unwrap();
        let expected = format!("Bearer {}", token(&auth));
        let server = MockServer::new(move |request| {
            assert_eq!(request.authorization, expected);
            assert_eq!(request.account_id, "a");
            (200, usage_response())
        });
        server.run(refresh_persisted_account_usage(&db, &account)).await.unwrap();
        assert_eq!(fs::read(root.path().join("auth.json")).unwrap(), before);
        let stored = load_official_account(&db, &account.id).await.unwrap();
        assert_eq!(stored.is_applied, applied);
        assert_eq!(stored.auth_snapshot, account.auth_snapshot);
    }
}

#[tokio::test]
async fn issue440_same_workspace_different_user_and_inconsistent_claims_are_not_adopted() {
    for mode in ["different-user", "conflicting-id", "access-conflict", "not-newer"] {
        let auth = synthetic_auth("a", &format!("identity-{mode}"), 10 * 86400);
        let mut other = synthetic_auth("a", &format!("identity-other-{mode}"), 15 * 86400);
        if mode == "different-user" {
            let encoded = other["tokens"]["id_token"].as_str().unwrap().split('.').nth(1).unwrap();
            let mut payload: Value = serde_json::from_slice(&base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(encoded).unwrap()).unwrap();
            payload["sub"] = Value::String("synthetic-other-user".to_string());
            payload["https://api.openai.com/auth"]["chatgpt_user_id"] = Value::String("synthetic-other-user".to_string());
            other["tokens"]["id_token"] = Value::String(format!("e30.{}.synthetic", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string())));
            other["tokens"]["access_token"] = other["tokens"]["id_token"].clone();
        } else if mode == "conflicting-id" {
            other["tokens"]["account_id"] = Value::String("b".to_string());
        } else if mode == "access-conflict" {
            other["tokens"]["access_token"] = synthetic_auth("b", "conflicting-access", 15 * 86400)["tokens"]["access_token"].clone();
        } else {
            other = synthetic_auth("a", "not-newer", 5 * 86400);
        }
        let (db, account, root) = fixture(&auth, true).await;
        fs::write(root.path().join("auth.json"), other.to_string()).unwrap();
        let before = fs::read(root.path().join("auth.json")).unwrap();
        let expected = format!("Bearer {}", token(&auth));
        let server = MockServer::new(move |request| {
            assert_eq!(request.authorization, expected);
            (200, usage_response())
        });
        server.run(refresh_persisted_account_usage(&db, &account)).await.unwrap();
        assert_eq!(fs::read(root.path().join("auth.json")).unwrap(), before);
        assert_eq!(load_official_account(&db, &account.id).await.unwrap().auth_snapshot, account.auth_snapshot);
    }
}

#[tokio::test]
async fn issue440_malformed_usage_and_timeout_keep_network_error_semantics() {
    for timeout in [false, true] {
        let auth = synthetic_auth("a", &format!("network-{timeout}"), 10 * 86400);
        let (db, account, _root) = fixture(&auth, false).await;
        let server = MockServer::new(move |_| {
            if timeout { std::thread::sleep(Duration::from_secs(4)); }
            (200, serde_json::json!({"__raw": "not valid json"}))
        });
        let error = server.run(refresh_persisted_account_usage(&db, &account)).await.unwrap_err();
        assert!(!error.contains("CODEX_REAUTH_REQUIRED"));
        assert_eq!(server.paths(), ["/usage"]);
    }
}

#[tokio::test]
async fn issue440_corrupt_snapshots_never_send_empty_authorization() {
    for snapshot in [None, Some("invalid-json"), Some("{}"), Some(r#"{"tokens":{"access_token":" "}}"#)] {
        let auth = synthetic_auth("a", "corrupt-fixture", 10 * 86400);
        let (db, account, _root) = fixture(&auth, false).await;
        db.with_conn(|conn| db_patch_fields(conn, DbTable::CodexOfficialAccount, &account.id,
            &[("auth_snapshot", snapshot.map(|value| Value::String(value.to_string())).unwrap_or(Value::Null))])).unwrap();
        let server = MockServer::new(move |_| (200, usage_response()));
        assert!(server.run(refresh_persisted_account_usage(&db, &account)).await.is_err());
        assert!(server.paths().is_empty());
    }
}

#[tokio::test]
async fn issue440_runtime_refresh_only_rotation_is_persisted() {
    let old = synthetic_auth("a", "refresh-only", 10 * 86400);
    let mut new = old.clone();
    new["tokens"]["refresh_token"] = Value::String("synthetic-new-refresh-only".to_string());
    new["last_refresh"] = Value::String("2026-02-01T00:00:00Z".to_string());
    let (db, account, root) = fixture(&old, true).await;
    fs::write(root.path().join("auth.json"), new.to_string()).unwrap();
    let server = MockServer::new(move |_| (200, usage_response()));
    server.run(refresh_persisted_account_usage(&db, &account)).await.unwrap();
    let stored = load_official_account(&db, &account.id).await.unwrap();
    assert_eq!(auth_json_from_snapshot(stored.auth_snapshot.as_deref().unwrap()).unwrap()["tokens"], new["tokens"]);
    assert_eq!(server.paths(), ["/usage"]);
}

#[tokio::test]
async fn issue440_gateway_runtime_is_not_adopted_or_rewritten() {
    let old = synthetic_auth("a", "gateway-db", -3600);
    let new = synthetic_auth("a", "gateway-refresh", 10 * 86400);
    let mut gateway = synthetic_auth("a", "gateway-runtime", 15 * 86400);
    gateway["auth_mode"] = Value::String("apikey".to_string());
    gateway["OPENAI_API_KEY"] = Value::String("synthetic-gateway-key".to_string());
    let (db, account, root) = fixture(&old, true).await;
    fs::write(root.path().join("auth.json"), gateway.to_string()).unwrap();
    let before = fs::read(root.path().join("auth.json")).unwrap();
    let server = MockServer::new(move |request| {
        if request.path == "/oauth/token" { return (200, token_response(&new)) }
        assert_eq!(request.authorization, format!("Bearer {}", token(&new)));
        (200, usage_response())
    });
    server.run(refresh_persisted_account_usage(&db, &account)).await.unwrap();
    assert_eq!(fs::read(root.path().join("auth.json")).unwrap(), before);
    assert_eq!(server.paths(), ["/oauth/token", "/usage"]);
}

#[tokio::test]
async fn issue440_usage_context_comes_from_consistent_credentials_not_old_metadata() {
    let auth = synthetic_auth("a", "old-metadata", 10 * 86400);
    let (db, account, _root) = fixture(&auth, false).await;
    db.with_conn(|conn| db_patch_fields(conn, DbTable::CodexOfficialAccount, &account.id,
        &[("account_id", Value::String("stale-metadata".to_string())), ("plan_type", Value::String("pro".to_string()))])).unwrap();
    let expected = format!("Bearer {}", token(&auth));
    let server = MockServer::new(move |request| {
        assert_eq!(request.authorization, expected);
        assert_eq!(request.account_id, "a");
        (200, serde_json::json!({"rate_limit": {"primary_window": {"used_percent": 25, "limit_window_seconds": 18000}}}))
    });
    let result = server.run(refresh_persisted_account_usage(&db, &account)).await.unwrap();
    assert_eq!(result.account_id.as_deref(), Some("a"));
    assert_eq!(result.plan_type.as_deref(), Some("plus"));
    assert_eq!(result.limit_5h_text.as_deref(), Some("75%"));
}

#[tokio::test]
async fn issue440_deleted_or_replaced_snapshot_stops_before_usage_and_live_write() {
    for delete in [false, true] {
        let old = synthetic_auth("a", &format!("cas-old-{delete}"), -3600);
        let new = synthetic_auth("a", &format!("cas-new-{delete}"), 10 * 86400);
        let replacement = synthetic_auth("a", &format!("cas-replacement-{delete}"), 15 * 86400);
        let (db, account, root) = fixture(&old, true).await;
        let observed_db = db.clone();
        let id = account.id.clone();
        let replacement_snapshot = replacement.to_string();
        let server = MockServer::new(move |request| {
            assert_eq!(request.path, "/oauth/token");
            observed_db.with_conn(|conn| {
                if delete { db_delete(conn, DbTable::CodexOfficialAccount, &id).map(|_| ()) }
                else { db_patch_fields(conn, DbTable::CodexOfficialAccount, &id,
                    &[("auth_snapshot", Value::String(replacement_snapshot.clone()))]).map(|_| ()) }
            }).unwrap();
            (200, token_response(&new))
        });
        assert!(server.run(refresh_persisted_account_usage(&db, &account)).await.is_err());
        assert_eq!(server.paths(), ["/oauth/token"]);
        assert_eq!(fs::read_to_string(root.path().join("auth.json")).unwrap(), old.to_string());
        let remaining = db.with_conn(|conn| db_get(conn, DbTable::CodexOfficialAccount, &account.id)).unwrap();
        if delete { assert!(remaining.is_none()) }
        else { assert_eq!(remaining.unwrap()["auth_snapshot"], replacement.to_string()) }
    }
}

#[tokio::test]
async fn issue440_concurrent_refreshes_consume_rotated_token_once() {
    let old = synthetic_auth("a", "concurrent-old", -3600);
    let new = synthetic_auth("a", "concurrent-new", 10 * 86400);
    let (db, account, _root) = fixture(&old, false).await;
    let server = MockServer::new(move |request| {
        if request.path == "/oauth/token" { return (200, token_response(&new)) }
        assert_eq!(request.authorization, format!("Bearer {}", token(&new)));
        (200, usage_response())
    });
    // join! stays in the explicitly scoped task; spawned tasks must install
    // their own HTTP scope rather than inheriting or falling back to the network.
    let (first, second) = server.run(async {
        tokio::join!(refresh_persisted_account_usage(&db, &account), refresh_persisted_account_usage(&db, &account))
    }).await;
    first.unwrap(); second.unwrap();
    assert_eq!(server.paths(), ["/oauth/token", "/usage", "/usage"]);
}

#[tokio::test]
async fn issue440_oauth_wait_root_or_provider_switch_does_not_overwrite_new_runtime() {
    for change_root in [false, true] {
        let old = synthetic_auth("a", &format!("switch-old-{change_root}"), -3600);
        let new = synthetic_auth("a", &format!("switch-new-{change_root}"), 10 * 86400);
        let other = synthetic_auth("b", &format!("switch-other-{change_root}"), 15 * 86400);
        let (db, account, root) = fixture(&old, true).await;
        let alternate = tempfile::Builder::new().prefix("ai-toolbox-issue440-").tempdir().unwrap();
        fs::write(alternate.path().join(".issue440-synthetic"), b"synthetic fixture only").unwrap();
        fs::write(alternate.path().join("auth.json"), other.to_string()).unwrap();
        let observed_db = db.clone();
        let id = account.id.clone();
        let live_path = root.path().join("auth.json");
        let alternate_root = alternate.path().to_str().unwrap().to_string();
        let other_for_server = other.clone();
        let server = MockServer::new(move |request| {
            if request.path == "/oauth/token" {
                observed_db.with_conn(|conn| {
                    if change_root {
                        db_patch_fields(conn, DbTable::CodexCommonConfig, "common", &[("root_dir", Value::String(alternate_root.clone()))])?;
                    } else {
                        db_patch_fields(conn, DbTable::CodexProvider, "provider-a", &[("is_applied", Value::Bool(false))])?;
                        db_patch_fields(conn, DbTable::CodexOfficialAccount, &id, &[("is_applied", Value::Bool(false))])?;
                    }
                    Ok(())
                }).unwrap();
                if !change_root { fs::write(&live_path, other_for_server.to_string()).unwrap(); }
                return (200, token_response(&new));
            }
            assert_eq!(request.authorization, format!("Bearer {}", token(&new)));
            (200, usage_response())
        });
        server.run(refresh_persisted_account_usage(&db, &account)).await.unwrap();
        assert_eq!(fs::read_to_string(alternate.path().join("auth.json")).unwrap(), other.to_string());
        assert_eq!(fs::read_to_string(root.path().join("auth.json")).unwrap(), if change_root { old.to_string() } else { other.to_string() });
    }
}

#[tokio::test]
async fn issue440_local_refresh_rotates_without_creating_a_saved_account() {
    let old = synthetic_auth("a", "local-old", -3600);
    let new = synthetic_auth("a", "local-new", 10 * 86400);
    let (db, account, root) = fixture(&old, false).await;
    db.with_conn(|conn| db_delete(conn, DbTable::CodexOfficialAccount, &account.id)).unwrap();
    let expected = new.clone();
    let server = MockServer::new(move |request| {
        if request.path == "/oauth/token" { return (200, token_response(&expected)) }
        assert_eq!(request.authorization, format!("Bearer {}", token(&expected)));
        (200, usage_response())
    });
    let mut wrote = false;
    let result = server.run(usage_refresh::refresh_local(&db, "provider-a", &mut wrote)).await.unwrap();
    assert!(wrote);
    assert!(result.is_virtual);
    assert_eq!(result.id, LOCAL_OFFICIAL_ACCOUNT_ID);
    assert_eq!(result.credits.unwrap().balance, Some(42.5));
    assert!(db.with_conn(|conn| db_list(conn, DbTable::CodexOfficialAccount, None)).unwrap().is_empty());
    let live: Value = serde_json::from_str(&fs::read_to_string(root.path().join("auth.json")).unwrap()).unwrap();
    assert_eq!(live["tokens"], new["tokens"]);
}

#[tokio::test]
async fn issue440_local_usage_rejects_late_success_after_account_switch() {
    let old = synthetic_auth("a", "local-switch-old", 10 * 86400);
    let other = synthetic_auth("b", "local-switch-other", 15 * 86400);
    let (db, account, root) = fixture(&old, false).await;
    db.with_conn(|conn| db_delete(conn, DbTable::CodexOfficialAccount, &account.id)).unwrap();
    let live_path = root.path().join("auth.json");
    let expected = other.clone();
    let server = MockServer::new(move |_| {
        fs::write(&live_path, expected.to_string()).unwrap();
        (200, usage_response())
    });
    let error = server.run(usage_refresh::refresh_local(&db, "provider-a", &mut false)).await.unwrap_err();
    assert!(error.contains("changed"));
    assert_eq!(server.paths(), ["/usage"]);
    assert_eq!(fs::read_to_string(root.path().join("auth.json")).unwrap(), other.to_string());
}

#[tokio::test]
async fn issue440_missing_mock_scope_fails_closed() {
    let auth = synthetic_auth("a", "fail-closed", 10 * 86400);
    let (db, account, _root) = fixture(&auth, false).await;
    let error = refresh_persisted_account_usage(&db, &account).await.unwrap_err();
    assert!(error.contains("Synthetic Codex HTTP scope"));
}

#[tokio::test]
async fn issue440_forced_recovery_does_not_reuse_cache_for_rejected_access() {
    let old = synthetic_auth("a", "cache-old", -3600);
    let first = synthetic_auth("a", "cache-first", 10 * 86400);
    let second = synthetic_auth("a", "cache-second", 11 * 86400);
    let (db, account, _root) = fixture(&old, false).await;
    let mut exchanges = 0;
    let server = MockServer::new(move |request| {
        if request.path == "/oauth/token" {
            exchanges += 1;
            if exchanges == 1 {
                let mut response = token_response(&first);
                response.as_object_mut().unwrap().remove("refresh_token");
                return (200, response);
            }
            return (200, token_response(&second));
        }
        if request.authorization == format!("Bearer {}", token(&second)) { (200, usage_response()) }
        else { (401, serde_json::json!({})) }
    });
    server.run(refresh_persisted_account_usage(&db, &account)).await.unwrap();
    assert_eq!(server.paths(), ["/oauth/token", "/usage", "/oauth/token", "/usage"]);
}

#[tokio::test]
async fn issue440_real_background_and_provider_apply_complete_while_usage_is_waiting() {
    let old = synthetic_auth("a", "real-owners-old", -3600);
    let new = synthetic_auth("a", "real-owners-new", 10 * 86400);
    let (db, account, root) = fixture(&old, true).await;
    db.with_conn(|conn| db_put(conn, DbTable::CodexProvider, "provider-b", &serde_json::json!({
        "name": "Synthetic B", "category": "custom", "is_applied": false,
        "settings_config": serde_json::json!({
            "auth": {"OPENAI_API_KEY": "synthetic-provider-b-key"},
            "config": "model_provider = \"synthetic-b\"\n[model_providers.synthetic-b]\nname = \"Synthetic B\"\nbase_url = \"https://example.invalid/v1\"\nwire_api = \"responses\"\n"
        }).to_string()
    }))).unwrap();
    let app = tauri::test::mock_app();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let mut started_tx = Some(started_tx);
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let server = MockServer::new(move |request| {
        if request.path == "/oauth/token" { return (200, token_response(&new)) }
        started_tx.take().unwrap().send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        (200, usage_response())
    });
    let (refreshed, ()) = server.run(async {
        tokio::join!(refresh_persisted_account_usage(&db, &account), async {
            started_rx.await.unwrap();
            tokio::time::timeout(Duration::from_secs(2), async {
                refresh_applied_codex_accounts_if_needed(&db, app.handle()).await.unwrap();
                super::super::commands::apply_config_internal(&db, app.handle(), "provider-b", false).await.unwrap();
            }).await.expect("usage HTTP must not hold the operation lock or deadlock real owners");
            release_tx.send(()).unwrap();
        })
    }).await;
    refreshed.unwrap();
    assert_eq!(server.paths(), ["/oauth/token", "/usage"]);
    assert!(query_provider(&db, "provider-b").await.unwrap().is_applied);
    assert!(!query_provider(&db, "provider-a").await.unwrap().is_applied);
    let config = fs::read_to_string(root.path().join("config.toml")).unwrap();
    assert!(config.contains("synthetic-b"));
    let live: Value = serde_json::from_str(&fs::read_to_string(root.path().join("auth.json")).unwrap()).unwrap();
    assert_eq!(live["OPENAI_API_KEY"], "synthetic-provider-b-key");
    assert_eq!(live["auth_mode"], "apikey");
    // OAuth preparation only patched its own credential fields; the late usage
    // success did not re-apply provider A or replace B's runtime projection.
    assert_eq!(load_official_account(&db, &account.id).await.unwrap().credits.unwrap().balance, Some(42.5));
}

#[tokio::test]
async fn issue440_cancelled_usage_releases_locks_and_cannot_commit_late_success() {
    let auth = synthetic_auth("a", "cancelled-usage", 10 * 86400);
    let (db, account, _root) = fixture(&auth, false).await;
    let (started_tx, mut started_rx) = tokio::sync::oneshot::channel();
    let mut started_tx = Some(started_tx);
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let server = MockServer::new(move |_| {
        if let Some(started) = started_tx.take() {
            started.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            (200, serde_json::json!({"credits": {"balance": "999"}}))
        } else { (200, usage_response()) }
    });
    server.run(async {
        let mut pending = Box::pin(refresh_persisted_account_usage(&db, &account));
        tokio::select! {
            _ = &mut started_rx => {}
            _ = &mut pending => panic!("synthetic usage must still be pending"),
        }
        drop(pending);
        let operation = tokio::time::timeout(Duration::from_secs(1), ACCOUNT_OPERATION_LOCK.lock()).await.unwrap();
        drop(operation);
        release_tx.send(()).unwrap();
        let result = tokio::time::timeout(Duration::from_secs(3), refresh_persisted_account_usage(&db, &account)).await.unwrap().unwrap();
        assert_eq!(result.credits.unwrap().balance, Some(42.5));
    }).await;
    assert_eq!(load_official_account(&db, &account.id).await.unwrap().credits.unwrap().balance, Some(42.5));
    assert_eq!(server.paths(), ["/usage", "/usage"]);
}
