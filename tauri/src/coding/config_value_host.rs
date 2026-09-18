//! Environment a provider config value belongs to.
//!
//! Pi and OMP both let a provider's `apiKey` / header values carry `!command`
//! values, and both run that command in the tool's own runtime. A WSL Direct
//! runtime therefore has to resolve values inside its target distribution
//! instead of against the desktop process environment.

use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::coding::cli_resolver;
use crate::coding::runtime_location::{RuntimeLocationInfo, RuntimeLocationMode};

/// Command timeout both CLIs use for a `!command` config value.
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

/// Environment a config value is resolved against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValueHost {
    /// The tool runs against the host machine, so `!cmd` uses the host env.
    Local,
    /// The tool runs inside WSL Direct, so values must be resolved in that distro.
    Wsl { distro: String },
}

/// Host matching a resolved runtime location.
pub fn config_value_host_from_location(location: &RuntimeLocationInfo) -> ConfigValueHost {
    match (&location.mode, &location.wsl) {
        (RuntimeLocationMode::WslDirect, Some(wsl)) => ConfigValueHost::Wsl {
            distro: wsl.distro.clone(),
        },
        _ => ConfigValueHost::Local,
    }
}

/// Argument vector for running `program args` inside a WSL distribution.
///
/// Kept separate so the distro, program and command stay distinct process
/// arguments instead of being interpolated into a shell string.
fn wsl_exec_arguments(distro: &str, program: &str, args: &[&str]) -> Vec<String> {
    let mut arguments = vec![
        "-d".to_string(),
        distro.to_string(),
        "--exec".to_string(),
        program.to_string(),
    ];
    arguments.extend(args.iter().map(|arg| arg.to_string()));
    arguments
}

pub fn wsl_command(distro: &str, program: &str, args: &[&str]) -> Command {
    let mut command = Command::new("wsl");
    cli_resolver::apply_create_no_window_tokio(&mut command);
    command.args(wsl_exec_arguments(distro, program, args));
    command
}

/// Read one environment variable from the resolution host.
///
/// `printenv` exits non-zero when the name is unset, which keeps "unset" and
/// "set to an empty string" distinguishable the way both CLIs treat them.
pub async fn lookup_env_var(name: &str, host: &ConfigValueHost) -> Option<String> {
    let raw_value = match host {
        ConfigValueHost::Local => std::env::var(name).ok()?,
        ConfigValueHost::Wsl { distro } => {
            let mut wsl = wsl_command(distro, "printenv", &[name]);
            wsl.stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            let output = tokio::time::timeout(COMMAND_TIMEOUT, wsl.output())
                .await
                .ok()?
                .ok()?;
            if !output.status.success() {
                return None;
            }
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            stdout
                .strip_suffix('\n')
                .map(|value| value.strip_suffix('\r').unwrap_or(value))
                .unwrap_or(stdout.as_str())
                .to_string()
        }
    };

    Some(raw_value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding::runtime_location::WslLocationInfo;

    fn location(mode: RuntimeLocationMode, wsl: Option<WslLocationInfo>) -> RuntimeLocationInfo {
        RuntimeLocationInfo {
            mode,
            source: "test".to_string(),
            host_path: std::path::PathBuf::from("C:/root"),
            wsl,
        }
    }

    #[test]
    fn wsl_execution_keeps_distro_and_command_as_separate_arguments() {
        let arguments = wsl_exec_arguments(
            "Ubuntu",
            "bash",
            &["-c", "printf '%s' \"$MY_KEY\"; echo injected"],
        );

        assert_eq!(
            arguments,
            vec![
                "-d",
                "Ubuntu",
                "--exec",
                "bash",
                "-c",
                "printf '%s' \"$MY_KEY\"; echo injected"
            ]
        );
    }

    #[test]
    fn runtime_location_picks_the_matching_host() {
        assert_eq!(
            config_value_host_from_location(&location(RuntimeLocationMode::LocalWindows, None)),
            ConfigValueHost::Local
        );

        // A WSL Direct location without distro details cannot be targeted.
        assert_eq!(
            config_value_host_from_location(&location(RuntimeLocationMode::WslDirect, None)),
            ConfigValueHost::Local
        );

        assert_eq!(
            config_value_host_from_location(&location(
                RuntimeLocationMode::WslDirect,
                Some(WslLocationInfo {
                    distro: "Ubuntu".to_string(),
                    linux_path: "/root".to_string(),
                    linux_user_root: Some("/root".to_string()),
                }),
            )),
            ConfigValueHost::Wsl {
                distro: "Ubuntu".to_string()
            }
        );
    }
}
