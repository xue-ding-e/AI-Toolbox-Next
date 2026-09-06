//! OpenClaw Control UI 打开 / gateway 启动能力。
//!
//! OpenClaw 是常驻 gateway,其 Web UI(Control UI)监听本地端口(默认 18789,
//! 端口优先级 `--port` → `OPENCLAW_GATEWAY_PORT` → `gateway.port` → 默认)。
//! 启动服务 = 在终端执行 `openclaw gateway`。
//! 探测在线:GET 根路径未认证返回 200(或 API 端点返 401)。200/401 均视为在线。

use std::process::Command;

/// 覆盖端口的 env 键。
pub const OPENCLAW_GATEWAY_PORT_ENV: &str = "OPENCLAW_GATEWAY_PORT";
/// OpenClaw Control UI 默认端口。
pub const OPENCLAW_DEFAULT_PORT: u16 = 18789;

/// 解析端口:env > 配置项 > 默认(纯函数,`config_port` 由命令层从 openclaw.json 解析)。
pub fn resolve_web_port(config_port: Option<u16>) -> u16 {
    std::env::var(OPENCLAW_GATEWAY_PORT_ENV)
        .ok()
        .and_then(|raw| raw.trim().parse::<u16>().ok())
        .or(config_port)
        .unwrap_or(OPENCLAW_DEFAULT_PORT)
}

/// 构造 Control UI 完整 URL(`http://127.0.0.1:{port}` + 可选 path)。
pub fn build_web_url(port: u16, path: Option<&str>) -> String {
    let base = format!("http://127.0.0.1:{port}");
    match path {
        Some(p) if p.starts_with('/') => format!("{base}{p}"),
        Some(p) if !p.is_empty() => format!("{base}/{p}"),
        _ => format!("{base}/"),
    }
}

/// 探测网关是否在线(根路径 200 或 401 均视为在线)。
pub async fn probe_web_up(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{port}/");
    let Ok(client) = crate::http_client::create_client_no_proxy(2) else {
        return false;
    };
    match client.get(&url).send().await {
        Ok(response) => matches!(response.status().as_u16(), 200 | 401),
        Err(_) => false,
    }
}

/// 用系统浏览器打开 Control UI(调用前应先用 `probe_web_up` 确认为在线)。
pub fn open_browser(port: u16, path: Option<&str>) -> Result<(), String> {
    let url = build_web_url(port, path);
    tauri_plugin_opener::open_url(url, None::<&str>)
        .map_err(|e| format!("打开 OpenClaw Control UI 失败: {e}"))
}

/// 在用户终端里非阻塞启动 `openclaw gateway`(OpenClaw 常驻服务)。
///
/// 启动前先用 `where`/`which` 验证 `openclaw` CLI 可解析;否则 `cmd /C start` 这类
/// 分层派生无论子进程是否真正起来都会返回 Ok,前端会误显示"启动成功"toast。
pub fn launch_gateway_in_terminal() -> Result<(), String> {
    if crate::coding::cli_resolver::resolve_local_cli_by_name("openclaw").is_none() {
        return Err(crate::coding::cli_resolver::local_cli_missing_hint(
            "openclaw",
        ));
    }

    #[cfg(target_os = "windows")]
    {
        launch_windows_gateway()
    }
    #[cfg(target_os = "macos")]
    {
        launch_macos_gateway()
    }
    #[cfg(target_os = "linux")]
    {
        launch_linux_gateway()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("Unsupported platform for launching OpenClaw gateway".to_string())
    }
}

#[cfg(target_os = "windows")]
fn launch_windows_gateway() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    Command::new("cmd")
        .args(["/C", "start", "", "cmd", "/K", "openclaw gateway"])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("打开 OpenClaw gateway 失败: {e}"))
}

#[cfg(target_os = "macos")]
fn launch_macos_gateway() -> Result<(), String> {
    let applescript = r#"tell application "Terminal"
    activate
    do script "openclaw gateway"
end tell"#;
    Command::new("osascript")
        .arg("-e")
        .arg(applescript)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("打开 OpenClaw gateway 失败: {e}"))
}

#[cfg(target_os = "linux")]
fn launch_linux_gateway() -> Result<(), String> {
    let terminals = [
        ("gnome-terminal", vec!["--".to_string()]),
        ("konsole", vec!["-e".to_string()]),
        ("xfce4-terminal", vec!["-e".to_string()]),
        ("x-terminal-emulator", vec!["-e".to_string()]),
        ("alacritty", vec!["-e".to_string()]),
        ("kitty", vec!["-e".to_string()]),
    ];

    let mut last_error = String::from("No usable terminal found");
    for (terminal, args) in terminals {
        let mut command = Command::new(terminal);
        command.args(&args);
        command.arg("sh").arg("-c").arg("openclaw gateway");
        match command.spawn() {
            Ok(_) => return Ok(()),
            Err(e) => last_error = format!("打开 {terminal} 失败: {e}"),
        }
    }
    Err(last_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    #[test]
    fn build_web_url_defaults_to_root() {
        assert_eq!(build_web_url(18789, None), "http://127.0.0.1:18789/");
        assert_eq!(build_web_url(18789, Some("")), "http://127.0.0.1:18789/");
        assert_eq!(build_web_url(18789, Some("/")), "http://127.0.0.1:18789/");
    }

    #[test]
    fn build_web_url_keeps_leading_slash() {
        assert_eq!(
            build_web_url(18789, Some("/sessions")),
            "http://127.0.0.1:18789/sessions"
        );
    }

    #[test]
    fn build_web_url_adds_slash_when_missing() {
        assert_eq!(
            build_web_url(18789, Some("chat")),
            "http://127.0.0.1:18789/chat"
        );
        assert_eq!(
            build_web_url(19001, Some("plugins")),
            "http://127.0.0.1:19001/plugins"
        );
    }

    #[test]
    fn resolve_web_port_prefers_env() {
        let _guard = test_guard();
        let old = std::env::var_os(OPENCLAW_GATEWAY_PORT_ENV);
        std::env::set_var(OPENCLAW_GATEWAY_PORT_ENV, "9999");
        assert_eq!(resolve_web_port(Some(5000)), 9999);
        assert_eq!(resolve_web_port(None), 9999);
        match old {
            Some(v) => std::env::set_var(OPENCLAW_GATEWAY_PORT_ENV, v),
            None => std::env::remove_var(OPENCLAW_GATEWAY_PORT_ENV),
        }
    }

    #[test]
    fn resolve_web_port_uses_config_then_default() {
        let _guard = test_guard();
        let old = std::env::var_os(OPENCLAW_GATEWAY_PORT_ENV);
        std::env::remove_var(OPENCLAW_GATEWAY_PORT_ENV);

        assert_eq!(resolve_web_port(Some(5000)), 5000);
        assert_eq!(resolve_web_port(None), OPENCLAW_DEFAULT_PORT);

        match old {
            Some(v) => std::env::set_var(OPENCLAW_GATEWAY_PORT_ENV, v),
            None => std::env::remove_var(OPENCLAW_GATEWAY_PORT_ENV),
        }
    }

    #[test]
    fn resolve_web_port_ignores_invalid_env() {
        let _guard = test_guard();
        let old = std::env::var_os(OPENCLAW_GATEWAY_PORT_ENV);
        std::env::set_var(OPENCLAW_GATEWAY_PORT_ENV, "not-a-port");
        assert_eq!(resolve_web_port(Some(5000)), 5000);
        assert_eq!(resolve_web_port(None), OPENCLAW_DEFAULT_PORT);
        match old {
            Some(v) => std::env::set_var(OPENCLAW_GATEWAY_PORT_ENV, v),
            None => std::env::remove_var(OPENCLAW_GATEWAY_PORT_ENV),
        }
    }
}
