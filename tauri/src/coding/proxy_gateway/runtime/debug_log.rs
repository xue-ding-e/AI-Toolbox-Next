use super::http_io::{DebugHttpRequest, DebugHttpResponse};
use super::providers::UpstreamProvider;
use crate::coding::proxy_gateway::request_log;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;

pub(super) fn log_incoming_request(request: &DebugHttpRequest) {
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
            request.id,
            name,
            format_header_text_value_for_debug(name, value)
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

pub(super) fn format_body_for_debug_log(body: &[u8]) -> String {
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

pub(super) fn log_gateway_decision(request: &DebugHttpRequest, response: &DebugHttpResponse) {
    println!(
        "[proxy-gateway] route_decision id={} route={} upstream={} note={}",
        request.id,
        response.route_name,
        response.upstream_url.as_deref().unwrap_or("<none>"),
        response.note
    );
}

pub(super) fn log_response(request: &DebugHttpRequest, response: &DebugHttpResponse) {
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
            request.id,
            name,
            format_header_text_value_for_debug(name, value)
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

pub(super) fn log_upstream_request(
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

pub(super) fn log_upstream_response(request: &DebugHttpRequest, response: &DebugHttpResponse) {
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
            request.id,
            name,
            format_header_text_value_for_debug(name, value)
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
    format_header_text_value_for_debug(name, value)
}

fn format_header_text_value_for_debug(name: &str, value: &str) -> String {
    if is_sensitive_header(name) {
        mask_secret(value)
    } else {
        value.to_string()
    }
}

fn is_sensitive_header(name: &str) -> bool {
    request_log::is_sensitive_header(&name.to_ascii_lowercase())
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
