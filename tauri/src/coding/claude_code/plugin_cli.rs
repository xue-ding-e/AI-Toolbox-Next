use crate::coding::runtime_location::{RuntimeLocationInfo, RuntimeLocationMode};
use tokio::process::Command;

fn build_claude_command(
    runtime_location: &RuntimeLocationInfo,
    args: &[&str],
) -> Result<Command, String> {
    match runtime_location.mode {
        RuntimeLocationMode::LocalWindows => {
            let mut command = Command::new("claude");
            command.args(args);
            command.env("CLAUDE_CONFIG_DIR", &runtime_location.host_path);
            Ok(command)
        }
        RuntimeLocationMode::WslDirect => {
            let wsl = runtime_location.wsl.as_ref().ok_or_else(|| {
                "Missing WSL runtime metadata for Claude plugin command".to_string()
            })?;
            let mut command = Command::new("wsl");
            command.args(["-d", &wsl.distro, "--exec", "env"]);
            command.arg(format!("CLAUDE_CONFIG_DIR={}", wsl.linux_path));
            command.arg("claude");
            command.args(args);
            Ok(command)
        }
    }
}

pub async fn run_claude_plugin_command(
    runtime_location: &RuntimeLocationInfo,
    args: &[&str],
) -> Result<(), String> {
    let output = build_claude_command(runtime_location, args)?
        .output()
        .await
        .map_err(|error| format!("Failed to run Claude plugin command: {}", error))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr_output = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout_output = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let error_message = if !stderr_output.is_empty() {
        stderr_output
    } else if !stdout_output.is_empty() {
        stdout_output
    } else {
        "Unknown Claude plugin command failure".to_string()
    };

    Err(error_message)
}
