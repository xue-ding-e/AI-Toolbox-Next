use super::http_io::{DebugHttpRequest, DebugHttpResponse};
use super::routes::split_request_target;
use super::GatewayRuntimeContext;
use crate::coding::proxy_gateway::paths::ProxyGatewayPaths;
use crate::coding::proxy_gateway::request_log;
use crate::coding::proxy_gateway::types::{
    GatewayRequestLogDetail, GatewayRequestLogSummary, GatewayStreamOutcome,
    GatewayUsageRecordedEvent, ProxyGatewaySettings,
};
use crate::coding::proxy_gateway::usage_parser::stable_usage_request_id;
use crate::coding::proxy_gateway::usage_stats::{self, RecordRequestSummaryOutcome};
use chrono::{DateTime, Utc};
use std::sync::OnceLock;
use tauri::Emitter;

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
    if should_skip_observability(&request.method, &request_path) {
        return;
    }

    let duration_ms = ended_at
        .signed_duration_since(started_at)
        .num_milliseconds()
        .max(0) as u64;
    let input_tokens = response.token_usage.input_tokens;
    let output_tokens = response.token_usage.output_tokens;
    let cache_read_tokens = response.token_usage.cache_read_tokens;
    let cache_creation_tokens = response.token_usage.cache_creation_tokens;
    let total_tokens = response.token_usage.total_tokens();
    let settings = context.settings_snapshot();
    let fallback_trace_id = process_local_trace_id(request);
    let upstream_response_body_snapshot = response.upstream_response_body_snapshot();
    // Prefer upstream envelope id for stable usage keys (cc-switch c9ac6efd).
    // Fallback remains process-local so request-list/detail still have a unique id.
    let trace_id = response
        .cli_key
        .map(|cli_key| {
            stable_usage_request_id(
                cli_key,
                response.provider_id.as_deref(),
                response.token_usage.envelope_id.as_deref(),
                &fallback_trace_id,
            )
        })
        .unwrap_or(fallback_trace_id);

    let should_record_summary = settings.request_log_enabled || settings.metrics_enabled;
    if should_record_summary {
        // Derive success from the stream verdict, not the HTTP status code.
        // Once `write_streaming_body` has written `HTTP/1.1 200`, a later
        // mid-stream failure can no longer change that code; the outcome
        // enum records what actually reached the client.
        let success = match response.stream_outcome {
            GatewayStreamOutcome::NotStreaming => is_success_status(response.status_code),
            _ => response.stream_outcome.is_success(),
        };
        // Build compact fields first (no body/header yet) so usage-key resolution can
        // decide skip/collision before we write expensive JSONL detail.
        let mut detail = GatewayRequestLogDetail {
            summary: GatewayRequestLogSummary {
                trace_id,
                started_at,
                ended_at,
                cli_key: response.cli_key,
                route_name: response.route_name.clone(),
                method: request.method.clone(),
                path: request_log::redact_request_path(&request.path),
                provider_id: response.provider_id.clone(),
                provider_name: response.provider_name.clone(),
                provider_type: response.provider_type.clone(),
                cost_multiplier: response.cost_multiplier.clone(),
                pricing_model_source: response.pricing_model_source.clone(),
                requested_model: response.requested_model.clone(),
                upstream_model_id: response.upstream_model_id.clone(),
                upstream_url: response.upstream_url.clone(),
                status_code: Some(response.status_code),
                upstream_status_code: response.upstream_status_code,
                success,
                error_category: response.error_category.clone(),
                error_message: (!success).then(|| response.note.clone()),
                stream_outcome: (response.stream_outcome != GatewayStreamOutcome::NotStreaming)
                    .then_some(response.stream_outcome),
                duration_ms,
                attempt_count: response.provider_attempt_count.max(1),
                total_attempt_count: response.attempt_count.max(1),
                failover: response.failover,
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_creation_tokens,
                total_tokens,
                request_body_bytes: request.body.len() as u64,
                response_body_bytes: response.response_body_bytes,
                is_streaming: response.is_streaming,
                first_token_ms: response.first_token_ms,
                detail_file: None,
                detail_offset: None,
            },
            request_headers: None,
            request_body: None,
            upstream_request_body: None,
            response_headers: None,
            upstream_response_body: None,
            response_body: None,
            provider_attempts: response.provider_attempts.clone(),
        };

        let summary_outcome = if let Some(db) = context.db.as_ref() {
            match usage_stats::record_request_summary(db, &settings, &detail) {
                Ok(outcome) => Some(outcome),
                Err(error) => {
                    log::warn!("Failed to record proxy gateway request summary: {error}");
                    None
                }
            }
        } else {
            // No DB: still allow JSONL detail under the provisional stable/fallback id.
            Some(RecordRequestSummaryOutcome::Written {
                request_id: detail.summary.trace_id.clone(),
                collision: false,
            })
        };

        let Some(summary_outcome) = summary_outcome else {
            return;
        };

        match summary_outcome {
            RecordRequestSummaryOutcome::Skipped => {
                // Identical proxy semantic replay with existing detail: do not rewrite
                // JSONL, recount, or emit events.
            }
            RecordRequestSummaryOutcome::NeedsDetail { request_id } => {
                // Summary already exists; only retry JSONL detail attachment.
                detail.summary.trace_id = request_id;
                maybe_write_request_detail(
                    context,
                    paths,
                    &settings,
                    request,
                    response,
                    upstream_response_body_snapshot.as_ref(),
                    &mut detail,
                );
            }
            RecordRequestSummaryOutcome::Written {
                request_id,
                collision: _,
            } => {
                // Keep SQLite request_id and JSONL trace_id identical, including collision keys.
                detail.summary.trace_id = request_id;
                maybe_write_request_detail(
                    context,
                    paths,
                    &settings,
                    request,
                    response,
                    upstream_response_body_snapshot.as_ref(),
                    &mut detail,
                );
                emit_usage_recorded_event(context, &detail.summary);
            }
        }
    }
}

fn maybe_write_request_detail(
    context: &GatewayRuntimeContext,
    paths: &ProxyGatewayPaths,
    settings: &ProxyGatewaySettings,
    request: &DebugHttpRequest,
    response: &DebugHttpResponse,
    upstream_response_body_snapshot: Option<&(Vec<u8>, u64)>,
    detail: &mut GatewayRequestLogDetail,
) {
    if !settings.request_log_enabled {
        return;
    }

    detail.request_headers = settings
        .store_headers
        .then(|| request_log::redact_headers(&request.headers));
    detail.request_body = stored_body_text(
        &request.body,
        request.body.len() as u64,
        settings.store_request_body,
        settings.log_max_body_size_kb,
    );
    detail.upstream_request_body = response.upstream_request_body.as_deref().and_then(|body| {
        stored_body_text(
            body,
            body.len() as u64,
            settings.store_request_body,
            settings.log_max_body_size_kb,
        )
    });
    detail.response_headers = settings
        .store_headers
        .then(|| request_log::redact_headers(&response.headers));
    detail.upstream_response_body =
        upstream_response_body_snapshot.and_then(|(body, original_len)| {
            stored_body_text(
                body,
                *original_len,
                settings.store_response_body,
                settings.log_max_body_size_kb,
            )
        });
    detail.response_body = stored_body_text(
        &response.body,
        response.response_body_bytes,
        settings.store_response_body,
        settings.log_max_body_size_kb,
    );

    let record = request_log::new_request_log_record(detail.clone());
    match request_log::write_request_log(paths, settings, &record) {
        Ok(Some(location)) => {
            detail.summary.detail_file = Some(location.detail_file);
            detail.summary.detail_offset = Some(location.detail_offset);
            // Best-effort: attach locator to the existing SQLite summary row.
            if let Some(db) = context.db.as_ref() {
                if let Err(error) = usage_stats::update_request_detail_locator(
                    db,
                    &detail.summary.trace_id,
                    detail.summary.detail_file.as_deref(),
                    detail.summary.detail_offset,
                ) {
                    log::warn!("Failed to attach gateway detail locator to summary: {error}");
                }
            }
        }
        Ok(None) => {}
        Err(error) => {
            log::warn!("Failed to record proxy gateway request detail: {error}");
        }
    }
}

fn emit_usage_recorded_event(context: &GatewayRuntimeContext, summary: &GatewayRequestLogSummary) {
    let Some(app_handle) = context.app_handle.as_ref() else {
        return;
    };
    let Some(cli_key) = summary.cli_key else {
        return;
    };

    let payload = GatewayUsageRecordedEvent {
        cli_key: Some(cli_key),
        trace_id: Some(summary.trace_id.clone()),
        data_source: "proxy".to_string(),
        inserted_records: 1,
    };
    if let Err(error) = app_handle.emit("usage-log-recorded", payload) {
        log::warn!("Failed to emit gateway usage recorded event: {error}");
    }
}

fn should_skip_observability(method: &str, request_path: &str) -> bool {
    if method == "GET" && request_path == "/health" {
        return true;
    }
    matches!(method, "GET" | "HEAD")
        && matches!(
            request_path,
            "/anthropic" | "/openai/v1" | "/grok/v1" | "/gemini/v1beta"
        )
}

fn process_local_trace_id(request: &DebugHttpRequest) -> String {
    let run_id = TRACE_RUN_ID
        .get_or_init(|| format!("{}-{}", std::process::id(), Utc::now().timestamp_micros()));
    format!("gw-{}-{}", run_id, request.id)
}

fn is_success_status(status_code: u16) -> bool {
    (200..=399).contains(&status_code)
}

fn stored_body_text(
    body: &[u8],
    original_len: u64,
    enabled: bool,
    max_body_size_kb: u64,
) -> Option<String> {
    if !enabled {
        return None;
    }
    let max_bytes = max_body_size_kb.saturating_mul(1024) as usize;
    if max_bytes == 0 {
        return Some(String::new());
    }
    if original_len <= max_bytes as u64 {
        return Some(String::from_utf8_lossy(body).to_string());
    }
    let mut text = String::from_utf8_lossy(&body[..body.len().min(max_bytes)]).to_string();
    text.push_str(&format!(
        "\n[truncated: stored {} of {} bytes]",
        body.len().min(max_bytes),
        original_len
    ));
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_with_id(id: u64) -> DebugHttpRequest {
        DebugHttpRequest {
            id,
            method: "POST".to_string(),
            path: "/anthropic/v1/messages".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    #[test]
    fn process_local_trace_id_contains_process_run_prefix() {
        let trace = process_local_trace_id(&request_with_id(1));

        assert!(trace.starts_with("gw-"));
        assert!(trace.ends_with("-1"));
        assert_ne!(trace, "gw-1");
    }

    #[test]
    fn skips_cli_root_probe_observability() {
        assert!(should_skip_observability("HEAD", "/anthropic"));
        assert!(should_skip_observability("GET", "/openai/v1"));
        assert!(should_skip_observability("GET", "/grok/v1"));
        assert!(should_skip_observability("HEAD", "/grok/v1"));
        assert!(should_skip_observability("HEAD", "/gemini/v1beta"));
        assert!(!should_skip_observability("POST", "/anthropic"));
        assert!(!should_skip_observability("POST", "/anthropic/v1/messages"));
    }
}
