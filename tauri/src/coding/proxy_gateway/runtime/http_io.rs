use crate::coding::proxy_gateway::types::GatewayCliKey;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

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
    pub(super) cli_key: Option<GatewayCliKey>,
    pub(super) route_name: String,
    pub(super) provider_id: Option<String>,
    pub(super) provider_name: Option<String>,
    pub(super) requested_model: Option<String>,
    pub(super) upstream_model_id: Option<String>,
    pub(super) upstream_url: Option<String>,
    pub(super) error_category: Option<String>,
    pub(super) attempt_count: u32,
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
        body,
        cli_key: None,
        route_name: route_name.to_string(),
        provider_id: None,
        provider_name: None,
        requested_model: None,
        upstream_model_id: None,
        upstream_url,
        error_category: None,
        attempt_count: 0,
        failover: false,
        note: note.to_string(),
    }
}

pub(super) fn write_response(
    stream: &mut TcpStream,
    response: &DebugHttpResponse,
) -> std::io::Result<()> {
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
