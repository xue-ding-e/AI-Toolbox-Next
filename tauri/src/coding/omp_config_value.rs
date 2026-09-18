//! OMP-style config value resolution for values stored in `models.yml`.
//!
//! Mirrors `packages/coding-agent/src/config/model-config-values.ts` from OMP:
//! a provider `apiKey` / header value is either a `!command` or a value that is
//! looked up as an exact environment variable name first and used as a literal
//! second. Unlike Pi there is no `$VAR` interpolation, and an unresolvable value
//! is omitted instead of failing the request, so the shared model-discovery and
//! connectivity commands behave the way the OMP runtime would.
//!
//! Unlike the CLI, these one-shot diagnostics resolve per request instead of
//! caching a successful command for the process lifetime, so a rotated
//! credential is picked up on the next request.

use std::ffi::OsStr;
use std::process::Stdio;

use serde_json::{Map, Value};
use tokio::process::Command;

#[cfg(target_os = "windows")]
use crate::coding::cli_resolver;
use crate::coding::config_value_host::{
    lookup_env_var, wsl_command, ConfigValueHost, COMMAND_TIMEOUT,
};

/// Exact-case environment lookup mirroring OMP's `$envExact`.
///
/// Windows environment lookups are case-insensitive, so a literal value such as
/// `public` would otherwise be hijacked by the system `PUBLIC` variable. OMP only
/// trusts a variable whose stored name matches the configured value exactly.
fn lookup_local_env_var(name: &str) -> Option<String> {
    std::env::vars_os()
        .find(|(key, _)| key.as_os_str() == OsStr::new(name))
        .and_then(|(_, value)| value.into_string().ok())
}

#[cfg(target_os = "windows")]
fn local_shell_command(command: &str) -> Command {
    // OMP executes `!command` through Node's `execSync`, which uses cmd.exe.
    let mut shell = Command::new("cmd");
    cli_resolver::apply_create_no_window_tokio(&mut shell);
    shell.args(["/d", "/s", "/c", command]);
    shell
}

#[cfg(not(target_os = "windows"))]
fn local_shell_command(command: &str) -> Command {
    // `execSync` uses `/bin/sh` outside Windows.
    let mut shell = Command::new("/bin/sh");
    shell.arg("-c").arg(command);
    shell
}

async fn run_shell_command(command: &str, host: &ConfigValueHost) -> Option<String> {
    let mut shell = match host {
        ConfigValueHost::Local => local_shell_command(command),
        ConfigValueHost::Wsl { distro } => wsl_command(distro, "/bin/sh", &["-c", command]),
    };
    shell
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let output = tokio::time::timeout(COMMAND_TIMEOUT, shell.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}

/// Resolve one OMP config value the way OMP's `resolveConfigValue` does.
///
/// `!command` yields trimmed stdout, or nothing when the command fails, times
/// out or prints nothing. Every other value is the environment variable value
/// when that exact variable is set and non-empty, and a literal otherwise.
pub async fn resolve_config_value(raw: &str, host: &ConfigValueHost) -> Option<String> {
    if let Some(command) = raw.strip_prefix('!') {
        return run_shell_command(command.trim(), host).await;
    }

    let env_value = match host {
        ConfigValueHost::Local => lookup_local_env_var(raw),
        ConfigValueHost::Wsl { .. } => lookup_env_var(raw, host).await,
    };
    match env_value {
        Some(value) if !value.is_empty() => Some(value),
        _ => Some(raw.to_string()),
    }
}

/// Resolve every string header value with the same rules as API keys.
///
/// Mirrors OMP's `resolveConfigHeaders`, which drops a header whose value
/// resolves to nothing; non-string values are kept because the caller already
/// skips them when building the request.
pub async fn resolve_header_values(
    headers: &Map<String, Value>,
    host: &ConfigValueHost,
) -> Map<String, Value> {
    let mut resolved = Map::with_capacity(headers.len());

    for (key, value) in headers {
        let Some(raw) = value.as_str() else {
            resolved.insert(key.clone(), value.clone());
            continue;
        };
        if let Some(value) = resolve_config_value(raw, host)
            .await
            .filter(|value| !value.is_empty())
        {
            resolved.insert(key.clone(), Value::String(value));
        }
    }

    resolved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding::test_env;

    const TEST_ENV_KEY: &str = "AI_TOOLBOX_OMP_CONFIG_VALUE_TEST";

    fn set_test_env(value: Option<&str>) {
        match value {
            Some(value) => std::env::set_var(TEST_ENV_KEY, value),
            None => std::env::remove_var(TEST_ENV_KEY),
        }
    }

    async fn resolve_local(raw: &str) -> Option<String> {
        resolve_config_value(raw, &ConfigValueHost::Local).await
    }

    #[tokio::test]
    async fn values_use_the_exact_environment_variable_and_fall_back_to_literals() {
        let _guard = test_env::lock();
        set_test_env(Some("env-secret"));

        assert_eq!(
            resolve_local(TEST_ENV_KEY).await,
            Some("env-secret".to_string())
        );
        assert_eq!(
            resolve_local("sk-plaintext").await,
            Some("sk-plaintext".to_string())
        );
        // Unset names are not an error: OMP uses the configured value as-is.
        assert_eq!(
            resolve_local("AI_TOOLBOX_OMP_CONFIG_VALUE_MISSING").await,
            Some("AI_TOOLBOX_OMP_CONFIG_VALUE_MISSING".to_string())
        );

        set_test_env(None);
    }

    #[tokio::test]
    async fn empty_environment_values_fall_back_to_the_literal() {
        let _guard = test_env::lock();
        set_test_env(Some(""));

        assert_eq!(
            resolve_local(TEST_ENV_KEY).await,
            Some(TEST_ENV_KEY.to_string())
        );

        set_test_env(None);
    }

    #[tokio::test]
    async fn environment_lookup_does_not_match_a_different_case() {
        let _guard = test_env::lock();
        set_test_env(Some("env-secret"));

        assert_eq!(
            resolve_local(&TEST_ENV_KEY.to_ascii_lowercase()).await,
            Some(TEST_ENV_KEY.to_ascii_lowercase())
        );

        set_test_env(None);
    }

    #[tokio::test]
    async fn executes_shell_commands_and_trims_output() {
        assert_eq!(
            resolve_local("!echo ai-toolbox-omp-config-value").await,
            Some("ai-toolbox-omp-config-value".to_string())
        );
    }

    #[tokio::test]
    async fn failed_and_empty_commands_resolve_to_nothing() {
        assert_eq!(resolve_local("!exit 1").await, None);

        #[cfg(target_os = "windows")]
        let empty_output_command = "!rem ai-toolbox-omp";
        #[cfg(not(target_os = "windows"))]
        let empty_output_command = "!true";

        assert_eq!(resolve_local(empty_output_command).await, None);
    }

    #[tokio::test]
    async fn header_values_are_resolved_and_empty_entries_are_dropped() {
        let _guard = test_env::lock();
        set_test_env(Some("header-secret"));

        let resolved = resolve_header_values(
            &serde_json::json!({
                "X-Corp-Auth": TEST_ENV_KEY,
                "X-Static": "plain",
                "X-Empty": "",
                "X-Numeric": 7,
            })
            .as_object()
            .unwrap()
            .clone(),
            &ConfigValueHost::Local,
        )
        .await;

        assert_eq!(
            resolved["X-Corp-Auth"],
            Value::String("header-secret".to_string())
        );
        assert_eq!(resolved["X-Static"], Value::String("plain".to_string()));
        assert_eq!(resolved["X-Numeric"], Value::Number(7.into()));
        assert!(!resolved.contains_key("X-Empty"));

        set_test_env(None);
    }
}
