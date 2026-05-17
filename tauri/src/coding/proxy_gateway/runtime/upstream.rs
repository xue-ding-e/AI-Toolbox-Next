use super::debug_log::{log_upstream_request, log_upstream_response};
use super::http_io::{json_response, DebugHttpRequest, DebugHttpResponse};
use super::providers::{load_candidate_providers, UpstreamProvider};
use super::routes::{build_target_url, match_gateway_route, split_request_target, GatewayRoute};
use super::GatewayRuntimeContext;
use crate::coding::proxy_gateway::model_health::{self, GatewayFailureKind, ModelHealthRegistry};
use crate::coding::proxy_gateway::types::{GatewayCliKey, ProviderModelHealthKey};
use crate::db::DbState;
use crate::http_client;
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT_ENCODING, AUTHORIZATION, CONNECTION, CONTENT_LENGTH,
    HOST, PROXY_AUTHENTICATE, PROXY_AUTHORIZATION, TE, TRAILER, TRANSFER_ENCODING, UPGRADE,
};
use serde_json::{json, Value};
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

struct GatewayForwardError {
    message: String,
    kind: GatewayFailureKind,
}

pub(super) async fn route_request(
    request: &DebugHttpRequest,
    context: &GatewayRuntimeContext,
) -> DebugHttpResponse {
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

    let Some(db) = context.db.as_ref() else {
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

    forward_to_upstream(request, db, context, &route).await
}

async fn forward_to_upstream(
    request: &DebugHttpRequest,
    db: &Surreal<Db>,
    context: &GatewayRuntimeContext,
    route: &GatewayRoute,
) -> DebugHttpResponse {
    let requested_model =
        extract_requested_model(request, route).unwrap_or_else(|| "unknown".to_string());
    let providers = match load_candidate_providers(db, route.cli_key).await {
        Ok(providers) if !providers.is_empty() => providers,
        Ok(_) => {
            let mut response = json_response(
                502,
                "Bad Gateway",
                json!({
                    "error": "gateway_provider_missing",
                    "message": format!("No enabled provider for {}", route.cli_key.as_str()),
                }),
                route.route_name,
                None,
                "matched CLI gateway route, but no enabled upstream provider is configured",
            );
            response.cli_key = Some(route.cli_key);
            response.requested_model = Some(requested_model);
            response.error_category = Some("provider_missing".to_string());
            return response;
        }
        Err(error) => {
            let mut response = json_response(
                502,
                "Bad Gateway",
                json!({
                    "error": "gateway_provider_load_failed",
                    "message": error,
                }),
                route.route_name,
                None,
                "failed to resolve upstream provider candidates",
            );
            response.cli_key = Some(route.cli_key);
            response.requested_model = Some(requested_model);
            response.error_category = Some("provider_load_failed".to_string());
            return response;
        }
    };

    let settings = context.settings_snapshot();
    let mut health_registry = context.paths.as_ref().and_then(|paths| {
        match ModelHealthRegistry::load(&paths.model_health_path(), settings.clone()) {
            Ok(mut registry) => {
                registry.refresh_due_cooldowns(chrono::Utc::now());
                Some(registry)
            }
            Err(error) => {
                log::warn!("Failed to load proxy gateway model health: {error}");
                None
            }
        }
    });
    let mut health_changed = false;
    let mut attempt_count = 0_u32;
    let mut last_failure_response = None;
    let mut skipped_by_health = Vec::new();

    for provider in providers {
        let upstream_model_id = requested_model.clone();
        let health_key = ProviderModelHealthKey {
            cli_key: route.cli_key,
            provider_id: provider.id.clone(),
            upstream_model_id: upstream_model_id.clone(),
        };

        if health_registry
            .as_ref()
            .is_some_and(|registry| !registry.is_model_available(&health_key, chrono::Utc::now()))
        {
            skipped_by_health.push(provider.name.clone());
            continue;
        }

        attempt_count = attempt_count.saturating_add(1);
        match send_upstream_request(request, db, route, &provider).await {
            Ok(mut response) => {
                response.cli_key = Some(route.cli_key);
                response.provider_id = Some(provider.id.clone());
                response.provider_name = Some(provider.name.clone());
                response.requested_model = Some(requested_model.clone());
                response.upstream_model_id = Some(upstream_model_id);
                response.attempt_count = attempt_count;
                response.failover = attempt_count > 1;

                if let Some(failure_kind) = classify_status_failure(response.status_code) {
                    let category = model_health::classify_failure(failure_kind).category;
                    response.error_category = Some(category.to_string());
                    if let Some(registry) = health_registry.as_mut() {
                        health_changed |=
                            registry.record_failure(&health_key, failure_kind, chrono::Utc::now());
                    }
                    if should_retry_failure(failure_kind) {
                        last_failure_response = Some(response);
                        continue;
                    }
                } else if let Some(registry) = health_registry.as_mut() {
                    health_changed |= registry.record_success(&health_key);
                }

                save_health_registry_if_needed(context, health_registry.as_ref(), health_changed);
                return response;
            }
            Err(error) => {
                let category = model_health::classify_failure(error.kind).category;
                if let Some(registry) = health_registry.as_mut() {
                    health_changed |=
                        registry.record_failure(&health_key, error.kind, chrono::Utc::now());
                }
                let mut response = json_response(
                    502,
                    "Bad Gateway",
                    json!({
                        "error": "upstream_forward_failed",
                        "message": error.message,
                    }),
                    route.route_name,
                    None,
                    "upstream forwarding failed before a response was available",
                );
                response.cli_key = Some(route.cli_key);
                response.provider_id = Some(provider.id);
                response.provider_name = Some(provider.name);
                response.requested_model = Some(requested_model.clone());
                response.upstream_model_id = Some(health_key.upstream_model_id);
                response.error_category = Some(category.to_string());
                response.attempt_count = attempt_count;
                response.failover = attempt_count > 1;
                last_failure_response = Some(response);
            }
        }
    }

    save_health_registry_if_needed(context, health_registry.as_ref(), health_changed);
    if let Some(response) = last_failure_response {
        return response;
    }

    let mut response = json_response(
        503,
        "Service Unavailable",
        json!({
            "error": "model_temporarily_unavailable",
            "message": "All provider candidates for this model are currently cooling down.",
            "skipped_providers": skipped_by_health,
        }),
        route.route_name,
        None,
        "all upstream provider candidates were skipped by model health",
    );
    response.cli_key = Some(route.cli_key);
    response.requested_model = Some(requested_model);
    response.error_category = Some("cooling_down".to_string());
    response
}

async fn send_upstream_request(
    request: &DebugHttpRequest,
    db: &Surreal<Db>,
    route: &GatewayRoute,
    provider: &UpstreamProvider,
) -> Result<DebugHttpResponse, GatewayForwardError> {
    let upstream_url = build_target_url(
        &provider.base_url,
        &route.forwarded_path,
        route.query.as_deref(),
    )
    .map_err(|message| GatewayForwardError {
        message,
        kind: GatewayFailureKind::GatewayParse,
    })?;
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).map_err(|error| {
        GatewayForwardError {
            message: format!("Invalid HTTP method '{}': {error}", request.method),
            kind: GatewayFailureKind::RequestSchema,
        }
    })?;
    let headers =
        build_upstream_headers(request, provider).map_err(|message| GatewayForwardError {
            message,
            kind: GatewayFailureKind::GatewayParse,
        })?;

    log_upstream_request(request, provider, &upstream_url, &headers);

    let db_state = DbState(db.clone());
    let client = http_client::client_with_timeout_no_compression(&db_state, 600)
        .await
        .map_err(|message| GatewayForwardError {
            message,
            kind: GatewayFailureKind::Connection,
        })?;
    let response = client
        .request(method, upstream_url.clone())
        .headers(headers)
        .body(request.body.clone())
        .send()
        .await
        .map_err(|error| GatewayForwardError {
            message: format!("Failed to send upstream request: {error}"),
            kind: classify_reqwest_error(&error),
        })?;

    let status = response.status();
    let response_headers = filtered_response_headers(response.headers());
    let body = response
        .bytes()
        .await
        .map_err(|error| GatewayForwardError {
            message: format!("Failed to read upstream response body: {error}"),
            kind: classify_reqwest_error(&error),
        })?
        .to_vec();

    let gateway_response = DebugHttpResponse {
        status_code: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or("Unknown").to_string(),
        headers: response_headers,
        body,
        cli_key: Some(provider.cli_key),
        route_name: route.route_name.to_string(),
        provider_id: Some(provider.id.clone()),
        provider_name: Some(provider.name.clone()),
        requested_model: None,
        upstream_model_id: None,
        upstream_url: Some(upstream_url.to_string()),
        error_category: None,
        attempt_count: 1,
        failover: false,
        note: format!(
            "forwarded to provider id={} name={}",
            provider.id, provider.name
        ),
    };
    log_upstream_response(request, &gateway_response);
    Ok(gateway_response)
}

pub(super) fn build_upstream_headers(
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

fn extract_requested_model(request: &DebugHttpRequest, route: &GatewayRoute) -> Option<String> {
    extract_model_from_json_body(&request.body).or_else(|| {
        if route.cli_key == GatewayCliKey::Gemini {
            extract_gemini_model_from_path(&route.forwarded_path)
        } else {
            None
        }
    })
}

fn extract_model_from_json_body(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn extract_gemini_model_from_path(path: &str) -> Option<String> {
    let marker = "/models/";
    let start = path.find(marker)? + marker.len();
    let model_part = &path[start..];
    let end = model_part
        .find(|ch| matches!(ch, ':' | '/' | '?'))
        .unwrap_or(model_part.len());
    Some(model_part[..end].trim().to_string()).filter(|value| !value.is_empty())
}

fn classify_reqwest_error(error: &reqwest::Error) -> GatewayFailureKind {
    if error.is_timeout() {
        GatewayFailureKind::Timeout
    } else {
        GatewayFailureKind::Connection
    }
}

fn classify_status_failure(status_code: u16) -> Option<GatewayFailureKind> {
    match status_code {
        200..=399 => None,
        400 => Some(GatewayFailureKind::RequestSchema),
        401 | 403 => Some(GatewayFailureKind::Auth),
        404 => Some(GatewayFailureKind::ModelNotFound),
        408 => Some(GatewayFailureKind::Timeout),
        429 => Some(GatewayFailureKind::RateLimit),
        500..=599 => Some(GatewayFailureKind::Upstream5xx),
        _ => Some(GatewayFailureKind::RequestSchema),
    }
}

fn should_retry_failure(kind: GatewayFailureKind) -> bool {
    !matches!(
        kind,
        GatewayFailureKind::RequestSchema
            | GatewayFailureKind::ClientCancelled
            | GatewayFailureKind::GatewayParse
    )
}

fn save_health_registry_if_needed(
    context: &GatewayRuntimeContext,
    registry: Option<&ModelHealthRegistry>,
    changed: bool,
) {
    if !changed {
        return;
    }
    let (Some(paths), Some(registry)) = (context.paths.as_ref(), registry) else {
        return;
    };
    if let Err(error) = registry.save(&paths.model_health_path()) {
        log::warn!("Failed to save proxy gateway model health: {error}");
    }
}
