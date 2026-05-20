use crate::coding::proxy_gateway::types::{GatewayCliKey, ProxyGatewaySettings};
use crate::coding::proxy_gateway::usage_parser::{SseUsageCollector, TokenUsage};
use futures_util::Stream;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::pin::Pin;
use std::time::{Duration, Instant};

pub(super) type DebugBodyStream =
    Pin<Box<dyn Stream<Item = Result<Vec<u8>, String>> + Send + 'static>>;

#[derive(Debug)]
pub(super) struct DebugHttpRequest {
    pub(super) id: u64,
    pub(super) peer_addr: SocketAddr,
    pub(super) method: String,
    pub(super) path: String,
    pub(super) version: String,
    pub(super) first_line: String,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: Vec<u8>,
    pub(super) raw_len: usize,
}

pub(super) struct DebugHttpResponse {
    pub(super) status_code: u16,
    pub(super) status_text: String,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: Vec<u8>,
    pub(super) body_stream: Option<DebugBodyStream>,
    pub(super) response_body_bytes: u64,
    pub(super) token_usage: TokenUsage,
    pub(super) first_token_ms: Option<u64>,
    pub(super) is_streaming: bool,
    pub(super) cli_key: Option<GatewayCliKey>,
    pub(super) route_name: String,
    pub(super) provider_id: Option<String>,
    pub(super) provider_name: Option<String>,
    pub(super) requested_model: Option<String>,
    pub(super) upstream_model_id: Option<String>,
    pub(super) upstream_request_body: Option<Vec<u8>>,
    pub(super) upstream_url: Option<String>,
    pub(super) error_category: Option<String>,
    pub(super) attempt_count: u32,
    pub(super) provider_attempt_count: u32,
    pub(super) failover: bool,
    pub(super) note: String,
}

pub(super) fn read_http_request(
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

pub(super) fn json_response(
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
        response_body_bytes: body.len() as u64,
        body,
        body_stream: None,
        token_usage: TokenUsage::default(),
        first_token_ms: None,
        is_streaming: false,
        cli_key: None,
        route_name: route_name.to_string(),
        provider_id: None,
        provider_name: None,
        requested_model: None,
        upstream_model_id: None,
        upstream_request_body: None,
        upstream_url,
        error_category: None,
        attempt_count: 0,
        provider_attempt_count: 0,
        failover: false,
        note: note.to_string(),
    }
}

pub(super) fn write_response(
    stream: &mut TcpStream,
    response: &mut DebugHttpResponse,
    started_instant: Instant,
    settings: &ProxyGatewaySettings,
) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {} {}\r\n",
        response.status_code, response.status_text
    )?;
    let mut has_content_length = false;
    let mut has_connection = false;
    let streaming = response.body_stream.is_some();
    for (name, value) in &response.headers {
        if streaming
            && (name.eq_ignore_ascii_case("content-length")
                || name.eq_ignore_ascii_case("transfer-encoding"))
        {
            continue;
        }
        if name.eq_ignore_ascii_case("content-length") {
            has_content_length = true;
        }
        if name.eq_ignore_ascii_case("connection") {
            has_connection = true;
        }
        write!(stream, "{}: {}\r\n", name, value)?;
    }
    if streaming {
        write!(stream, "Transfer-Encoding: chunked\r\n")?;
    } else if !has_content_length {
        write!(stream, "Content-Length: {}\r\n", response.body.len())?;
    }
    if !has_connection {
        write!(stream, "Connection: close\r\n")?;
    }
    write!(stream, "\r\n")?;
    if streaming {
        write_streaming_body(stream, response, started_instant, settings)?;
    } else {
        stream.write_all(&response.body)?;
    }
    stream.flush()
}

fn write_streaming_body(
    stream: &mut TcpStream,
    response: &mut DebugHttpResponse,
    started_instant: Instant,
    settings: &ProxyGatewaySettings,
) -> std::io::Result<()> {
    let mut body_stream = match response.body_stream.take() {
        Some(body_stream) => body_stream,
        None => return Ok(()),
    };
    let mut usage_collector = response.cli_key.map(|_| SseUsageCollector::default());
    response.response_body_bytes = 0;
    response.body.clear();

    loop {
        let next_chunk = tauri::async_runtime::block_on(async {
            use futures_util::StreamExt;
            body_stream.next().await
        });
        let Some(chunk_result) = next_chunk else {
            break;
        };
        let chunk =
            chunk_result.map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
        if chunk.is_empty() {
            continue;
        }
        if response.first_token_ms.is_none() {
            response.first_token_ms = Some(
                started_instant
                    .elapsed()
                    .as_millis()
                    .min(u128::from(u64::MAX)) as u64,
            );
        }
        response.response_body_bytes = response
            .response_body_bytes
            .saturating_add(chunk.len() as u64);
        if let Some(collector) = usage_collector.as_mut() {
            collector.push_chunk(&chunk);
        }
        append_body_snapshot(response, &chunk, settings);
        write!(stream, "{:X}\r\n", chunk.len())?;
        stream.write_all(&chunk)?;
        write!(stream, "\r\n")?;
        stream.flush()?;
    }

    write!(stream, "0\r\n\r\n")?;
    if let (Some(cli_key), Some(collector)) = (response.cli_key, usage_collector) {
        response.token_usage = collector.finish(cli_key);
    }
    Ok(())
}

fn append_body_snapshot(
    response: &mut DebugHttpResponse,
    chunk: &[u8],
    settings: &ProxyGatewaySettings,
) {
    if !settings.store_response_body {
        return;
    }
    let max_bytes = settings.log_max_body_size_kb.saturating_mul(1024) as usize;
    if max_bytes == 0 || response.body.len() >= max_bytes {
        return;
    }
    let remaining = max_bytes.saturating_sub(response.body.len());
    response
        .body
        .extend_from_slice(&chunk[..chunk.len().min(remaining)]);
}

pub(super) fn find_header_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
        .or_else(|| {
            raw.windows(2)
                .position(|window| window == b"\n\n")
                .map(|index| index + 2)
        })
}

pub(super) fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}
