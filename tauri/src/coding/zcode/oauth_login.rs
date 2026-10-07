//! The browser login flow behind `zcode login`.
//!
//! ZCode's CLI does not own a callback URL. It asks the server to start a flow,
//! opens the returned `authorize_url`, and then *polls* the same server until
//! the flow turns ready — the browser hand-off and the terminal are joined by
//! the flow id, not by a redirect back into the process. AI Toolbox drives the
//! poll half only. That matters: the alternative completion path, the
//! `zcode://oauth/callback` deep link, belongs to the ZCode desktop app, and
//! claiming that URL scheme would break it.
//!
//! The flow is a three-step conversation with `https://zcode.z.ai`:
//!
//! 1. `POST /api/v1/oauth/cli/init` — with the poll token as the bearer —
//!    answers with `authorize_url`, `flow_id`, `expires_at` and a poll cadence.
//! 2. The user finishes login in their browser.
//! 3. `GET /api/v1/oauth/cli/poll/{flow_id}` reports `pending`, `failed` or
//!    `ready`, and the ready body carries the tokens to persist.
//!
//! There is no PKCE and no client-side state check: the server mints the state
//! and pairs it with the flow id, and the poll token is what authorizes reading
//! the result.

use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};

use crate::db::SqliteDbState;

const OAUTH_BASE_URL: &str = "https://zcode.z.ai/api/v1";
const ZCODE_ORIGIN: &str = "https://zcode.z.ai";
const ZCODE_CHANNEL: &str = "stable";

/// Sent as `X-ZCode-App-Version`. This is attribution only — the flow is
/// authorized by the poll token and keyed on the flow id, so a stale value
/// cannot break a login. `ZCODE_APP_VERSION` overrides it.
const CLIENT_APP_VERSION: &str = "3.11.2";

/// The provider ids `zcode login` accepts, in the order its help lists them.
pub const ZCODE_LOGIN_PROVIDERS: &[(&str, &str)] = &[
    ("zai", "z.ai"),
    ("bigmodel", "BigModel"),
];

pub fn is_supported_provider(provider: &str) -> bool {
    ZCODE_LOGIN_PROVIDERS
        .iter()
        .any(|(id, _)| *id == provider)
}

fn node_platform() -> &'static str {
    match std::env::consts::OS {
        "windows" => "win32",
        "macos" => "darwin",
        other => other,
    }
}

/// `<os>-<arch>`, matching Node's `process.platform()` + `process.arch`.
fn client_platform() -> String {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    format!("{}-{}", node_platform(), arch)
}

fn app_version() -> String {
    std::env::var("ZCODE_APP_VERSION")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| CLIENT_APP_VERSION.to_string())
}

/// A fresh poll token: 32 random bytes, hex encoded.
///
/// This is the bearer of the whole flow, so it is derived from a UUID v4 —
/// already cryptographically random — rather than a seeded generator.
fn new_poll_token() -> String {
    let mut bytes = [0u8; 32];
    for chunk in bytes.chunks_mut(16) {
        let uuid = uuid::Uuid::new_v4();
        chunk.copy_from_slice(&uuid.as_bytes()[..chunk.len()]);
    }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn request_headers(poll_token: &str, device_mid: Option<&str>) -> Vec<(String, String)> {
    let mut headers = vec![
        ("Authorization".to_string(), format!("Bearer {poll_token}")),
        ("User-Agent".to_string(), format!("ZCode/{}", app_version())),
        ("HTTP-Referer".to_string(), ZCODE_ORIGIN.to_string()),
        ("X-Title".to_string(), "Z Code@electron".to_string()),
        ("X-ZCode-App-Version".to_string(), app_version()),
        ("X-Platform".to_string(), client_platform()),
        ("X-Release-Channel".to_string(), ZCODE_CHANNEL.to_string()),
        ("X-Client-Language".to_string(), "zh-CN".to_string()),
        ("X-Os-Category".to_string(), std::env::consts::OS.to_string()),
        ("x-request-id".to_string(), uuid::Uuid::new_v4().to_string()),
    ];
    if let Some(mid) = device_mid {
        headers.push(("X-Device-Mid".to_string(), mid.to_string()));
    }
    headers
}

/// A started flow, as handed back by `init`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZcodeOauthFlow {
    pub provider: String,
    pub authorize_url: String,
    pub poll_url: String,
    pub poll_token: String,
    pub interval_seconds: u64,
    /// Seconds of remaining validity, measured when the flow was started.
    pub lifetime_seconds: u64,
}

/// The credentials a completed flow yields.
#[derive(Debug, Clone)]
pub struct ZcodeLoginMaterial {
    /// The platform JWT, stored as `zcodejwttoken`.
    pub jwt: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    /// `oauth:<provider>:user_info` payload.
    pub user_info: Option<Value>,
}

fn api_code(body: &Value) -> Option<i64> {
    body.get("code").and_then(|code| code.as_i64())
}

fn api_message(body: &Value) -> String {
    body.get("msg")
        .and_then(|msg| msg.as_str())
        .unwrap_or_default()
        .to_string()
}

fn trimmed_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Starts a login flow.
pub async fn start_flow(
    db: &SqliteDbState,
    provider: &str,
    device_mid: Option<&str>,
) -> Result<ZcodeOauthFlow, String> {
    if !is_supported_provider(provider) {
        return Err(format!("Unsupported ZCode login provider: {provider}"));
    }

    let client = crate::http_client::client_with_timeout(db, 20).await?;
    let poll_token = new_poll_token();
    let mut request = client
        .post(format!("{OAUTH_BASE_URL}/oauth/cli/init"))
        .json(&json!({ "provider": provider }));
    for (name, value) in request_headers(&poll_token, device_mid) {
        request = request.header(name, value);
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("Failed to start ZCode login: {error}"))?;
    let status = response.status();
    let body: Value = response
        .json()
        .await
        .map_err(|error| format!("ZCode login returned an unreadable response ({status}): {error}"))?;

    if api_code(&body) != Some(0) {
        let message = api_message(&body);
        return Err(if message.is_empty() {
            format!("ZCode refused to start the login flow (HTTP {status})")
        } else {
            format!("ZCode refused to start the login flow: {message}")
        });
    }

    let data = body
        .get("data")
        .ok_or_else(|| "ZCode login response carried no flow".to_string())?;
    let flow_id = trimmed_string(data.get("flow_id"))
        .ok_or_else(|| "ZCode login response carried no flow id".to_string())?;
    let authorize_url = trimmed_string(data.get("authorize_url"))
        .ok_or_else(|| "ZCode login response carried no authorize URL".to_string())?;
    // The server may replace the token it was handed; when it does, the new one
    // is the one that reads the result.
    let poll_token = trimmed_string(data.get("poll_token")).unwrap_or(poll_token);
    let interval_seconds = data
        .get("poll_interval_sec")
        .and_then(|value| value.as_f64())
        .filter(|value| *value >= 1.0)
        .unwrap_or(3.0) as u64;
    let expires_at = data
        .get("expires_at")
        .and_then(|value| value.as_f64())
        .ok_or_else(|| "ZCode login response carried no expiry".to_string())?;
    let now = chrono::Utc::now().timestamp() as f64;
    let lifetime_seconds = (expires_at - now).max(0.0) as u64;

    Ok(ZcodeOauthFlow {
        provider: provider.to_string(),
        authorize_url,
        poll_url: format!("{OAUTH_BASE_URL}/oauth/cli/poll/{flow_id}"),
        poll_token,
        interval_seconds,
        lifetime_seconds,
    })
}

/// What one poll learned.
pub enum ZcodePollOutcome {
    Pending,
    Ready(ZcodeLoginMaterial),
}

/// Reads the flow once.
///
/// A transport failure is `Pending`, not an error: the browser still holds the
/// user's attention and a dropped request says nothing about the login. Only a
/// terminal answer from the server — `failed`, an expired flow, or a ready body
/// missing its tokens — ends the wait.
pub async fn poll_flow_once(
    db: &SqliteDbState,
    flow: &ZcodeOauthFlow,
    device_mid: Option<&str>,
) -> Result<ZcodePollOutcome, String> {
    let client = crate::http_client::client_with_timeout(db, 20).await?;
    let mut request = client.get(&flow.poll_url);
    for (name, value) in request_headers(&flow.poll_token, device_mid) {
        request = request.header(name, value);
    }

    let response = match request.send().await {
        Ok(response) => response,
        Err(_) => return Ok(ZcodePollOutcome::Pending),
    };
    let status = response.status();
    let body: Value = match response.json().await {
        Ok(body) => body,
        Err(_) => return Ok(ZcodePollOutcome::Pending),
    };

    if api_code(&body) != Some(0) {
        // 3004 is the flow's expiry code; everything else 4xx is terminal too,
        // but a 5xx is the server having a bad minute and is worth retrying.
        if status.is_server_error() {
            return Ok(ZcodePollOutcome::Pending);
        }
        let message = api_message(&body);
        return Err(if message.is_empty() {
            format!("ZCode login failed (HTTP {status})")
        } else {
            format!("ZCode login failed: {message}")
        });
    }

    let data = body
        .get("data")
        .ok_or_else(|| "ZCode login poll carried no result".to_string())?;
    match data.get("status").and_then(|value| value.as_str()) {
        Some("pending") => Ok(ZcodePollOutcome::Pending),
        Some("failed") => Err("ZCode reported the login as failed".to_string()),
        Some("ready") => Ok(ZcodePollOutcome::Ready(read_material(data)?)),
        Some(other) => Err(format!("ZCode reported an unknown login status: {other}")),
        None => Err("ZCode login poll carried no status".to_string()),
    }
}

/// The per-machine identifier ZCode sends as `X-Device-Mid`.
///
/// Optional header, so an unreadable or absent file is not an error.
pub fn read_device_mid(root_dir: &std::path::Path) -> Option<String> {
    let path = root_dir.join("v2/telemetry-state.json");
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    value
        .get("deviceMid")
        .and_then(|mid| mid.as_str())
        .map(|mid| mid.trim().to_string())
        .filter(|mid| !mid.is_empty())
}

fn read_material(data: &Value) -> Result<ZcodeLoginMaterial, String> {
    let jwt = trimmed_string(data.get("token"))
        .ok_or_else(|| "ZCode login result carried no token".to_string())?;

    // The provider block is keyed by whichever provider completed the flow.
    let provider_block = data.get("zai").or_else(|| data.get("bigmodel"));
    let access_token = provider_block.and_then(|block| {
        trimmed_string(block.get("access_token")).or_else(|| trimmed_string(block.get("accessToken")))
    });
    let refresh_token = provider_block.and_then(|block| {
        trimmed_string(block.get("refresh_token")).or_else(|| trimmed_string(block.get("refreshToken")))
    });

    let user_info = data.get("user").filter(|value| value.is_object()).cloned();

    if user_info
        .as_ref()
        .map(|user| {
            user.get("user_id")
                .and_then(|value| value.as_str())
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
        })
        .unwrap_or(true)
    {
        return Err("ZCode login result carried no user".to_string());
    }

    Ok(ZcodeLoginMaterial {
        jwt,
        access_token,
        refresh_token,
        user_info,
    })
}

/// Folds a completed login into the credentials map ZCode reads.
///
/// Keys are namespaced per provider, so a map legitimately holds more than one
/// login and `oauth:active_provider` selects the live one. The zai branch has
/// no refresh token to store; the bigmodel branch may.
pub fn credentials_from_material(provider: &str, material: &ZcodeLoginMaterial) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("zcodejwttoken".to_string(), json!(material.jwt));
    map.insert("oauth:active_provider".to_string(), json!(provider));
    if let Some(access_token) = material
        .access_token
        .as_deref()
        .filter(|token| !token.trim().is_empty())
    {
        map.insert(format!("oauth:{provider}:access_token"), json!(access_token));
    }
    if let Some(refresh_token) = material
        .refresh_token
        .as_deref()
        .filter(|token| !token.trim().is_empty())
    {
        map.insert(format!("oauth:{provider}:refresh_token"), json!(refresh_token));
    }
    if let Some(user_info) = material.user_info.as_ref() {
        // Stored as a JSON *string*; ZCode parses it on read.
        map.insert(
            format!("oauth:{provider}:user_info"),
            json!(user_info.to_string()),
        );
    }
    Value::Object(map)
}

/// How long a flow is allowed to sit waiting for the browser.
pub const FLOW_TIMEOUT: Duration = Duration::from_secs(300);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_two_cli_providers_are_accepted() {
        assert!(is_supported_provider("zai"));
        assert!(is_supported_provider("bigmodel"));
        assert!(!is_supported_provider("openai"));
        assert!(!is_supported_provider(""));
    }

    #[test]
    fn a_poll_token_is_64_hex_characters() {
        let token = new_poll_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(token, new_poll_token());
    }

    #[test]
    fn the_request_headers_carry_the_bearer_and_the_app_version() {
        let headers = request_headers("poll-token", Some("device-1"));
        let find = |name: &str| {
            headers
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        };
        assert_eq!(find("Authorization").as_deref(), Some("Bearer poll-token"));
        assert_eq!(find("X-Device-Mid").as_deref(), Some("device-1"));
        assert_eq!(find("X-ZCode-App-Version").as_deref(), Some(app_version().as_str()));
        assert_eq!(find("HTTP-Referer").as_deref(), Some("https://zcode.z.ai"));
    }

    #[test]
    fn a_ready_body_becomes_a_credentials_map() {
        let data = json!({
            "token": "the-jwt",
            "zai": { "accessToken": "the-access-token" },
            "user": { "user_id": "u-1", "email": "someone@example.com", "name": "Someone" },
        });

        let material = read_material(&data).unwrap();
        let credentials = credentials_from_material("zai", &material);

        assert_eq!(credentials["zcodejwttoken"], json!("the-jwt"));
        assert_eq!(credentials["oauth:active_provider"], json!("zai"));
        assert_eq!(credentials["oauth:zai:access_token"], json!("the-access-token"));
        // zai logins never carry a refresh token, so the key must not appear.
        assert!(credentials.get("oauth:zai:refresh_token").is_none());
        // user_info is a JSON string, not an object.
        let user_info = credentials["oauth:zai:user_info"].as_str().unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(user_info).unwrap()["user_id"],
            json!("u-1")
        );
    }

    #[test]
    fn a_bigmodel_body_keeps_its_refresh_token() {
        let data = json!({
            "token": "the-jwt",
            "bigmodel": { "access_token": "at", "refresh_token": "rt" },
            "user": { "user_id": "u-2" },
        });

        let credentials =
            credentials_from_material("bigmodel", &read_material(&data).unwrap());
        assert_eq!(credentials["oauth:bigmodel:refresh_token"], json!("rt"));
        assert_eq!(credentials["oauth:active_provider"], json!("bigmodel"));
    }

    #[test]
    fn a_ready_body_without_tokens_or_user_is_rejected() {
        assert!(read_material(&json!({ "user": { "user_id": "u-1" } })).is_err());
        assert!(read_material(&json!({ "token": "jwt" })).is_err());
    }
}