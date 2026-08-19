//! DSh (DeepSeek Harness) Web UI 打开 / `dsh web` 启动能力(对齐 hermes/openclaw)。
//!
//! DSh 的 web UI 监听本地端口(默认 3080,`DSH_WEB_PORT` 可覆盖),通过 `dsh web` 或
//! `npx @deepseek-ai/dsh web` 启动。与 hermes/openclaw 不同,dsh 根路径返回的状态码
//! 未经证实(未必是 200|401),故 `probe_web_up` 采用 socket-level liveness:任何 HTTP
//! 响应(2xx/3xx/4xx/5xx)即视为在线,只有连接错误/超时才视为离线——避免误判在线为离线
//! 后又因端口占用启动失败的死循环。

use std::process::Command;

/// 覆盖端口的 env 键。
pub const DSH_WEB_PORT_ENV: &str = "DSH_WEB_PORT";
/// DSh Web UI 默认端口。
pub const DSH_WEB_DEFAULT_PORT: u16 = 3080;

/// 解析 Web 端口:读 `DSH_WEB_PORT` env(非法/未设回落默认)。
pub fn resolve_web_port() -> u16 {
    std::env::var(DSH_WEB_PORT_ENV)
        .ok()
        .and_then(|raw| raw.trim().parse::<u16>().ok())
        .unwrap_or(DSH_WEB_DEFAULT_PORT)
}

/// 构造 DSh Web UI 完整 URL(`http://127.0.0.1:{port}` + 可选 path)。
pub fn build_web_url(port: u16, path: Option<&str>) -> String {
    let base = format!("http://127.0.0.1:{port}");
    match path {
        Some(p) if p.starts_with('/') => format!("{base}{p}"),
        Some(p) if !p.is_empty() => format!("{base}/{p}"),
        _ => format!("{base}/"),
    }
}

/// 探测 DSh 服务是否在线(socket-level liveness:任何 HTTP 响应即在线)。
///
/// dsh web 根路径状态码未经证实(未必是 200|401),故只要能拿到响应就视为在线,
/// 只有连接错误/超时才视为离线,避免把在线误判为离线后启动冲突死循环。
pub async fn probe_web_up(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{port}/");
    let Ok(client) = crate::http_client::create_client_no_proxy(2) else {
        return false;
    };
    client.get(&url).send().await.is_ok()
}

/// 用系统浏览器打开 DSh Web UI(调用前应先用 `probe_web_up` 确认为在线)。
pub fn open_web_ui_browser(port: u16, path: Option<&str>) -> Result<(), String> {
    let url = build_web_url(port, path);
    tauri_plugin_opener::open_url(url, None::<&str>)
        .map_err(|e| format!("打开 DSh Web UI 失败: {e}"))
}

/// 在用户终端里非阻塞启动 `dsh web` / `npx @deepseek-ai/dsh web`(DSh 的 web 进程)。
///
/// 启动前先用 `resolve_local_cli_by_name` 验证目标 CLI 可解析(含候选目录扫描,覆盖 GUI
/// 进程不继承终端 PATH 的场景);否则 `cmd /C start` 这类分层派生无论子进程是否真正起来
/// 都会返回 Ok,前端会误显示"启动成功"toast。
pub fn launch_dsh_web_in_terminal(use_npx: bool) -> Result<(), String> {
    let cli_name = if use_npx { "npx" } else { "dsh" };
    let program = crate::coding::cli_resolver::resolve_local_cli_by_name(cli_name)
        .ok_or_else(|| crate::coding::cli_resolver::local_cli_missing_hint(cli_name))?;
    // Manual overrides apply to the dsh binary itself; for `npx` we keep the
    // bare npx command because there is no per-tab npx override.
    let resolved_path = if use_npx {
        None
    } else {
        Some(program.path.as_path())
    };

    #[cfg(target_os = "windows")]
    {
        launch_windows_dashboard(use_npx, resolved_path)
    }
    #[cfg(target_os = "macos")]
    {
        launch_macos_dashboard(use_npx, resolved_path)
    }
    #[cfg(target_os = "linux")]
    {
        launch_linux_dashboard(use_npx, resolved_path)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("Unsupported platform for launching DSh dashboard".to_string())
    }
}

/// 启动命令串:`dsh web` 或 `npx @deepseek-ai/dsh web`。
///
/// `resolved_path` is the concrete CLI path resolved by `cli_resolver` (it
/// already honors user manual overrides). It is quoted because a path may
/// contain spaces.
fn dashboard_command(use_npx: bool, resolved_path: Option<&std::path::Path>) -> String {
    if use_npx {
        "npx @deepseek-ai/dsh web".to_string()
    } else if let Some(path) = resolved_path {
        format!("\"{}\" web", path.display())
    } else {
        "dsh web".to_string()
    }
}

#[cfg(target_os = "windows")]
fn launch_windows_dashboard(use_npx: bool, resolved_path: Option<&std::path::Path>) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let command = dashboard_command(use_npx, resolved_path);
    // `start` 第一个 `""` 是窗口标题;后续 `cmd /K "<cmd>"` 在新终端窗口内运行。
    Command::new("cmd")
        .args(["/C", "start", "", "cmd", "/K", &command])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("打开 DSh dashboard 失败: {e}"))
}

#[cfg(target_os = "macos")]
fn launch_macos_dashboard(use_npx: bool, resolved_path: Option<&std::path::Path>) -> Result<(), String> {
    let command = dashboard_command(use_npx, resolved_path)
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
        .map_err(|e| format!("打开 DSh dashboard 失败: {e}"))
}

#[cfg(any(target_os = "linux", test))]
fn escape_linux_double_quoted_shell_command(command: &str) -> String {
    command
        .replace('\\', "\\\\")
        .replace('$', "\\$")
        .replace('`', "\\`")
}

#[cfg(target_os = "linux")]
fn launch_linux_dashboard(use_npx: bool, resolved_path: Option<&std::path::Path>) -> Result<(), String> {
    let raw_command = dashboard_command(use_npx, resolved_path);
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
        let mut cmd = Command::new(terminal);
        cmd.args(&args);
        cmd.arg("sh").arg("-c").arg(&command);
        match cmd.spawn() {
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
        let raw_command = dashboard_command(
            false,
            Some(std::path::Path::new("/opt/DeepSeek Harness/bin/dsh")),
        );
        assert_eq!(
            escape_linux_double_quoted_shell_command(&raw_command),
            "\"/opt/DeepSeek Harness/bin/dsh\" web"
        );
    }

    #[test]
    fn build_web_url_defaults_to_root() {
        assert_eq!(build_web_url(3080, None), "http://127.0.0.1:3080/");
        assert_eq!(build_web_url(3080, Some("")), "http://127.0.0.1:3080/");
        assert_eq!(build_web_url(3080, Some("/")), "http://127.0.0.1:3080/");
    }

    #[test]
    fn build_web_url_keeps_leading_slash() {
        assert_eq!(build_web_url(3080, Some("/sessions")), "http://127.0.0.1:3080/sessions");
    }

    #[test]
    fn build_web_url_adds_slash_when_missing() {
        assert_eq!(build_web_url(3080, Some("settings")), "http://127.0.0.1:3080/settings");
        assert_eq!(build_web_url(8080, Some("health")), "http://127.0.0.1:8080/health");
    }

    #[test]
    fn resolve_web_port_defaults_when_unset_or_invalid() {
        let _guard = test_guard();
        let old = std::env::var_os(DSH_WEB_PORT_ENV);

        std::env::remove_var(DSH_WEB_PORT_ENV);
        assert_eq!(resolve_web_port(), DSH_WEB_DEFAULT_PORT);

        std::env::set_var(DSH_WEB_PORT_ENV, "not-a-port");
        assert_eq!(resolve_web_port(), DSH_WEB_DEFAULT_PORT);

        match old {
            Some(v) => std::env::set_var(DSH_WEB_PORT_ENV, v),
            None => std::env::remove_var(DSH_WEB_PORT_ENV),
        }
    }

    #[test]
    fn resolve_web_port_reads_env() {
        let _guard = test_guard();
        let old = std::env::var_os(DSH_WEB_PORT_ENV);

        std::env::set_var(DSH_WEB_PORT_ENV, "9999");
        assert_eq!(resolve_web_port(), 9999);

        match old {
            Some(v) => std::env::set_var(DSH_WEB_PORT_ENV, v),
            None => std::env::remove_var(DSH_WEB_PORT_ENV),
        }
    }

    #[test]
    fn dashboard_command_switches_with_use_npx() {
        assert_eq!(dashboard_command(false, None), "dsh web");
        assert_eq!(dashboard_command(true, None), "npx @deepseek-ai/dsh web");
        // 手动覆盖的具体 CLI 路径用引号包裹,避免含空格的路径被 shell 拆分。
        let resolved = std::path::Path::new("/usr/local/bin/dsh");
        assert_eq!(
            dashboard_command(false, Some(resolved)),
            "\"/usr/local/bin/dsh\" web"
        );
    }
}
