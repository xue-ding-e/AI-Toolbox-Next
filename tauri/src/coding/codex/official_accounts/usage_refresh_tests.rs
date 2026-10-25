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
                let body = body.to_string();
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
