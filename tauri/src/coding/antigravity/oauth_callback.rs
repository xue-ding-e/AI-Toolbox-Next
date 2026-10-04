use std::io;
use std::net::TcpListener as StdTcpListener;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

const DEFAULT_PORT: u16 = 8086;
const CALLBACK_PATH: &str = "/oauth2callback";
const MAX_REQUEST_BYTES: usize = 16 * 1024;
const MAX_CONNECTIONS: usize = 16;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn redirect_uri(port: u16) -> String {
    format!("http://localhost:{port}{CALLBACK_PATH}")
}

// Google installed-app OAuth permits a dynamic loopback port. Windows may
// reserve the entire preferred range (e.g. for Hyper-V/WSL), so scanning only
// adjacent ports is insufficient. Port 0 asks the OS for an available port.
fn bind_with(
    mut bind: impl FnMut(u16) -> io::Result<StdTcpListener>,
) -> io::Result<StdTcpListener> {
    for port in DEFAULT_PORT..DEFAULT_PORT + 20 {
        if let Ok(listener) = bind(port) {
            return Ok(listener);
        }
    }
    bind(0)
}

pub(super) fn bind_listener() -> Result<(TcpListener, u16), String> {
    let listener = bind_with(|port| StdTcpListener::bind(("127.0.0.1", port))).map_err(|error| {
        format!(
            "Failed to bind local Antigravity OAuth listener (preferred ports 8086-8105 and OS-assigned port): {error}"
        )
    })?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("Failed to read OAuth callback port: {error}"))?
        .port();
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Failed to configure OAuth listener: {error}"))?;
    let listener = TcpListener::from_std(listener)
        .map_err(|error| format!("Failed to initialize OAuth listener: {error}"))?;
    Ok((listener, port))
}

async fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    // Plain text prevents provider-controlled error text from becoming HTML.
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

async fn handle_connection(
    mut stream: TcpStream,
    expected_state: &str,
) -> Option<Result<String, String>> {
    let mut request = Vec::new();
    let mut buffer = [0; 2048];
    // TCP reads are not HTTP message boundaries. Read a complete header before
    // parsing, with a size limit and an outer per-connection deadline.
    loop {
        let size = stream.read(&mut buffer).await.ok()?;
        if size == 0 {
            return None;
        }
        request.extend_from_slice(&buffer[..size]);
        if request.len() > MAX_REQUEST_BYTES {
            respond(
                &mut stream,
                "431 Request Header Fields Too Large",
                "Request too large",
            )
            .await;
            return None;
        }
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let request = std::str::from_utf8(&request).ok()?;
    let mut parts = request.lines().next()?.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    if method != "GET" {
        respond(&mut stream, "405 Method Not Allowed", "Expected GET").await;
        return None;
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != CALLBACK_PATH {
        respond(&mut stream, "404 Not Found", "Not Found").await;
        return None;
    }
    let params: Vec<_> = url::form_urlencoded::parse(query.as_bytes()).collect();
    // Ambiguous callbacks must not select a value by parameter order.
    let unique = |key: &str| {
        let mut values = params.iter().filter(|(name, _)| name == key);
        let (_, value) = values.next()?;
        values.next().is_none().then_some(value.as_ref())
    };
    if unique("state") != Some(expected_state) {
        respond(
            &mut stream,
            "400 Bad Request",
            "Authentication failed: invalid state parameter.",
        )
        .await;
        // An unrelated/stale callback must not cancel the active login.
        return None;
    }
    if let Some(error) = unique("error") {
        respond(
            &mut stream,
            "400 Bad Request",
            "Authentication was not authorized. Return to AI Toolbox.",
        )
        .await;
        return Some(Err(format!("OAuth provider error: {error}")));
    }
    let Some(code) = unique("code").filter(|value| !value.trim().is_empty()) else {
        respond(
            &mut stream,
            "400 Bad Request",
            "Missing or ambiguous authorization code.",
        )
        .await;
        return None;
    };
    let code = code.to_string();
    respond(
        &mut stream,
        "200 OK",
        "Authorization received. Return to AI Toolbox to finish signing in.",
    )
    .await;
    Some(Ok(code))
}

pub(super) async fn capture_callback(
    listener: TcpListener,
    expected_state: &str,
    timeout: Duration,
) -> Result<String, String> {
    let mut connections = JoinSet::new();
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => return Err("Timed out waiting for OAuth callback".to_string()),
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(Ok(Some(result))) = result {
                    return result;
                }
            }
            accepted = listener.accept(), if connections.len() < MAX_CONNECTIONS => {
                let (stream, _) = accepted
                    .map_err(|error| format!("Failed to accept OAuth callback: {error}"))?;
                let state = expected_state.to_string();
                connections.spawn(async move {
                    tokio::time::timeout(CONNECTION_TIMEOUT, handle_connection(stream, &state))
                        .await
                        .ok()
                        .flatten()
                });
            }
        }
    }
    // Dropping JoinSet aborts incomplete connections on success, timeout or
    // cancellation. No spawn_blocking task can keep a port alive indefinitely.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_or_occupied_preferred_ports_fall_back_to_os_assigned_port() {
        for kind in [io::ErrorKind::PermissionDenied, io::ErrorKind::AddrInUse] {
            let mut attempts = Vec::new();
            let listener = bind_with(|port| {
                attempts.push(port);
                if port == 0 {
                    StdTcpListener::bind(("127.0.0.1", 0))
                } else {
                    Err(io::Error::from(kind))
                }
            })
            .unwrap();
            assert_eq!(attempts, (8086..8106).chain([0]).collect::<Vec<_>>());
            assert_ne!(listener.local_addr().unwrap().port(), 0);
            assert!(listener.local_addr().unwrap().ip().is_loopback());
        }
    }

    #[test]
    fn failure_to_bind_even_dynamic_port_is_reported() {
        let error =
            bind_with(|_| Err(io::Error::from(io::ErrorKind::PermissionDenied))).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    async fn listener() -> (TcpListener, std::net::SocketAddr) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        (listener, addr)
    }

    async fn request(addr: std::net::SocketAddr, target: &str) -> String {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut response = String::new();
        tokio::time::timeout(Duration::from_secs(2), stream.read_to_string(&mut response))
            .await
            .unwrap()
            .unwrap();
        response
    }

    #[tokio::test]
    async fn actual_machine_bind_and_callback_use_selected_port() {
        // This also exercises Windows excluded port ranges on real hosts.
        let (listener, port) = bind_listener().unwrap();
        let uri = url::Url::parse(&redirect_uri(port)).unwrap();
        assert_eq!(uri.port(), Some(listener.local_addr().unwrap().port()));
        assert_eq!(uri.path(), CALLBACK_PATH);
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            capture_callback(listener, "state", Duration::from_secs(3)).await
        });
        let response = request(addr, "/oauth2callback?state=state&code=code%2Bvalue").await;
        assert!(response.starts_with("HTTP/1.1 200"));
        assert_eq!(task.await.unwrap().unwrap(), "code+value");
    }

    #[tokio::test]
    async fn idle_listener_and_idle_connection_obey_overall_timeout() {
        for connect in [false, true] {
            let (listener, addr) = listener().await;
            let _stream = if connect {
                Some(TcpStream::connect(addr).await.unwrap())
            } else {
                None
            };
            let result = tokio::time::timeout(
                Duration::from_secs(2),
                capture_callback(listener, "state", Duration::from_millis(50)),
            )
            .await
            .expect("callback must actually time out");
            assert!(result.unwrap_err().contains("Timed out"));
        }
    }

    #[tokio::test]
    async fn stale_and_malformed_requests_do_not_abort_valid_fragmented_callback() {
        let (listener, addr) = listener().await;
        let task = tokio::spawn(async move {
            capture_callback(listener, "state", Duration::from_secs(5)).await
        });
        // Browser preconnect must not block the actual callback.
        let _idle = TcpStream::connect(addr).await.unwrap();
        for target in [
            "/favicon.ico",
            "/oauth2callback?state=old&code=stale",
            "/oauth2callback?state=state&state=other&code=stale",
            "/oauth2callback?state=state&code=",
            "/oauth2callback?state=state&code=one&code=two",
        ] {
            assert!(!request(addr, target).await.starts_with("HTTP/1.1 200"));
        }
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(b"GET /oauth2callback?state=state&co")
            .await
            .unwrap();
        tokio::task::yield_now().await;
        stream
            .write_all(b"de=valid HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        assert_eq!(task.await.unwrap().unwrap(), "valid");
    }

    #[tokio::test]
    async fn provider_denial_finishes_login_without_rendering_untrusted_html() {
        let (listener, addr) = listener().await;
        let task = tokio::spawn(async move {
            capture_callback(listener, "state", Duration::from_secs(3)).await
        });
        let response = request(addr, "/oauth2callback?state=state&error=%3Cscript%3E").await;
        assert!(response.contains("Content-Type: text/plain"));
        assert!(!response.contains("<script>"));
        assert!(task
            .await
            .unwrap()
            .unwrap_err()
            .contains("OAuth provider error"));
    }
}
