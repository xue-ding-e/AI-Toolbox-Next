//! Pi-style config value resolution for values stored in Pi's `models.json`.
//!
//! Mirrors `packages/coding-agent/src/core/resolve-config-value.ts` from the Pi
//! CLI: the shared model-discovery and connectivity commands must resolve
//! `$ENV_VAR` / `${ENV_VAR}` interpolation, `!command`, `$$` and `$!` the same
//! way the Pi runtime does, instead of forwarding the raw template as a literal
//! credential upstream.
//!
//! Values belong to the Pi runtime that will consume them, so resolution runs in
//! that runtime's environment: the host process for a local Pi root, and inside
//! the target WSL distribution for a WSL Direct Pi root.

use std::path::Path;
use std::process::{Output, Stdio};

use serde_json::{Map, Value};
use tokio::process::Command;

#[cfg(target_os = "windows")]
use crate::coding::cli_resolver;
use crate::coding::config_value_host::{
    lookup_env_var, wsl_command, ConfigValueHost, COMMAND_TIMEOUT,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum TemplatePart {
    Literal(String),
    Env(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConfigValueReference {
    /// `!cmd`: the whole value after the leading `!` is a shell command.
    Command(String),
    /// Everything else: literals plus `${VAR}` / `$VAR` interpolations.
    Template(Vec<TemplatePart>),
}

fn is_env_var_name(value: &str) -> bool {
    !value.is_empty() && env_var_name_prefix(value).len() == value.len()
}

/// Leading `[A-Za-z_][A-Za-z0-9_]*` run, mirroring the upstream prefix regex.
/// `$VAR_SUFFIX` therefore keeps matching `$VAR` only when the name really ends
/// there, and `$1` is not an environment reference at all.
fn env_var_name_prefix(value: &str) -> &str {
    let mut characters = value.char_indices();
    let Some((_, first)) = characters.next() else {
        return "";
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return "";
    }

    let mut end = first.len_utf8();
    for (index, character) in characters {
        if character.is_ascii_alphanumeric() || character == '_' {
            end = index + character.len_utf8();
        } else {
            break;
        }
    }
    &value[..end]
}

fn push_literal(parts: &mut Vec<TemplatePart>, value: &str) {
    if value.is_empty() {
        return;
    }
    match parts.last_mut() {
        Some(TemplatePart::Literal(previous)) => previous.push_str(value),
        _ => parts.push(TemplatePart::Literal(value.to_string())),
    }
}

fn parse_template(config: &str) -> Vec<TemplatePart> {
    let mut parts = Vec::new();
    let mut index = 0usize;

    while index < config.len() {
        let Some(offset) = config[index..].find('$') else {
            push_literal(&mut parts, &config[index..]);
            break;
        };
        let dollar_index = index + offset;
        push_literal(&mut parts, &config[index..dollar_index]);

        let next_index = dollar_index + 1;
        match config[next_index..].chars().next() {
            // `$$` and `$!` emit the escaped character itself.
            Some(escaped @ ('$' | '!')) => {
                push_literal(&mut parts, &escaped.to_string());
                index = next_index + escaped.len_utf8();
            }
            Some('{') => match config[next_index + 1..].find('}') {
                // Unclosed `${` stays a literal `$`.
                None => {
                    push_literal(&mut parts, "$");
                    index = next_index;
                }
                Some(brace_offset) => {
                    let end_index = next_index + 1 + brace_offset;
                    let name = &config[next_index + 1..end_index];
                    if is_env_var_name(name) {
                        parts.push(TemplatePart::Env(name.to_string()));
                    } else {
                        push_literal(&mut parts, &config[dollar_index..end_index + 1]);
                    }
                    index = end_index + 1;
                }
            },
            _ => {
                let name = env_var_name_prefix(&config[next_index..]);
                if name.is_empty() {
                    push_literal(&mut parts, "$");
                    index = next_index;
                } else {
                    parts.push(TemplatePart::Env(name.to_string()));
                    index = next_index + name.len();
                }
            }
        }
    }

    parts
}

fn parse_reference(config: &str) -> ConfigValueReference {
    if config.starts_with('!') {
        return ConfigValueReference::Command(config.to_string());
    }
    ConfigValueReference::Template(parse_template(config))
}

fn missing_env_var_error(label: &str, names: &[String]) -> String {
    if names.len() == 1 {
        format!(
            "Failed to resolve {label} from environment variable: {}",
            names[0]
        )
    } else {
        format!(
            "Failed to resolve {label} from environment variables: {}",
            names.join(", ")
        )
    }
}

#[cfg(target_os = "windows")]
fn resolve_local_bash() -> Option<std::path::PathBuf> {
    // Pi searches Git for Windows in the well-known Program Files locations
    // before falling back to `bash` on PATH. Checking the candidates first also
    // matters because `where bash` often resolves to the System32 WSL relay,
    // which is not the bash Pi would pick.
    for env_key in ["ProgramFiles", "ProgramFiles(x86)"] {
        let Ok(root) = std::env::var(env_key) else {
            continue;
        };
        if root.trim().is_empty() {
            continue;
        }
        let candidate = Path::new(&root).join("Git").join("bin").join("bash.exe");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let from_path = cli_resolver::resolve_named_cli_program("bash", Vec::new());
    from_path.path.exists().then_some(from_path.path)
}

#[cfg(target_os = "windows")]
fn local_shell_command(command: &str) -> Command {
    if let Some(bash) = resolve_local_bash() {
        let mut shell = cli_resolver::build_local_tokio_command(&bash);
        shell.arg("-c").arg(command);
        return shell;
    }

    // Pi falls back to the Node default shell when no bash is available.
    let mut shell = Command::new("cmd");
    cli_resolver::apply_create_no_window_tokio(&mut shell);
    shell.args(["/C", command]);
    shell
}

#[cfg(not(target_os = "windows"))]
fn local_shell_command(command: &str) -> Command {
    // Pi prefers bash on unix and only falls back to plain sh.
    let program = if Path::new("/bin/bash").exists() {
        "/bin/bash"
    } else {
        "sh"
    };
    let mut shell = Command::new(program);
    shell.arg("-c").arg(command);
    shell
}

async fn run_local_shell_command(command: &str) -> Option<Output> {
    let mut shell = local_shell_command(command);
    shell
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    tokio::time::timeout(COMMAND_TIMEOUT, shell.output())
        .await
        .ok()?
        .ok()
}

async fn run_wsl_shell_command(distro: &str, command: &str) -> Option<Output> {
    // Pi uses bash inside the distro; minimal images without bash fall back to sh.
    for program in ["bash", "sh"] {
        let mut wsl = wsl_command(distro, program, &["-c", command]);
        wsl.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        match tokio::time::timeout(COMMAND_TIMEOUT, wsl.output()).await {
            Ok(Ok(output)) => return Some(output),
            // The shell itself could not be started; try the next candidate.
            Ok(Err(_)) => continue,
            // A timeout must not be retried, or a hanging command doubles the wait.
            Err(_) => return None,
        }
    }
    None
}

async fn run_shell_command(command: &str, host: &ConfigValueHost) -> Option<String> {
    let output = match host {
        ConfigValueHost::Local => run_local_shell_command(command).await?,
        ConfigValueHost::Wsl { distro } => run_wsl_shell_command(distro, command).await?,
    };

    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}

/// Resolve a Pi config value the way the Pi runtime would.
///
/// `label` names the resolved value in errors, e.g.
/// `API key for provider "custom"`. Unresolvable values are reported instead of
/// being sent upstream as literals, so a missing environment variable surfaces
/// as a clear error rather than an authentication failure.
pub async fn resolve_config_value(
    raw: &str,
    label: &str,
    host: &ConfigValueHost,
) -> Result<String, String> {
    match parse_reference(raw) {
        ConfigValueReference::Command(command_config) => {
            let command = &command_config[1..];
            run_shell_command(command, host)
                .await
                .ok_or_else(|| format!("Failed to resolve {label} from shell command: {command}"))
        }
        ConfigValueReference::Template(parts) => {
            let mut resolved = String::new();
            let mut missing: Vec<String> = Vec::new();

            for part in &parts {
                match part {
                    TemplatePart::Literal(value) => resolved.push_str(value),
                    TemplatePart::Env(name) => match lookup_env_var(name, host).await {
                        Some(value) => resolved.push_str(&value),
                        None => {
                            if !missing.contains(name) {
                                missing.push(name.clone());
                            }
                        }
                    },
                }
            }

            if missing.is_empty() {
                Ok(resolved)
            } else {
                Err(missing_env_var_error(label, &missing))
            }
        }
    }
}

/// Resolve every string header value with the same rules as API keys.
///
/// Mirrors the Pi CLI's `resolveHeadersOrThrow`; non-string values are left
/// untouched because Pi itself only accepts string header values.
pub async fn resolve_header_values(
    headers: &Map<String, Value>,
    provider_id: &str,
    host: &ConfigValueHost,
) -> Result<Map<String, Value>, String> {
    let mut resolved = Map::with_capacity(headers.len());

    for (key, value) in headers {
        let Some(raw) = value.as_str() else {
            resolved.insert(key.clone(), value.clone());
            continue;
        };
        let label = format!("provider \"{provider_id}\" header \"{key}\"");
        resolved.insert(
            key.clone(),
            Value::String(resolve_config_value(raw, &label, host).await?),
        );
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding::test_env;

    const TEST_ENV_KEY: &str = "AI_TOOLBOX_PI_CONFIG_VALUE_TEST";

    fn set_test_env(value: Option<&str>) {
        match value {
            Some(value) => std::env::set_var(TEST_ENV_KEY, value),
            None => std::env::remove_var(TEST_ENV_KEY),
        }
    }

    async fn resolve_local(raw: &str, label: &str) -> Result<String, String> {
        resolve_config_value(raw, label, &ConfigValueHost::Local).await
    }

    #[tokio::test]
    async fn resolves_literals_env_templates_and_escapes() {
        let _guard = test_env::lock();
        set_test_env(Some("left"));
        std::env::set_var("AI_TOOLBOX_PI_CONFIG_VALUE_TEST_RIGHT", "right");

        assert_eq!(
            resolve_local("sk-literal", "test").await.unwrap(),
            "sk-literal"
        );
        assert_eq!(
            resolve_local(&format!("${TEST_ENV_KEY}"), "test")
                .await
                .unwrap(),
            "left"
        );
        assert_eq!(
            resolve_local(
                &format!("${{{TEST_ENV_KEY}}}_$AI_TOOLBOX_PI_CONFIG_VALUE_TEST_RIGHT"),
                "test",
            )
            .await
            .unwrap(),
            "left_right"
        );
        assert_eq!(
            resolve_local(&format!("$${TEST_ENV_KEY}"), "test")
                .await
                .unwrap(),
            format!("${TEST_ENV_KEY}")
        );
        assert_eq!(
            resolve_local(&format!("$!literal-${TEST_ENV_KEY}"), "test")
                .await
                .unwrap(),
            "!literal-left"
        );

        set_test_env(None);
        std::env::remove_var("AI_TOOLBOX_PI_CONFIG_VALUE_TEST_RIGHT");
    }

    #[tokio::test]
    async fn missing_env_var_is_reported_by_name() {
        let _guard = test_env::lock();
        set_test_env(None);

        let error = resolve_local(&format!("${TEST_ENV_KEY}"), "test")
            .await
            .unwrap_err();
        assert_eq!(
            error,
            format!("Failed to resolve test from environment variable: {TEST_ENV_KEY}")
        );
    }

    #[tokio::test]
    async fn non_interpolated_references_stay_literal() {
        let _guard = test_env::lock();
        std::env::set_var("AI_TOOLBOX_PI_CONFIG_VALUE_TEST_SUFFIXED", "kept");

        // A name that is not a plain variable reference must not be truncated,
        // and a digit-leading `$1` is not an environment reference at all.
        assert_eq!(resolve_local("${1AB}", "test").await.unwrap(), "${1AB}");
        assert_eq!(resolve_local("$1", "test").await.unwrap(), "$1");
        assert_eq!(resolve_local("$", "test").await.unwrap(), "$");
        assert_eq!(
            resolve_local("${unclosed", "test").await.unwrap(),
            "${unclosed"
        );

        std::env::remove_var("AI_TOOLBOX_PI_CONFIG_VALUE_TEST_SUFFIXED");
    }

    #[tokio::test]
    async fn executes_shell_commands_and_trims_output() {
        let value = resolve_local("!echo ai-toolbox-config-value", "test")
            .await
            .unwrap();
        assert_eq!(value, "ai-toolbox-config-value");
    }

    #[tokio::test]
    async fn failed_commands_report_the_command() {
        let error = resolve_local("!exit 1", "test").await.unwrap_err();
        assert_eq!(error, "Failed to resolve test from shell command: exit 1");
    }

    #[tokio::test]
    async fn resolves_header_values_and_reports_failures() {
        let _guard = test_env::lock();
        set_test_env(Some("header-value"));

        let resolved = resolve_header_values(
            &serde_json::json!({
                "X-Corp-Auth": format!("${TEST_ENV_KEY}"),
                "X-Static": "plain",
                "X-Numeric": 7,
            })
            .as_object()
            .unwrap()
            .clone(),
            "custom",
            &ConfigValueHost::Local,
        )
        .await
        .unwrap();

        assert_eq!(
            resolved["X-Corp-Auth"],
            Value::String("header-value".to_string())
        );
        assert_eq!(resolved["X-Static"], Value::String("plain".to_string()));
        assert_eq!(resolved["X-Numeric"], Value::Number(7.into()));

        set_test_env(None);
        let error = resolve_header_values(
            &serde_json::json!({ "X-Corp-Auth": format!("${TEST_ENV_KEY}") })
                .as_object()
                .unwrap()
                .clone(),
            "custom",
            &ConfigValueHost::Local,
        )
        .await
        .unwrap_err();
        assert_eq!(
            error,
            format!(
                "Failed to resolve provider \"custom\" header \"X-Corp-Auth\" from environment variable: {TEST_ENV_KEY}"
            )
        );
    }
}
