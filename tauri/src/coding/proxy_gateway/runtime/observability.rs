use super::http_io::{DebugHttpRequest, DebugHttpResponse};
use super::routes::split_request_target;
use super::GatewayRuntimeContext;
use crate::coding::proxy_gateway::metrics;
use crate::coding::proxy_gateway::request_log;
use crate::coding::proxy_gateway::types::{
    GatewayRequestLogDetail, GatewayRequestLogSummary, MetricEvent,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::sync::OnceLock;

static TRACE_RUN_ID: OnceLock<String> = OnceLock::new();

pub(super) fn record_gateway_observability(
    request: &DebugHttpRequest,
    response: &DebugHttpResponse,
    context: &GatewayRuntimeContext,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
) {
    let Some(paths) = context.paths.as_ref() else {
        return;
    };
    let (request_path, _) = split_request_target(&request.path);
    if request.method == "GET" && request_path == "/health" {
        return;
    }

    let duration_ms = ended_at
        .signed_duration_since(started_at)
        .num_milliseconds()
        .max(0) as u64;
    let (input_tokens, output_tokens) = extract_token_usage(&response.body);
    let total_tokens = input_tokens
        .zip(output_tokens)
        .map(|(input, output)| input.saturating_add(output));
    let settings = context.settings_snapshot();
    let trace_id = trace_id(request);

    if settings.metrics_enabled {
        if let (Some(cli_key), Some(provider_id)) =
            (response.cli_key, response.provider_id.as_ref())
        {
            let event = MetricEvent {
                schema_version: 1,
                trace_id: trace_id.clone(),
                ended_at,
                cli_key,
                provider_id: provider_id.clone(),
                requested_model: response
                    .requested_model
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                upstream_model_id: response
                    .upstream_model_id
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                success: is_success_status(response.status_code),
                status_code: Some(response.status_code),
                error_category: response.error_category.clone(),
                duration_ms,
                attempt_count: response.provider_attempt_count.max(1),
                total_attempt_count: response.attempt_count.max(1),
                failover: response.failover,
                input_tokens,
                output_tokens,
            };
            if let Err(error) = metrics::record_metric_event(paths, &event) {
                log::warn!("Failed to record proxy gateway metric event: {error}");
            }
        }
    }

    if settings.request_log_enabled {
        let record = request_log::new_request_log_record(GatewayRequestLogDetail {
            summary: GatewayRequestLogSummary {
                trace_id,
                started_at,
                ended_at,
                cli_key: response.cli_key,
                route_name: response.route_name.clone(),
                method: request.method.clone(),
                path: request.path.clone(),
                provider_id: response.provider_id.clone(),
                provider_name: response.provider_name.clone(),
                requested_model: response.requested_model.clone(),
                upstream_model_id: response.upstream_model_id.clone(),
                upstream_url: response.upstream_url.clone(),
                status_code: Some(response.status_code),
                success: is_success_status(response.status_code),
                error_category: response.error_category.clone(),
                error_message: (!is_success_status(response.status_code))
                    .then(|| response.note.clone()),
                duration_ms,
                attempt_count: response.provider_attempt_count.max(1),
                total_attempt_count: response.attempt_count.max(1),
                failover: response.failover,
                input_tokens,
                output_tokens,
                total_tokens,
                request_body_bytes: request.body.len() as u64,
                response_body_bytes: response.body.len() as u64,
            },
            request_headers: settings
                .store_headers
                .then(|| request_log::redact_headers(&request.headers)),
            request_body: stored_body_text(
                &request.body,
                settings.store_request_body,
                settings.log_max_body_size_kb,
            ),
            response_headers: settings
                .store_headers
                .then(|| request_log::redact_headers(&response.headers)),
            response_body: stored_body_text(
                &response.body,
                settings.store_response_body,
                settings.log_max_body_size_kb,
            ),
        });
        if let Err(error) = request_log::write_request_log(paths, &settings, &record) {
            log::warn!("Failed to record proxy gateway request log: {error}");
        }
    }
}

fn trace_id(request: &DebugHttpRequest) -> String {
    let run_id = TRACE_RUN_ID
        .get_or_init(|| format!("{}-{}", std::process::id(), Utc::now().timestamp_micros()));
    format!("gw-{}-{}", run_id, request.id)
}

fn is_success_status(status_code: u16) -> bool {
    (200..=399).contains(&status_code)
}

fn stored_body_text(body: &[u8], enabled: bool, max_body_size_kb: u64) -> Option<String> {
    if !enabled {
        return None;
    }
    let max_bytes = max_body_size_kb.saturating_mul(1024) as usize;
    if max_bytes == 0 {
        return Some(String::new());
    }
    if body.len() <= max_bytes {
        return Some(String::from_utf8_lossy(body).to_string());
    }
    let mut text = String::from_utf8_lossy(&body[..max_bytes]).to_string();
    text.push_str(&format!(
        "\n[truncated: stored {} of {} bytes]",
        max_bytes,
        body.len()
    ));
    Some(text)
}

fn extract_token_usage(body: &[u8]) -> (Option<u64>, Option<u64>) {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return (None, None);
    };
    let input_tokens = first_u64_at_paths(
        &value,
        &[
            "/usage/input_tokens",
            "/usage/prompt_tokens",
            "/usageMetadata/promptTokenCount",
        ],
    );
    let output_tokens = first_u64_at_paths(
        &value,
        &[
            "/usage/output_tokens",
            "/usage/completion_tokens",
            "/usageMetadata/candidatesTokenCount",
        ],
    );
    (input_tokens, output_tokens)
}

fn first_u64_at_paths(value: &Value, paths: &[&str]) -> Option<u64> {
    paths
        .iter()
        .find_map(|path| value.pointer(path).and_then(Value::as_u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_with_id(id: u64) -> DebugHttpRequest {
        DebugHttpRequest {
            id,
            peer_addr: "127.0.0.1:50000".parse().unwrap(),
            method: "POST".to_string(),
            path: "/anthropic/v1/messages".to_string(),
            version: "HTTP/1.1".to_string(),
            first_line: "POST /anthropic/v1/messages HTTP/1.1".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
            raw_len: 0,
        }
    }

    #[test]
    fn trace_id_contains_process_run_prefix() {
        let trace = trace_id(&request_with_id(1));

        assert!(trace.starts_with("gw-"));
        assert!(trace.ends_with("-1"));
        assert_ne!(trace, "gw-1");
    }
}
