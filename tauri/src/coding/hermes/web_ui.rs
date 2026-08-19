//! Hermes Web UI 打开 / dashboard 启动能力(对齐 cc-switch)。
//!
//! Hermes 的 web dashboard 监听本地端口(默认 9119,`HERMES_WEB_PORT` 可覆盖),
//! 全部 `/api/*` 路由走 Bearer-token 中间件,因此 GET `/api/status` 返回 200 或
//! 401 都代表服务在线,只有连接错误/超时才视为离线。

use std::process::Command;

/// 覆盖端口的 env 键。
pub const HERMES_WEB_PORT_ENV: &str = "HERMES_WEB_PORT";
/// Hermes Web UI 默认端口。
pub const HERMES_WEB_DEFAULT_PORT: u16 = 9119;

/// 解析 Web 端口:读 `HERMES_WEB_PORT` env(非法/未设回落默认)。
pub fn resolve_web_port() -> u16 {
    std::env::var(HERMES_WEB_PORT_ENV)
        .ok()
        .and_then(|raw| raw.trim().parse::<u16>().ok())
        .unwrap_or(HERMES_WEB_DEFAULT_PORT)
}

/// 构造 Hermes Web UI 完整 URL(`http://127.0.0.1:{port}` + 可选 path)。
pub fn build_web_url(port: u16, path: Option<&str>) -> String {
    let base = format!("http://127.0.0.1:{port}");
    match path {
        Some(p) if p.starts_with('/') => format!("{base}{p}"),
        Some(p) if !p.is_empty() => format!("{base}/{p}"),
        _ => format!("{base}/"),
    }
}

/// 探测 Hermes 服务是否在线(`/api/status` 返回 200 或 401 即视为在线)。
pub async fn probe_web_up(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{port}/api/status");
    let Ok(client) = crate::http_client::create_client_no_proxy(2) else {
        return false;
    };
    match client.get(&url).send().await {
        Ok(response) => matches!(response.status().as_u16(), 200 | 401),
        Err(_) => false,
    }
}

/// 用系统浏览器打开 Hermes Web UI(调用前应先用 `probe_web_up` 确认为在线)。
pub fn open_web_ui_browser(port: u16, path: Option<&str>) -> Result<(), String> {
    let url = build_web_url(port, path);
    tauri_plugin_opener::open_url(url, None::<&str>)
        .map_err(|e| format!("打开 Hermes Web UI 失败: {e}"))
}

/// 在用户终端里非阻塞启动 `hermes dashboard`(Hermes 的 web dashboard 进程)。
///
/// 启动前先用 `where`/`which` 验证 `hermes` CLI 可解析;否则 `cmd /C start` 这类
/// 分层派生无论子进程是否真正起来都会返回 Ok,前端会误显示"启动成功"toast。
pub fn launch_dashboard_in_terminal() -> Result<(), String> {
    let program = crate::coding::cli_resolver::resolve_local_cli_by_name("hermes")
        .ok_or_else(|| crate::coding::cli_resolver::local_cli_missing_hint("hermes"))?;
    let resolved_path = program.path.as_path();

    #[cfg(target_os = "windows")]
    {
        launch_windows_dashboard(resolved_path)
    }
    #[cfg(target_os = "macos")]
    {
        launch_macos_dashboard(resolved_path)
    }
    #[cfg(target_os = "linux")]
    {
        launch_linux_dashboard(resolved_path)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("Unsupported platform for launching Hermes dashboard".to_string())
    }
}

#[cfg(target_os = "windows")]
fn launch_windows_dashboard(resolved_path: &std::path::Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let command = format!("\"{}\" dashboard", resolved_path.display());
    // `start` 第一个 `""` 是窗口标题;后续 `cmd /K "<cmd>"` 在新终端窗口内运行。
    Command::new("cmd")
        .args(["/C", "start", "", "cmd", "/K", &command])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("打开 Hermes dashboard 失败: {e}"))
}

#[cfg(target_os = "macos")]
fn launch_macos_dashboard(resolved_path: &std::path::Path) -> Result<(), String> {
    let command = format!("\"{}\" dashboard", resolved_path.display())
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let applescript = format!(
        r#"tell application "Terminal"
    activate
    do script "{command}"
end tell"#
    );
    Command::new("osascript")
        .arg("-e")
        .arg(applescript)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("打开 Hermes dashboard 失败: {e}"))
}

#[cfg(any(target_os = "linux", test))]
fn escape_linux_double_quoted_shell_command(command: &str) -> String {
    command
        .replace('\\', "\\\\")
        .replace('$', "\\$")
        .replace('`', "\\`")
}

#[cfg(target_os = "linux")]
fn launch_linux_dashboard(resolved_path: &std::path::Path) -> Result<(), String> {
    let raw_command = format!("\"{}\" dashboard", resolved_path.display());
    let command = escape_linux_double_quoted_shell_command(&raw_command);
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
        // Use `child` (not `command`) so the inner `Command` does not shadow
        // the outer `command: String` we pass to `sh -c` below — shadowing made
        // `arg(&command)` resolve to `&Command`, which does not impl `AsRef<OsStr>`.
        let mut child = Command::new(terminal);
        child.args(&args);
        child.arg("sh").arg("-c").arg(&command);
        match child.spawn() {
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
    fn linux_dashboard_command_keeps_executable_quotes() {
        let raw_command = format!(
            "\"{}\" dashboard",
            std::path::Path::new("/opt/Hermes Agent/bin/hermes").display()
        );
        assert_eq!(
            escape_linux_double_quoted_shell_command(&raw_command),
            "\"/opt/Hermes Agent/bin/hermes\" dashboard"
        );
    }

    #[test]
    fn build_web_url_defaults_to_root() {
        assert_eq!(build_web_url(9119, None), "http://127.0.0.1:9119/");
        assert_eq!(build_web_url(9119, Some("")), "http://127.0.0.1:9119/");
        assert_eq!(build_web_url(9119, Some("/")), "http://127.0.0.1:9119/");
    }

    #[test]
    fn build_web_url_keeps_leading_slash() {
        assert_eq!(build_web_url(9119, Some("/agents")), "http://127.0.0.1:9119/agents");
    }

    #[test]
    fn build_web_url_adds_slash_when_missing() {
        assert_eq!(build_web_url(9119, Some("memory")), "http://127.0.0.1:9119/memory");
        assert_eq!(build_web_url(8888, Some("health")), "http://127.0.0.1:8888/health");
    }

    #[test]
    fn resolve_web_port_defaults_when_unset_or_invalid() {
        let _guard = test_guard();
        let old = std::env::var_os(HERMES_WEB_PORT_ENV);

        std::env::remove_var(HERMES_WEB_PORT_ENV);
        assert_eq!(resolve_web_port(), HERMES_WEB_DEFAULT_PORT);

        std::env::set_var(HERMES_WEB_PORT_ENV, "not-a-port");
        assert_eq!(resolve_web_port(), HERMES_WEB_DEFAULT_PORT);

        match old {
            Some(v) => std::env::set_var(HERMES_WEB_PORT_ENV, v),
            None => std::env::remove_var(HERMES_WEB_PORT_ENV),
        }
    }

    #[test]
    fn resolve_web_port_reads_env() {
        let _guard = test_guard();
        let old = std::env::var_os(HERMES_WEB_PORT_ENV);

        std::env::set_var(HERMES_WEB_PORT_ENV, "9999");
        assert_eq!(resolve_web_port(), 9999);

        match old {
            Some(v) => std::env::set_var(HERMES_WEB_PORT_ENV, v),
            None => std::env::remove_var(HERMES_WEB_PORT_ENV),
        }
    }
}