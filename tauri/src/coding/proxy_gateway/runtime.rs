use super::listen::bind_gateway_listener;
use super::types::{
    GatewayCliKey, ProxyGatewayHealthCheckResult, ProxyGatewaySettings, ProxyGatewayStatus,
};
use crate::coding::{claude_code, codex, gemini_cli};
use crate::db::DbState;
use crate::http_client;
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT_ENCODING, AUTHORIZATION, CONNECTION, CONTENT_LENGTH,
    HOST, PROXY_AUTHENTICATE, PROXY_AUTHORIZATION, TE, TRAILER, TRANSFER_ENCODING, UPGRADE,
};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;
use toml_edit::{DocumentMut, Item};

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub struct ProxyGatewayState {
    pub manager: Mutex<ProxyGatewayManager>,
}

pub struct ProxyGatewayManager {
    runtime: Option<ProxyGatewayRuntime>,
    last_settings: ProxyGatewaySettings,
    last_error: Option<String>,
}

impl Default for ProxyGatewayManager {
    fn default() -> Self {
        Self {
            runtime: None,
            last_settings: ProxyGatewaySettings::default(),
            last_error: None,
        }
    }
}

impl ProxyGatewayManager {
    pub fn start(&mut self, settings: ProxyGatewaySettings) -> Result<ProxyGatewayStatus, String> {
        self.start_internal(settings, None)
    }

    pub fn start_with_db(
        &mut self,
        settings: ProxyGatewaySettings,
        db: Surreal<Db>,
    ) -> Result<ProxyGatewayStatus, String> {
        self.start_internal(settings, Some(db))
    }

    fn start_internal(
        &mut self,
        settings: ProxyGatewaySettings,
        db: Option<Surreal<Db>>,
    ) -> Result<ProxyGatewayStatus, String> {
        if self.runtime.is_some() {
            return Ok(self.status());
        }

        let bound = match bind_gateway_listener(&settings) {
            Ok(bound) => bound,
            Err(error) => {
                self.last_error = Some(error.clone());
                return Err(error);
            }
        };

        let runtime = ProxyGatewayRuntime::spawn(bound, db)?;
        self.last_settings = ProxyGatewaySettings {
            listen_host: runtime.listen_host.clone(),
            listen_port: runtime.listen_port,
            ..settings
        };
        self.last_error = None;
        self.runtime = Some(runtime);
        Ok(self.status())
    }

    pub fn stop(&mut self) -> Result<ProxyGatewayStatus, String> {
        if let Some(mut runtime) = self.runtime.take() {
            runtime.stop();
        }
        Ok(self.status())
    }

    pub fn status(&self) -> ProxyGatewayStatus {
        match &self.runtime {
            Some(runtime) => ProxyGatewayStatus {
                running: true,
                base_url: Some(runtime.base_url.clone()),
                listen_host: runtime.listen_host.clone(),
                listen_port: Some(runtime.listen_port),
                last_error: None,
            },
            None => ProxyGatewayStatus::stopped(&self.last_settings, self.last_error.clone()),
        }
    }

    pub fn health_check(&self) -> ProxyGatewayHealthCheckResult {
        let Some(runtime) = &self.runtime else {
            return ProxyGatewayHealthCheckResult {
                ok: false,
                status_code: None,
                error: Some("Gateway is not running".to_string()),
            };
        };

        health_check_socket(runtime.addr)
    }
}

pub struct ProxyGatewayRuntime {
    addr: SocketAddr,
    listen_host: String,
    listen_port: u16,
    base_url: String,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl ProxyGatewayRuntime {
    fn spawn(
        bound: super::listen::BoundGatewayListener,
        db: Option<Surreal<Db>>,
    ) -> Result<Self, String> {
        let addr = bound
            .listener
            .local_addr()
            .map_err(|error| format!("Failed to read gateway listener address: {error}"))?;
        let running = Arc::new(AtomicBool::new(true));
        let server_running = running.clone();

        let thread = thread::Builder::new()
            .name("ai-toolbox-proxy-gateway".to_string())
            .spawn(move || run_health_server(bound.listener, server_running, db))
            .map_err(|error| format!("Failed to spawn gateway server thread: {error}"))?;

        Ok(Self {
            addr,
            listen_host: bound.listen_host,
            listen_port: bound.listen_port,
            base_url: bound.base_url,
            running,
            thread: Some(thread),
        })
    }

    fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(100));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for ProxyGatewayRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_health_server(
    listener: std::net::TcpListener,
    running: Arc<AtomicBool>,
    db: Option<Surreal<Db>>,
) {
    while running.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, peer_addr)) => {
                if let Err(error) = handle_connection(&mut stream, peer_addr, db.as_ref()) {
                    println!(
                        "[proxy-gateway] request_error peer={} error={}",
                        peer_addr, error
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(25));
            }
            Err(_) => {
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn handle_connection(
    stream: &mut TcpStream,
    peer_addr: SocketAddr,
    db: Option<&Surreal<Db>>,
) -> std::io::Result<()> {
    let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::SeqCst);
    let request = read_http_request(stream, request_id, peer_addr)?;
    log_incoming_request(&request);

    let response = tauri::async_runtime::block_on(route_request(&request, db));
    log_gateway_decision(&request, &response);
    log_response(&request, &response);
    write_response(stream, &response)
}

#[derive(Debug)]
struct DebugHttpRequest {
    id: u64,
    peer_addr: SocketAddr,
    method: String,
    path: String,
    version: String,
    first_line: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    raw_len: usize,
}

struct DebugHttpResponse {
    status_code: u16,
    status_text: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    route_name: String,
    upstream_url: Option<String>,
    note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GatewayRoute {
    cli_key: GatewayCliKey,
    route_name: &'static str,
    forwarded_path: String,
    query: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UpstreamProvider {
    cli_key: GatewayCliKey,
    id: String,
    name: String,
    base_url: String,
    api_key: String,
}

fn read_http_request(
    stream: &mut TcpStream,
    request_id: u64,
    peer_addr: SocketAddr,
) -> std::io::Result<DebugHttpRequest> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;

    let mut raw = Vec::new();
    let mut header_end = None;
    let mut buffer = [0_u8; 8192];

    while header_end.is_none() {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&buffer[..read]);
        header_end = find_header_end(&raw);
    }

    let header_end = header_end.unwrap_or(raw.len());
    let mut header_text = String::from_utf8_lossy(&raw[..header_end]).to_string();
    while header_text.ends_with('\n') || header_text.ends_with('\r') {
        header_text.pop();
    }

    let mut lines = header_text.lines();
    let first_line = lines.next().unwrap_or_default().trim().to_string();
    let mut first_parts = first_line.split_whitespace();
    let method = first_parts.next().unwrap_or_default().to_string();
    let path = first_parts.next().unwrap_or_default().to_string();
    let version = first_parts.next().unwrap_or_default().to_string();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .collect();

    let content_length = header_value(&headers, "content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = header_end.min(raw.len());
    let mut body = raw[body_start..].to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&buffer[..read]);
        body.extend_from_slice(&buffer[..read]);
    }

    Ok(DebugHttpRequest {
        id: request_id,
        peer_addr,
        method,
        path,
        version,
        first_line,
        headers,
        body,
        raw_len: raw.len(),
    })
}

async fn route_request(request: &DebugHttpRequest, db: Option<&Surreal<Db>>) -> DebugHttpResponse {
    let (request_path, _) = split_request_target(&request.path);
    if request.method == "GET" && request_path == "/health" {
        return json_response(
            200,
            "OK",
            json!({"ok": true}),
            "health",
            None,
            "local health endpoint",
        );
    }

    let Some(route) = match_gateway_route(&request.path) else {
        return json_response(
            404,
            "Not Found",
            json!({"error": "not_found"}),
            "unknown",
            None,
            "no gateway route matched this path",
        );
    };

    let Some(db) = db else {
        return json_response(
            503,
            "Service Unavailable",
            json!({
                "error": "gateway_provider_state_missing",
                "message": "Proxy gateway was started without database access, so it cannot resolve upstream providers."
            }),
            route.route_name,
            None,
            "matched CLI gateway route, but runtime has no database handle",
        );
    };

    match forward_to_upstream(request, db, &route).await {
        Ok(response) => response,
        Err(error) => json_response(
            502,
            "Bad Gateway",
            json!({
                "error": "upstream_forward_failed",
                "message": error,
            }),
            route.route_name,
            None,
            "upstream forwarding failed before a response was available",
        ),
    }
}

fn json_response(
    status_code: u16,
    status_text: &str,
    value: Value,
    route_name: &str,
    upstream_url: Option<String>,
    note: &str,
) -> DebugHttpResponse {
    let body = serde_json::to_vec(&value)
        .unwrap_or_else(|_| br#"{"error":"response_serialize_failed"}"#.to_vec());
    DebugHttpResponse {
        status_code,
        status_text: status_text.to_string(),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body,
        route_name: route_name.to_string(),
        upstream_url,
        note: note.to_string(),
    }
}

fn write_response(stream: &mut TcpStream, response: &DebugHttpResponse) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {} {}\r\n",
        response.status_code, response.status_text
    )?;
    let mut has_content_length = false;
    let mut has_connection = false;
    for (name, value) in &response.headers {
        if name.eq_ignore_ascii_case("content-length") {
            has_content_length = true;
        }
        if name.eq_ignore_ascii_case("connection") {
            has_connection = true;
        }
        write!(stream, "{}: {}\r\n", name, value)?;
    }
    if !has_content_length {
        write!(stream, "Content-Length: {}\r\n", response.body.len())?;
    }
    if !has_connection {
        write!(stream, "Connection: close\r\n")?;
    }
    write!(stream, "\r\n")?;
    stream.write_all(&response.body)?;
    stream.flush()
}

async fn forward_to_upstream(
    request: &DebugHttpRequest,
    db: &Surreal<Db>,
    route: &GatewayRoute,
) -> Result<DebugHttpResponse, String> {
    let provider = load_applied_provider(db, route.cli_key).await?;
    let upstream_url = build_target_url(
        &provider.base_url,
        &route.forwarded_path,
        route.query.as_deref(),
    )?;
    let method = reqwest::Method::from_bytes(request.method.as_bytes())
        .map_err(|error| format!("Invalid HTTP method '{}': {error}", request.method))?;
    let headers = build_upstream_headers(request, &provider)?;

    log_upstream_request(request, &provider, &upstream_url, &headers);

    let db_state = DbState(db.clone());
    let client = http_client::client_with_timeout_no_compression(&db_state, 600).await?;
    let response = client
        .request(method, upstream_url.clone())
        .headers(headers)
        .body(request.body.clone())
        .send()
        .await
        .map_err(|error| format!("Failed to send upstream request: {error}"))?;

    let status = response.status();
    let response_headers = filtered_response_headers(response.headers());
    let body = response
        .bytes()
        .await
        .map_err(|error| format!("Failed to read upstream response body: {error}"))?
        .to_vec();

    let gateway_response = DebugHttpResponse {
        status_code: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or("Unknown").to_string(),
        headers: response_headers,
        body,
        route_name: route.route_name.to_string(),
        upstream_url: Some(upstream_url.to_string()),
        note: format!(
            "forwarded to provider id={} name={}",
            provider.id, provider.name
        ),
    };
    log_upstream_response(request, &gateway_response);
    Ok(gateway_response)
}

async fn load_applied_provider(
    db: &Surreal<Db>,
    cli_key: GatewayCliKey,
) -> Result<UpstreamProvider, String> {
    let table = match cli_key {
        GatewayCliKey::Claude => "claude_provider",
        GatewayCliKey::Codex => "codex_provider",
        GatewayCliKey::Gemini => "gemini_cli_provider",
        GatewayCliKey::OpenCode => {
            return Err(
                "OpenCode adapter is intentionally out of scope for the gateway MVP".to_string(),
            )
        }
    };
    let mut result = db
        .query(format!(
            "SELECT *, type::string(id) as id FROM {table} WHERE is_applied = true LIMIT 1"
        ))
        .await
        .map_err(|error| {
            format!(
                "Failed to query applied provider for {}: {error}",
                cli_key.as_str()
            )
        })?;
    let records: Vec<Value> = result.take(0).map_err(|error| {
        format!(
            "Failed to parse applied provider for {}: {error}",
            cli_key.as_str()
        )
    })?;
    let record = records
        .into_iter()
        .next()
        .ok_or_else(|| format!("No applied provider for {}", cli_key.as_str()))?;

    match cli_key {
        GatewayCliKey::Claude => {
            let provider = claude_code::adapter::from_db_value_provider(record);
            if provider.is_disabled {
                return Err(format!(
                    "Applied Claude provider '{}' is disabled",
                    provider.name
                ));
            }
            let settings =
                parse_json_config(&provider.settings_config, "Claude provider settings_config")?;
            let env = settings.get("env").and_then(Value::as_object);
            let base_url = json_object_string(env, "ANTHROPIC_BASE_URL")
                .unwrap_or_else(|| "https://api.anthropic.com".to_string());
            let api_key = json_object_string(env, "ANTHROPIC_AUTH_TOKEN")
                .or_else(|| json_object_string(env, "ANTHROPIC_API_KEY"))
                .ok_or_else(|| {
                    format!("Applied Claude provider '{}' has no API key", provider.name)
                })?;
            Ok(UpstreamProvider {
                cli_key,
                id: provider.id,
                name: provider.name,
                base_url,
                api_key,
            })
        }
        GatewayCliKey::Codex => {
            let provider = codex::adapter::from_db_value_provider(record);
            if provider.is_disabled {
                return Err(format!(
                    "Applied Codex provider '{}' is disabled",
                    provider.name
                ));
            }
            let settings =
                parse_json_config(&provider.settings_config, "Codex provider settings_config")?;
            let auth = settings.get("auth").and_then(Value::as_object);
            let api_key = json_object_string(auth, "OPENAI_API_KEY").ok_or_else(|| {
                format!(
                    "Applied Codex provider '{}' has no OPENAI_API_KEY",
                    provider.name
                )
            })?;
            let config_toml = settings.get("config").and_then(Value::as_str).unwrap_or("");
            let base_url = codex_base_url_from_config(config_toml)
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            Ok(UpstreamProvider {
                cli_key,
                id: provider.id,
                name: provider.name,
                base_url,
                api_key,
            })
        }
        GatewayCliKey::Gemini => {
            let provider = gemini_cli::adapter::from_db_value_provider(record);
            if provider.is_disabled {
                return Err(format!(
                    "Applied Gemini CLI provider '{}' is disabled",
                    provider.name
                ));
            }
            let settings = parse_json_config(
                &provider.settings_config,
                "Gemini CLI provider settings_config",
            )?;
            let env = settings.get("env").and_then(Value::as_object);
            let api_key = json_object_string(env, "GEMINI_API_KEY")
                .or_else(|| json_object_string(env, "GOOGLE_API_KEY"))
                .ok_or_else(|| {
                    format!(
                        "Applied Gemini CLI provider '{}' has no API key",
                        provider.name
                    )
                })?;
            let base_url = json_object_string(env, "GOOGLE_GEMINI_BASE_URL")
                .or_else(|| json_object_string(env, "GOOGLE_VERTEX_BASE_URL"))
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".to_string());
            Ok(UpstreamProvider {
                cli_key,
                id: provider.id,
                name: provider.name,
                base_url,
                api_key,
            })
        }
        GatewayCliKey::OpenCode => unreachable!("OpenCode is rejected before query"),
    }
}

fn parse_json_config(raw: &str, label: &str) -> Result<Value, String> {
    serde_json::from_str(raw).map_err(|error| format!("Failed to parse {label}: {error}"))
}

fn json_object_string(
    object: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Option<String> {
    object
        .and_then(|object| object.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn codex_base_url_from_config(config_toml: &str) -> Option<String> {
    let trimmed = config_toml.trim();
    if trimmed.is_empty() {
        return None;
    }
    let document = trimmed.parse::<DocumentMut>().ok()?;
    let root = document.as_table();
    let providers = root.get("model_providers")?.as_table()?;
    let selected_provider = root
        .get("model_provider")
        .and_then(Item::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(selected_provider) = selected_provider {
        if let Some(base_url) = providers
            .get(selected_provider)
            .and_then(Item::as_table)
            .and_then(|provider| provider.get("base_url"))
            .and_then(Item::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(base_url.to_string());
        }
    }

    let fallback = providers.iter().find_map(|(_, item)| {
        item.as_table()
            .and_then(|provider| provider.get("base_url"))
            .and_then(Item::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    });
    fallback
}

fn match_gateway_route(request_target: &str) -> Option<GatewayRoute> {
    let (path, query) = split_request_target(request_target);
    match strip_cli_prefix(&path, "/anthropic") {
        Some(forwarded_path) => Some(GatewayRoute {
            cli_key: GatewayCliKey::Claude,
            route_name: "anthropic",
            forwarded_path,
            query,
        }),
        None => match strip_cli_prefix(&path, "/openai") {
            Some(forwarded_path)
                if forwarded_path == "/v1" || forwarded_path.starts_with("/v1/") =>
            {
                Some(GatewayRoute {
                    cli_key: GatewayCliKey::Codex,
                    route_name: "openai-compatible",
                    forwarded_path,
                    query,
                })
            }
            _ => match strip_cli_prefix(&path, "/gemini") {
                Some(forwarded_path)
                    if forwarded_path == "/v1beta" || forwarded_path.starts_with("/v1beta/") =>
                {
                    Some(GatewayRoute {
                        cli_key: GatewayCliKey::Gemini,
                        route_name: "gemini",
                        forwarded_path,
                        query,
                    })
                }
                _ => None,
            },
        },
    }
}

fn split_request_target(request_target: &str) -> (String, Option<String>) {
    if let Ok(url) = reqwest::Url::parse(request_target) {
        return (url.path().to_string(), url.query().map(str::to_string));
    }

    match request_target.split_once('?') {
        Some((path, query)) => (path.to_string(), Some(query.to_string())),
        None => (request_target.to_string(), None),
    }
}

fn strip_cli_prefix(path: &str, prefix: &str) -> Option<String> {
    if path == prefix {
        return Some("/".to_string());
    }
    let rest = path.strip_prefix(prefix)?;
    if !rest.starts_with('/') {
        return None;
    }
    Some(rest.to_string())
}

fn build_target_url(
    base_url: &str,
    forwarded_path: &str,
    query: Option<&str>,
) -> Result<reqwest::Url, String> {
    let mut url = reqwest::Url::parse(base_url)
        .map_err(|error| format!("Invalid upstream base URL '{}': {error}", base_url))?;
    let base_path = url.path().trim_end_matches('/');
    let forwarded_path = if base_path.ends_with("/v1")
        && (forwarded_path == "/v1" || forwarded_path.starts_with("/v1/"))
    {
        forwarded_path.strip_prefix("/v1").unwrap_or(forwarded_path)
    } else if base_path.ends_with("/v1beta")
        && (forwarded_path == "/v1beta" || forwarded_path.starts_with("/v1beta/"))
    {
        forwarded_path
            .strip_prefix("/v1beta")
            .unwrap_or(forwarded_path)
    } else {
        forwarded_path
    };

    let mut combined_path = String::new();
    combined_path.push_str(base_path);
    combined_path.push_str(forwarded_path);
    if combined_path.is_empty() {
        combined_path.push('/');
    }
    if !combined_path.starts_with('/') {
        combined_path.insert(0, '/');
    }
    url.set_path(&combined_path);
    url.set_query(query);
    Ok(url)
}

fn build_upstream_headers(
    request: &DebugHttpRequest,
    provider: &UpstreamProvider,
) -> Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    for (name, value) in &request.headers {
        if should_skip_forwarded_request_header(name) {
            continue;
        }
        let header_name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|error| format!("Invalid request header name '{}': {error}", name))?;
        let header_value = HeaderValue::from_str(value)
            .map_err(|error| format!("Invalid request header value for '{}': {error}", name))?;
        headers.insert(header_name, header_value);
    }
    headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
    inject_provider_auth(provider, &mut headers)?;
    Ok(headers)
}

fn should_skip_forwarded_request_header(name: &str) -> bool {
    [
        HOST.as_str(),
        CONTENT_LENGTH.as_str(),
        CONNECTION.as_str(),
        "keep-alive",
        "proxy-connection",
        PROXY_AUTHENTICATE.as_str(),
        PROXY_AUTHORIZATION.as_str(),
        TE.as_str(),
        TRAILER.as_str(),
        TRANSFER_ENCODING.as_str(),
        UPGRADE.as_str(),
        AUTHORIZATION.as_str(),
        "x-api-key",
        "x-goog-api-key",
        "x-goog-api-client",
    ]
    .iter()
    .any(|skip| name.eq_ignore_ascii_case(skip))
}

fn inject_provider_auth(
    provider: &UpstreamProvider,
    headers: &mut HeaderMap,
) -> Result<(), String> {
    match provider.cli_key {
        GatewayCliKey::Claude => {
            let value = HeaderValue::from_str(provider.api_key.trim())
                .map_err(|error| format!("Invalid Claude API key header value: {error}"))?;
            headers.insert("x-api-key", value);
            if !headers.contains_key("anthropic-version") {
                headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
            }
        }
        GatewayCliKey::Codex => {
            let value = HeaderValue::from_str(&format!("Bearer {}", provider.api_key.trim()))
                .map_err(|error| format!("Invalid Codex Authorization header value: {error}"))?;
            headers.insert(AUTHORIZATION, value);
        }
        GatewayCliKey::Gemini => {
            let trimmed = provider.api_key.trim();
            let oauth_token = if trimmed.starts_with("ya29.") {
                Some(trimmed.to_string())
            } else if trimmed.starts_with('{') {
                serde_json::from_str::<Value>(trimmed)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("access_token")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
            } else {
                None
            };
            if let Some(token) = oauth_token {
                let value = HeaderValue::from_str(&format!("Bearer {token}")).map_err(|error| {
                    format!("Invalid Gemini Authorization header value: {error}")
                })?;
                headers.insert(AUTHORIZATION, value);
                headers.insert(
                    "x-goog-api-client",
                    HeaderValue::from_static("GeminiCLI/1.0"),
                );
            } else {
                let value = HeaderValue::from_str(trimmed)
                    .map_err(|error| format!("Invalid Gemini API key header value: {error}"))?;
                headers.insert("x-goog-api-key", value);
            }
        }
        GatewayCliKey::OpenCode => {
            return Err("OpenCode adapter is intentionally out of scope".to_string())
        }
    }
    Ok(())
}

fn filtered_response_headers(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            if should_skip_forwarded_response_header(name.as_str()) {
                return None;
            }
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_string(), value.to_string()))
        })
        .collect()
}

fn should_skip_forwarded_response_header(name: &str) -> bool {
    [
        CONTENT_LENGTH.as_str(),
        CONNECTION.as_str(),
        "keep-alive",
        "proxy-connection",
        PROXY_AUTHENTICATE.as_str(),
        PROXY_AUTHORIZATION.as_str(),
        TE.as_str(),
        TRAILER.as_str(),
        TRANSFER_ENCODING.as_str(),
        UPGRADE.as_str(),
    ]
    .iter()
    .any(|skip| name.eq_ignore_ascii_case(skip))
}

fn find_header_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
        .or_else(|| {
            raw.windows(2)
                .position(|window| window == b"\n\n")
                .map(|index| index + 2)
        })
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn log_incoming_request(request: &DebugHttpRequest) {
    println!(
        "[proxy-gateway] request_begin id={} peer={} raw_bytes={} first_line={}",
        request.id, request.peer_addr, request.raw_len, request.first_line
    );
    println!(
        "[proxy-gateway] request_line id={} method={} path={} version={}",
        request.id, request.method, request.path, request.version
    );
    println!(
        "[proxy-gateway] request_headers_begin id={} count={}",
        request.id,
        request.headers.len()
    );
    for (name, value) in &request.headers {
        println!(
            "[proxy-gateway] request_header id={} {}: {}",
            request.id, name, value
        );
    }
    println!("[proxy-gateway] request_headers_end id={}", request.id);
    println!(
        "[proxy-gateway] request_body_begin id={} bytes={}",
        request.id,
        request.body.len()
    );
    if request.body.is_empty() {
        println!("[proxy-gateway] request_body id={} <empty>", request.id);
    } else {
        println!(
            "[proxy-gateway] request_body id={}\n{}",
            request.id,
            format_body_for_debug_log(&request.body)
        );
    }
    println!("[proxy-gateway] request_body_end id={}", request.id);
}

fn format_body_for_debug_log(body: &[u8]) -> String {
    if body.is_empty() {
        return "<empty>".to_string();
    }

    let text = String::from_utf8_lossy(body);
    let Ok(mut json) = serde_json::from_str::<Value>(&text) else {
        return text.to_string();
    };

    omit_large_message_fields(&mut json);
    serde_json::to_string_pretty(&json).unwrap_or_else(|_| text.to_string())
}

fn omit_large_message_fields(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if let Some(messages) = object.get_mut("messages") {
                *messages = summarize_omitted_json_value(messages);
            }
            for child in object.values_mut() {
                omit_large_message_fields(child);
            }
        }
        Value::Array(items) => {
            for child in items {
                omit_large_message_fields(child);
            }
        }
        _ => {}
    }
}

fn summarize_omitted_json_value(value: &Value) -> Value {
    match value {
        Value::Array(items) => {
            Value::String(format!("[omitted messages array: {} items]", items.len()))
        }
        Value::Object(object) => {
            Value::String(format!("[omitted messages object: {} keys]", object.len()))
        }
        Value::String(text) => {
            Value::String(format!("[omitted messages string: {} chars]", text.len()))
        }
        _ => Value::String("[omitted messages]".to_string()),
    }
}

fn log_gateway_decision(request: &DebugHttpRequest, response: &DebugHttpResponse) {
    println!(
        "[proxy-gateway] route_decision id={} route={} upstream={} note={}",
        request.id,
        response.route_name,
        response.upstream_url.as_deref().unwrap_or("<none>"),
        response.note
    );
}

fn log_response(request: &DebugHttpRequest, response: &DebugHttpResponse) {
    println!(
        "[proxy-gateway] response_begin id={} status={} {} body_bytes={}",
        request.id,
        response.status_code,
        response.status_text,
        response.body.len()
    );
    for (name, value) in &response.headers {
        println!(
            "[proxy-gateway] response_header id={} {}: {}",
            request.id, name, value
        );
    }
    println!(
        "[proxy-gateway] response_header id={} Content-Length: {}",
        request.id,
        response.body.len()
    );
    println!(
        "[proxy-gateway] response_body id={}\n{}",
        request.id,
        format_body_for_debug_log(&response.body)
    );
    println!("[proxy-gateway] response_end id={}", request.id);
}

fn log_upstream_request(
    request: &DebugHttpRequest,
    provider: &UpstreamProvider,
    upstream_url: &reqwest::Url,
    headers: &HeaderMap,
) {
    println!(
        "[proxy-gateway] upstream_request_begin id={} provider_id={} provider_name={} cli={} method={} url={} body_bytes={}",
        request.id,
        provider.id,
        provider.name,
        provider.cli_key.as_str(),
        request.method,
        upstream_url,
        request.body.len()
    );
    println!(
        "[proxy-gateway] upstream_request_headers_begin id={} count={}",
        request.id,
        headers.len()
    );
    for (name, value) in headers {
        println!(
            "[proxy-gateway] upstream_request_header id={} {}: {}",
            request.id,
            name,
            format_header_value_for_debug(name.as_str(), value)
        );
    }
    println!(
        "[proxy-gateway] upstream_request_headers_end id={}",
        request.id
    );
    if request.body.is_empty() {
        println!(
            "[proxy-gateway] upstream_request_body id={} <empty>",
            request.id
        );
    } else {
        println!(
            "[proxy-gateway] upstream_request_body id={}\n{}",
            request.id,
            format_body_for_debug_log(&request.body)
        );
    }
    println!("[proxy-gateway] upstream_request_end id={}", request.id);
}

fn log_upstream_response(request: &DebugHttpRequest, response: &DebugHttpResponse) {
    println!(
        "[proxy-gateway] upstream_response_begin id={} status={} {} body_bytes={}",
        request.id,
        response.status_code,
        response.status_text,
        response.body.len()
    );
    for (name, value) in &response.headers {
        println!(
            "[proxy-gateway] upstream_response_header id={} {}: {}",
            request.id, name, value
        );
    }
    if response.body.is_empty() {
        println!(
            "[proxy-gateway] upstream_response_body id={} <empty>",
            request.id
        );
    } else {
        println!(
            "[proxy-gateway] upstream_response_body id={}\n{}",
            request.id,
            format_body_for_debug_log(&response.body)
        );
    }
    println!("[proxy-gateway] upstream_response_end id={}", request.id);
}

fn format_header_value_for_debug(name: &str, value: &HeaderValue) -> String {
    let value = value.to_str().unwrap_or("<non-utf8>");
    if is_sensitive_header(name) {
        mask_secret(value)
    } else {
        value.to_string()
    }
}

fn is_sensitive_header(name: &str) -> bool {
    [
        AUTHORIZATION.as_str(),
        "x-api-key",
        "x-goog-api-key",
        "cookie",
        "set-cookie",
    ]
    .iter()
    .any(|sensitive| name.eq_ignore_ascii_case(sensitive))
}

fn mask_secret(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "<empty>".to_string();
    }
    let char_count = trimmed.chars().count();
    if char_count <= 12 {
        return "***".to_string();
    }
    let head: String = trimmed.chars().take(6).collect();
    let tail: String = trimmed
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{head}...{tail}")
}

fn health_check_socket(addr: SocketAddr) -> ProxyGatewayHealthCheckResult {
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2));
    let Ok(mut stream) = stream else {
        return ProxyGatewayHealthCheckResult {
            ok: false,
            status_code: None,
            error: Some("Failed to connect to gateway health endpoint".to_string()),
        };
    };

    let request = b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
    if let Err(error) = stream.write_all(request) {
        return ProxyGatewayHealthCheckResult {
            ok: false,
            status_code: None,
            error: Some(format!("Failed to write health request: {error}")),
        };
    }

    let mut response = String::new();
    if let Err(error) = stream.read_to_string(&mut response) {
        return ProxyGatewayHealthCheckResult {
            ok: false,
            status_code: None,
            error: Some(format!("Failed to read health response: {error}")),
        };
    }

    let status_code = response
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok());

    ProxyGatewayHealthCheckResult {
        ok: status_code == Some(200),
        status_code,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::mpsc;
    use surrealdb::engine::local::SurrealKv;
    use surrealdb::Surreal;

    fn next_available_port() -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("reserve port");
        listener.local_addr().unwrap().port()
    }

    fn debug_request(method: &str, path: &str, body: &[u8]) -> DebugHttpRequest {
        DebugHttpRequest {
            id: 42,
            peer_addr: "127.0.0.1:50000".parse().unwrap(),
            method: method.to_string(),
            path: path.to_string(),
            version: "HTTP/1.1".to_string(),
            first_line: format!("{method} {path} HTTP/1.1"),
            headers: vec![
                ("Host".to_string(), "127.0.0.1".to_string()),
                ("Authorization".to_string(), "Bearer gateway".to_string()),
                ("Content-Type".to_string(), "application/json".to_string()),
                ("Content-Length".to_string(), body.len().to_string()),
            ],
            body: body.to_vec(),
            raw_len: body.len(),
        }
    }

    fn start_test_upstream() -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind upstream");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept upstream");
            let raw = read_test_http_request(&mut stream);
            tx.send(raw).expect("send captured request");
            let body = br#"{"ok":true}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nX-Upstream-Test: yes\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .expect("write upstream headers");
            stream.write_all(body).expect("write upstream body");
        });
        (base_url, rx)
    }

    fn read_test_http_request(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("set read timeout");
        let mut raw = Vec::new();
        let mut header_end = None;
        let mut buffer = [0_u8; 1024];
        while header_end.is_none() {
            let read = stream.read(&mut buffer).expect("read headers");
            if read == 0 {
                break;
            }
            raw.extend_from_slice(&buffer[..read]);
            header_end = find_header_end(&raw);
        }
        let header_end = header_end.unwrap_or(raw.len());
        let header_text = String::from_utf8_lossy(&raw[..header_end]).to_string();
        let headers: Vec<(String, String)> = header_text
            .lines()
            .skip(1)
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
            .collect();
        let content_length = header_value(&headers, "content-length")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        let mut body_len = raw.len().saturating_sub(header_end);
        while body_len < content_length {
            let read = stream.read(&mut buffer).expect("read body");
            if read == 0 {
                break;
            }
            raw.extend_from_slice(&buffer[..read]);
            body_len += read;
        }
        String::from_utf8_lossy(&raw).to_string()
    }

    async fn create_test_db() -> (tempfile::TempDir, Surreal<Db>) {
        let dir = tempfile::tempdir().expect("temp db");
        let db = Surreal::new::<SurrealKv>(dir.path().to_path_buf())
            .await
            .expect("open test db");
        db.use_ns("ai_toolbox")
            .use_db("main")
            .await
            .expect("select ns db");
        db.query("UPSERT settings:`app` CONTENT $data")
            .bind(("data", json!({"proxy_mode": "direct"})))
            .await
            .expect("save app settings");
        (dir, db)
    }

    #[test]
    fn status_is_stopped_by_default() {
        let manager = ProxyGatewayManager::default();
        let status = manager.status();
        assert!(!status.running);
        assert_eq!(status.base_url, None);
    }

    #[test]
    fn health_check_reports_not_running() {
        let manager = ProxyGatewayManager::default();
        let health = manager.health_check();
        assert!(!health.ok);
        assert_eq!(health.status_code, None);
    }

    #[test]
    fn start_exposes_health_endpoint_and_stop_releases_port() {
        let port = next_available_port();
        let mut manager = ProxyGatewayManager::default();
        let status = manager
            .start(ProxyGatewaySettings {
                listen_port: port,
                ..ProxyGatewaySettings::default()
            })
            .expect("start gateway");

        assert!(status.running);
        assert_eq!(status.listen_port, Some(port));
        assert_eq!(manager.health_check().status_code, Some(200));

        manager.stop().expect("stop gateway");
        assert!(!manager.status().running);

        let rebound = TcpListener::bind(("127.0.0.1", port));
        assert!(rebound.is_ok());
    }

    #[test]
    fn start_returns_current_status_when_already_running() {
        let port = next_available_port();
        let mut manager = ProxyGatewayManager::default();
        let first = manager
            .start(ProxyGatewaySettings {
                listen_port: port,
                ..ProxyGatewaySettings::default()
            })
            .expect("start gateway");
        let second = manager
            .start(ProxyGatewaySettings {
                listen_port: next_available_port(),
                ..ProxyGatewaySettings::default()
            })
            .expect("second start");

        assert_eq!(first.base_url, second.base_url);
        manager.stop().expect("stop gateway");
    }

    #[test]
    fn provider_route_reports_missing_db_when_started_without_db() {
        let port = next_available_port();
        let mut manager = ProxyGatewayManager::default();
        manager
            .start(ProxyGatewaySettings {
                listen_port: port,
                ..ProxyGatewaySettings::default()
            })
            .expect("start gateway");

        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect gateway");
        let body = r#"{"model":"debug-model","messages":[{"role":"user","content":"say hi"}]}"#;
        let request = format!(
            "POST /anthropic/v1/messages HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(request.as_bytes()).expect("write request");

        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read response");

        assert!(response.starts_with("HTTP/1.1 503 Service Unavailable"));
        assert!(response.contains("gateway_provider_state_missing"));
        manager.stop().expect("stop gateway");
    }

    #[test]
    fn debug_body_log_omits_messages_field() {
        let body = br#"{"model":"debug-model","messages":[{"role":"user","content":"large"}],"metadata":{"messages":[1,2,3]}}"#;
        let formatted = format_body_for_debug_log(body);

        assert!(formatted.contains(r#""model": "debug-model""#));
        assert!(formatted.contains("[omitted messages array: 1 items]"));
        assert!(formatted.contains("[omitted messages array: 3 items]"));
        assert!(!formatted.contains("large"));
    }

    #[test]
    fn gateway_routes_strip_cli_prefixes() {
        let claude = match_gateway_route("/anthropic/v1/messages?beta=1").unwrap();
        assert_eq!(claude.cli_key, GatewayCliKey::Claude);
        assert_eq!(claude.forwarded_path, "/v1/messages");
        assert_eq!(claude.query.as_deref(), Some("beta=1"));

        let codex = match_gateway_route("/openai/v1/responses").unwrap();
        assert_eq!(codex.cli_key, GatewayCliKey::Codex);
        assert_eq!(codex.forwarded_path, "/v1/responses");

        let gemini = match_gateway_route("/gemini/v1beta/models/gemini:generateContent").unwrap();
        assert_eq!(gemini.cli_key, GatewayCliKey::Gemini);
        assert_eq!(
            gemini.forwarded_path,
            "/v1beta/models/gemini:generateContent"
        );

        assert!(match_gateway_route("/openai/v2/responses").is_none());
        assert!(match_gateway_route("/anthropic-extra/v1/messages").is_none());
    }

    #[test]
    fn build_target_url_deduplicates_version_paths() {
        assert_eq!(
            build_target_url("https://api.example.com/v1", "/v1/messages", Some("a=1"))
                .unwrap()
                .to_string(),
            "https://api.example.com/v1/messages?a=1"
        );
        assert_eq!(
            build_target_url(
                "https://generativelanguage.googleapis.com/v1beta",
                "/v1beta/models/gemini:generateContent",
                None,
            )
            .unwrap()
            .to_string(),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini:generateContent"
        );
    }

    #[test]
    fn provider_config_extractors_read_existing_shapes() {
        let claude_settings = json!({
            "env": {
                "ANTHROPIC_AUTH_TOKEN": "claude-key",
                "ANTHROPIC_BASE_URL": "https://claude.example.com/v1"
            }
        });
        let env = claude_settings.get("env").and_then(Value::as_object);
        assert_eq!(
            json_object_string(env, "ANTHROPIC_AUTH_TOKEN").as_deref(),
            Some("claude-key")
        );

        let codex_toml = r#"
model_provider = "custom"

[model_providers.custom]
base_url = "https://openai.example.com/v1"
"#;
        assert_eq!(
            codex_base_url_from_config(codex_toml).as_deref(),
            Some("https://openai.example.com/v1")
        );

        let gemini_settings = json!({
            "env": {
                "GEMINI_API_KEY": "gemini-key",
                "GOOGLE_GEMINI_BASE_URL": "https://gemini.example.com/v1beta"
            }
        });
        let env = gemini_settings.get("env").and_then(Value::as_object);
        assert_eq!(
            json_object_string(env, "GOOGLE_GEMINI_BASE_URL").as_deref(),
            Some("https://gemini.example.com/v1beta")
        );
    }

    #[test]
    fn upstream_headers_strip_gateway_auth_and_inject_provider_auth() {
        let body = br#"{"model":"debug"}"#;
        let request = debug_request("POST", "/anthropic/v1/messages", body);
        let provider = UpstreamProvider {
            cli_key: GatewayCliKey::Claude,
            id: "p1".to_string(),
            name: "Provider".to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            api_key: "real-key".to_string(),
        };
        let headers = build_upstream_headers(&request, &provider).unwrap();

        assert!(!headers.contains_key(AUTHORIZATION));
        assert!(!headers.contains_key(HOST));
        assert!(!headers.contains_key(CONTENT_LENGTH));
        assert_eq!(
            headers
                .get("x-api-key")
                .and_then(|value| value.to_str().ok()),
            Some("real-key")
        );
        assert_eq!(
            headers
                .get("anthropic-version")
                .and_then(|value| value.to_str().ok()),
            Some("2023-06-01")
        );
    }

    #[test]
    fn route_request_forwards_to_applied_claude_provider() {
        let (base_url, captured_rx) = start_test_upstream();
        let body = br#"{"model":"debug-model","messages":[{"role":"user","content":"say hi"}]}"#;
        let request = debug_request("POST", "/anthropic/v1/messages?debug=1", body);

        let (_dir, db) = tauri::async_runtime::block_on(create_test_db());
        tauri::async_runtime::block_on(async {
            let settings_config = json!({
                "env": {
                    "ANTHROPIC_BASE_URL": base_url,
                    "ANTHROPIC_AUTH_TOKEN": "provider-key"
                }
            })
            .to_string();
            db.query("CREATE claude_provider CONTENT $data")
                .bind((
                    "data",
                    json!({
                        "name": "Local Upstream",
                        "category": "custom",
                        "settings_config": settings_config,
                        "extra_settings_config": "{}",
                        "is_applied": true,
                        "is_disabled": false,
                    }),
                ))
                .await
                .expect("insert provider");
        });

        let response = tauri::async_runtime::block_on(route_request(&request, Some(&db)));
        assert_eq!(response.status_code, 200);
        assert_eq!(response.body, br#"{"ok":true}"#);
        assert!(response
            .headers
            .iter()
            .any(|(name, value)| name.eq_ignore_ascii_case("x-upstream-test") && value == "yes"));

        let captured = captured_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("captured upstream request");
        let captured_lower = captured.to_ascii_lowercase();
        assert!(captured.starts_with("POST /v1/messages?debug=1 HTTP/1.1"));
        assert!(captured_lower.contains("x-api-key: provider-key"));
        assert!(!captured_lower.contains("authorization: bearer gateway"));
        assert!(captured.contains(r#""content":"say hi""#));
    }
}
