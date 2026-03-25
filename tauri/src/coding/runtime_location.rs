use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::coding::open_code::shell_env;
use crate::coding::{claude_code, codex, open_claw, open_code};

const MODULE_KEYS: [&str; 4] = ["opencode", "claude", "codex", "openclaw"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeLocationMode {
    LocalWindows,
    WslDirect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WslLocationInfo {
    pub distro: String,
    pub linux_path: String,
    pub linux_user_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLocationInfo {
    pub mode: RuntimeLocationMode,
    pub source: String,
    pub host_path: PathBuf,
    pub wsl: Option<WslLocationInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WslDirectModuleStatus {
    pub module: String,
    pub is_wsl_direct: bool,
    pub reason: Option<String>,
    pub source_path: Option<String>,
    pub linux_path: Option<String>,
    pub linux_user_root: Option<String>,
    pub distro: Option<String>,
}

pub fn is_wsl_unc_path(path: &str) -> bool {
    let lower = path.trim().to_ascii_lowercase();
    lower.starts_with("\\\\wsl\\")
        || lower.starts_with("\\\\wsl$\\")
        || lower.starts_with("\\\\wsl.localhost\\")
}

pub fn parse_wsl_unc_path(path: &str) -> Option<WslLocationInfo> {
    let trimmed = path.trim();
    if trimmed.is_empty() || !is_wsl_unc_path(trimmed) {
        return None;
    }

    let without_prefix = trimmed.trim_start_matches('\\');
    let mut segments = without_prefix
        .split('\\')
        .filter(|segment| !segment.is_empty());
    let host = segments.next()?.to_ascii_lowercase();
    if host != "wsl" && host != "wsl$" && host != "wsl.localhost" {
        return None;
    }

    let distro = segments.next()?.to_string();
    let linux_segments: Vec<String> = segments.map(|segment| segment.to_string()).collect();
    let linux_path = if linux_segments.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", linux_segments.join("/"))
    };

    let linux_user_root = detect_linux_user_root(&linux_path);

    Some(WslLocationInfo {
        distro,
        linux_path,
        linux_user_root,
    })
}

fn detect_linux_user_root(linux_path: &str) -> Option<String> {
    let segments: Vec<&str> = linux_path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    if segments.len() >= 2 && segments[0] == "home" {
        return Some(format!("/home/{}", segments[1]));
    }

    if segments.first().copied() == Some("root") {
        return Some("/root".to_string());
    }

    None
}

pub fn build_windows_unc_path(distro: &str, linux_path: &str) -> PathBuf {
    let normalized_linux_path = linux_path.trim();
    let suffix = normalized_linux_path
        .trim_start_matches('/')
        .replace('/', "\\");
    if suffix.is_empty() {
        PathBuf::from(format!("\\\\wsl.localhost\\{}", distro))
    } else {
        PathBuf::from(format!("\\\\wsl.localhost\\{}\\{}", distro, suffix))
    }
}

pub fn expand_home_from_user_root(linux_user_root: Option<&str>, candidate: &str) -> String {
    if candidate == "~" {
        return linux_user_root.unwrap_or("~").to_string();
    }

    if let Some(rest) = candidate.strip_prefix("~/") {
        return match linux_user_root {
            Some(root) => format!("{}/{}", root.trim_end_matches('/'), rest),
            None => candidate.to_string(),
        };
    }

    candidate.to_string()
}

fn build_wsl_reason(_module: &str, _source_path: &str, _distro: &str) -> String {
    "wsl_direct_config_path".to_string()
}

pub fn module_status_from_location(
    module: &str,
    location: &RuntimeLocationInfo,
) -> WslDirectModuleStatus {
    match &location.wsl {
        Some(wsl) => WslDirectModuleStatus {
            module: module.to_string(),
            is_wsl_direct: true,
            reason: Some(build_wsl_reason(
                module,
                &location.host_path.to_string_lossy(),
                &wsl.distro,
            )),
            source_path: Some(location.host_path.to_string_lossy().to_string()),
            linux_path: Some(wsl.linux_path.clone()),
            linux_user_root: wsl.linux_user_root.clone(),
            distro: Some(wsl.distro.clone()),
        },
        None => WslDirectModuleStatus {
            module: module.to_string(),
            is_wsl_direct: false,
            reason: None,
            source_path: Some(location.host_path.to_string_lossy().to_string()),
            linux_path: None,
            linux_user_root: None,
            distro: None,
        },
    }
}

pub fn get_wsl_direct_status_map(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<Vec<WslDirectModuleStatus>, String> {
    let mut statuses = Vec::new();
    for module in MODULE_KEYS {
        let location = match module {
            "opencode" => get_opencode_runtime_location_sync(db)?,
            "claude" => get_claude_runtime_location_sync(db)?,
            "codex" => get_codex_runtime_location_sync(db)?,
            "openclaw" => get_openclaw_runtime_location_sync(db)?,
            _ => continue,
        };
        statuses.push(module_status_from_location(module, &location));
    }
    Ok(statuses)
}

pub async fn get_wsl_direct_status_map_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<Vec<WslDirectModuleStatus>, String> {
    let opencode = get_opencode_runtime_location_async(db).await?;
    let claude = get_claude_runtime_location_async(db).await?;
    let codex = get_codex_runtime_location_async(db).await?;
    let openclaw = get_openclaw_runtime_location_async(db).await?;

    Ok(vec![
        module_status_from_location("opencode", &opencode),
        module_status_from_location("claude", &claude),
        module_status_from_location("codex", &codex),
        module_status_from_location("openclaw", &openclaw),
    ])
}

pub fn get_wsl_direct_status_for_module(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    module: &str,
) -> Result<WslDirectModuleStatus, String> {
    let location = match module {
        "opencode" => get_opencode_runtime_location_sync(db)?,
        "claude" => get_claude_runtime_location_sync(db)?,
        "codex" => get_codex_runtime_location_sync(db)?,
        "openclaw" => get_openclaw_runtime_location_sync(db)?,
        other => {
            return Err(format!("Unsupported runtime module: {}", other));
        }
    };
    Ok(module_status_from_location(module, &location))
}

pub async fn get_wsl_direct_status_for_module_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    module: &str,
) -> Result<WslDirectModuleStatus, String> {
    let location = match module {
        "opencode" => get_opencode_runtime_location_async(db).await?,
        "claude" => get_claude_runtime_location_async(db).await?,
        "codex" => get_codex_runtime_location_async(db).await?,
        "openclaw" => get_openclaw_runtime_location_async(db).await?,
        other => {
            return Err(format!("Unsupported runtime module: {}", other));
        }
    };
    Ok(module_status_from_location(module, &location))
}

pub fn get_opencode_runtime_location_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<RuntimeLocationInfo, String> {
    let (path, source) = resolve_opencode_config_path_sync(db)?;
    Ok(build_runtime_location(path, source))
}

pub async fn get_opencode_runtime_location_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<RuntimeLocationInfo, String> {
    let (path, source) = resolve_opencode_config_path_async(db).await?;
    Ok(build_runtime_location(path, source))
}

pub fn get_opencode_config_dir_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    get_opencode_runtime_location_sync(db)?
        .host_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Failed to determine OpenCode config directory".to_string())
}

pub async fn get_opencode_config_dir_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    get_opencode_runtime_location_async(db)
        .await?
        .host_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Failed to determine OpenCode config directory".to_string())
}

pub fn get_opencode_prompt_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_opencode_config_dir_sync(db)?.join("AGENTS.md"))
}

pub async fn get_opencode_prompt_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_opencode_config_dir_async(db).await?.join("AGENTS.md"))
}

pub fn get_omo_config_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    let dir = get_opencode_config_dir_sync(db)?;
    let jsonc_path = dir.join("oh-my-opencode.jsonc");
    let json_path = dir.join("oh-my-opencode.json");
    if jsonc_path.exists() {
        Ok(jsonc_path)
    } else if json_path.exists() {
        Ok(json_path)
    } else {
        Ok(jsonc_path)
    }
}

pub async fn get_omo_config_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    let dir = get_opencode_config_dir_async(db).await?;
    let jsonc_path = dir.join("oh-my-opencode.jsonc");
    let json_path = dir.join("oh-my-opencode.json");
    if jsonc_path.exists() {
        Ok(jsonc_path)
    } else if json_path.exists() {
        Ok(json_path)
    } else {
        Ok(jsonc_path)
    }
}

pub fn get_omos_config_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_opencode_config_dir_sync(db)?.join("oh-my-opencode-slim.json"))
}

pub async fn get_omos_config_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_opencode_config_dir_async(db)
        .await?
        .join("oh-my-opencode-slim.json"))
}

pub fn get_opencode_wsl_target_path(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> String {
    get_opencode_runtime_location_sync(db)
        .ok()
        .and_then(|location| {
            location.wsl.map(|wsl| wsl.linux_path).or_else(|| {
                location
                    .host_path
                    .file_name()
                    .map(|name| format!("~/.config/opencode/{}", name.to_string_lossy()))
            })
        })
        .unwrap_or_else(|| "~/.config/opencode/opencode.jsonc".to_string())
}

pub async fn get_opencode_wsl_target_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> String {
    get_opencode_runtime_location_async(db)
        .await
        .ok()
        .and_then(|location| {
            location.wsl.map(|wsl| wsl.linux_path).or_else(|| {
                location
                    .host_path
                    .file_name()
                    .map(|name| format!("~/.config/opencode/{}", name.to_string_lossy()))
            })
        })
        .unwrap_or_else(|| "~/.config/opencode/opencode.jsonc".to_string())
}

pub fn get_opencode_prompt_wsl_target_path(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> String {
    get_opencode_runtime_location_sync(db)
        .ok()
        .and_then(|location| {
            location.wsl.map(|wsl| {
                let prompt_path = format!(
                    "{}/AGENTS.md",
                    wsl.linux_path
                        .rsplit_once('/')
                        .map(|(parent, _)| parent)
                        .unwrap_or("/")
                        .trim_end_matches('/')
                );
                if prompt_path.starts_with("//") {
                    "/AGENTS.md".to_string()
                } else {
                    prompt_path
                }
            })
        })
        .unwrap_or_else(|| "~/.config/opencode/AGENTS.md".to_string())
}

pub async fn get_opencode_prompt_wsl_target_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> String {
    get_opencode_runtime_location_async(db)
        .await
        .ok()
        .and_then(|location| {
            location.wsl.map(|wsl| {
                let prompt_path = format!(
                    "{}/AGENTS.md",
                    wsl.linux_path
                        .rsplit_once('/')
                        .map(|(parent, _)| parent)
                        .unwrap_or("/")
                        .trim_end_matches('/')
                );
                if prompt_path.starts_with("//") {
                    "/AGENTS.md".to_string()
                } else {
                    prompt_path
                }
            })
        })
        .unwrap_or_else(|| "~/.config/opencode/AGENTS.md".to_string())
}

pub fn get_claude_runtime_location_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<RuntimeLocationInfo, String> {
    let custom_path = get_custom_path_blocking(
        db,
        "SELECT * OMIT id FROM claude_common_config:`common` LIMIT 1",
        |value| {
            crate::coding::claude_code::adapter::from_db_value_common(value)
                .root_dir
                .filter(|path| !path.trim().is_empty())
        },
    );

    let (path, source) = if let Some(path) = custom_path {
        (PathBuf::from(path), "custom".to_string())
    } else if let Ok(env_path) = std::env::var("CLAUDE_CONFIG_DIR") {
        if !env_path.trim().is_empty() {
            (PathBuf::from(env_path), "env".to_string())
        } else if let Some(shell_path) = shell_env::get_env_from_shell_config("CLAUDE_CONFIG_DIR") {
            (PathBuf::from(shell_path), "shell".to_string())
        } else {
            (
                claude_code::get_claude_default_root_dir()?,
                "default".to_string(),
            )
        }
    } else if let Some(shell_path) = shell_env::get_env_from_shell_config("CLAUDE_CONFIG_DIR") {
        (PathBuf::from(shell_path), "shell".to_string())
    } else {
        (
            claude_code::get_claude_default_root_dir()?,
            "default".to_string(),
        )
    };

    Ok(build_runtime_location(path, source))
}

pub async fn get_claude_runtime_location_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<RuntimeLocationInfo, String> {
    let path_info = get_custom_path_from_query(
        db,
        "SELECT * OMIT id FROM claude_common_config:`common` LIMIT 1",
        |value| {
            crate::coding::claude_code::adapter::from_db_value_common(value)
                .root_dir
                .filter(|path| !path.trim().is_empty())
        },
    )
    .await;

    let (path, source) = if let Some(path) = path_info {
        (PathBuf::from(path), "custom".to_string())
    } else if let Ok(env_path) = std::env::var("CLAUDE_CONFIG_DIR") {
        if !env_path.trim().is_empty() {
            (PathBuf::from(env_path), "env".to_string())
        } else if let Some(shell_path) = shell_env::get_env_from_shell_config("CLAUDE_CONFIG_DIR") {
            (PathBuf::from(shell_path), "shell".to_string())
        } else {
            (
                claude_code::get_claude_default_root_dir()?,
                "default".to_string(),
            )
        }
    } else if let Some(shell_path) = shell_env::get_env_from_shell_config("CLAUDE_CONFIG_DIR") {
        (PathBuf::from(shell_path), "shell".to_string())
    } else {
        (
            claude_code::get_claude_default_root_dir()?,
            "default".to_string(),
        )
    };

    Ok(build_runtime_location(path, source))
}

pub fn get_claude_settings_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_claude_runtime_location_sync(db)?
        .host_path
        .join("settings.json"))
}

pub async fn get_claude_settings_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_claude_runtime_location_async(db)
        .await?
        .host_path
        .join("settings.json"))
}

pub fn get_claude_plugin_config_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_claude_runtime_location_sync(db)?
        .host_path
        .join("config.json"))
}

pub async fn get_claude_plugin_config_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_claude_runtime_location_async(db)
        .await?
        .host_path
        .join("config.json"))
}

pub fn get_claude_prompt_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_claude_runtime_location_sync(db)?
        .host_path
        .join("CLAUDE.md"))
}

pub async fn get_claude_prompt_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_claude_runtime_location_async(db)
        .await?
        .host_path
        .join("CLAUDE.md"))
}

pub fn get_claude_mcp_config_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    let location = get_claude_runtime_location_sync(db)?;
    if let Some(wsl) = &location.wsl {
        Ok(build_windows_unc_path(
            &wsl.distro,
            &format!(
                "{}/.claude.json",
                wsl.linux_user_root
                    .as_deref()
                    .unwrap_or_else(|| wsl.linux_path.as_str())
                    .trim_end_matches('/')
            ),
        ))
    } else {
        let home_dir = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .map_err(|_| "Failed to get home directory".to_string())?;
        Ok(Path::new(&home_dir).join(".claude.json"))
    }
}

pub async fn get_claude_mcp_config_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    let location = get_claude_runtime_location_async(db).await?;
    if let Some(wsl) = &location.wsl {
        Ok(build_windows_unc_path(
            &wsl.distro,
            &format!(
                "{}/.claude.json",
                wsl.linux_user_root
                    .as_deref()
                    .unwrap_or_else(|| wsl.linux_path.as_str())
                    .trim_end_matches('/')
            ),
        ))
    } else {
        let home_dir = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .map_err(|_| "Failed to get home directory".to_string())?;
        Ok(Path::new(&home_dir).join(".claude.json"))
    }
}

pub fn get_claude_wsl_target_path(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    file_name: &str,
) -> String {
    match get_claude_runtime_location_sync(db) {
        Ok(location) => location
            .wsl
            .map(|wsl| format!("{}/{}", wsl.linux_path.trim_end_matches('/'), file_name))
            .unwrap_or_else(|| format!("~/.claude/{}", file_name)),
        Err(_) => format!("~/.claude/{}", file_name),
    }
}

pub async fn get_claude_wsl_target_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    file_name: &str,
) -> String {
    match get_claude_runtime_location_async(db).await {
        Ok(location) => location
            .wsl
            .map(|wsl| format!("{}/{}", wsl.linux_path.trim_end_matches('/'), file_name))
            .unwrap_or_else(|| format!("~/.claude/{}", file_name)),
        Err(_) => format!("~/.claude/{}", file_name),
    }
}

pub fn get_claude_wsl_claude_json_path(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> String {
    match get_claude_runtime_location_sync(db) {
        Ok(location) => location
            .wsl
            .and_then(|wsl| {
                wsl.linux_user_root
                    .map(|root| format!("{}/.claude.json", root.trim_end_matches('/')))
            })
            .unwrap_or_else(|| "~/.claude.json".to_string()),
        Err(_) => "~/.claude.json".to_string(),
    }
}

pub async fn get_claude_wsl_claude_json_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> String {
    match get_claude_runtime_location_async(db).await {
        Ok(location) => location
            .wsl
            .and_then(|wsl| {
                wsl.linux_user_root
                    .map(|root| format!("{}/.claude.json", root.trim_end_matches('/')))
            })
            .unwrap_or_else(|| "~/.claude.json".to_string()),
        Err(_) => "~/.claude.json".to_string(),
    }
}

pub fn get_codex_runtime_location_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<RuntimeLocationInfo, String> {
    let custom_path = get_custom_path_blocking(
        db,
        "SELECT * OMIT id FROM codex_common_config:`common` LIMIT 1",
        |value| {
            crate::coding::codex::adapter::from_db_value_common(value)
                .root_dir
                .filter(|path| !path.trim().is_empty())
        },
    );

    let (path, source) = if let Some(path) = custom_path {
        (PathBuf::from(path), "custom".to_string())
    } else if let Ok(env_path) = std::env::var("CODEX_HOME") {
        if !env_path.trim().is_empty() {
            (PathBuf::from(env_path), "env".to_string())
        } else if let Some(shell_path) = shell_env::get_env_from_shell_config("CODEX_HOME") {
            (PathBuf::from(shell_path), "shell".to_string())
        } else {
            (codex::get_codex_default_root_dir()?, "default".to_string())
        }
    } else if let Some(shell_path) = shell_env::get_env_from_shell_config("CODEX_HOME") {
        (PathBuf::from(shell_path), "shell".to_string())
    } else {
        (codex::get_codex_default_root_dir()?, "default".to_string())
    };

    Ok(build_runtime_location(path, source))
}

pub async fn get_codex_runtime_location_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<RuntimeLocationInfo, String> {
    let path_info = get_custom_path_from_query(
        db,
        "SELECT * OMIT id FROM codex_common_config:`common` LIMIT 1",
        |value| {
            crate::coding::codex::adapter::from_db_value_common(value)
                .root_dir
                .filter(|path| !path.trim().is_empty())
        },
    )
    .await;

    let (path, source) = if let Some(path) = path_info {
        (PathBuf::from(path), "custom".to_string())
    } else if let Ok(env_path) = std::env::var("CODEX_HOME") {
        if !env_path.trim().is_empty() {
            (PathBuf::from(env_path), "env".to_string())
        } else if let Some(shell_path) = shell_env::get_env_from_shell_config("CODEX_HOME") {
            (PathBuf::from(shell_path), "shell".to_string())
        } else {
            (codex::get_codex_default_root_dir()?, "default".to_string())
        }
    } else if let Some(shell_path) = shell_env::get_env_from_shell_config("CODEX_HOME") {
        (PathBuf::from(shell_path), "shell".to_string())
    } else {
        (codex::get_codex_default_root_dir()?, "default".to_string())
    };

    Ok(build_runtime_location(path, source))
}

pub fn get_codex_auth_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_codex_runtime_location_sync(db)?
        .host_path
        .join("auth.json"))
}

pub async fn get_codex_auth_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_codex_runtime_location_async(db)
        .await?
        .host_path
        .join("auth.json"))
}

pub fn get_codex_config_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_codex_runtime_location_sync(db)?
        .host_path
        .join("config.toml"))
}

pub async fn get_codex_config_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_codex_runtime_location_async(db)
        .await?
        .host_path
        .join("config.toml"))
}

pub fn get_codex_prompt_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_codex_runtime_location_sync(db)?
        .host_path
        .join("AGENTS.md"))
}

pub async fn get_codex_prompt_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_codex_runtime_location_async(db)
        .await?
        .host_path
        .join("AGENTS.md"))
}

pub fn get_codex_wsl_target_path(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    file_name: &str,
) -> String {
    match get_codex_runtime_location_sync(db) {
        Ok(location) => location
            .wsl
            .map(|wsl| format!("{}/{}", wsl.linux_path.trim_end_matches('/'), file_name))
            .unwrap_or_else(|| format!("~/.codex/{}", file_name)),
        Err(_) => format!("~/.codex/{}", file_name),
    }
}

pub async fn get_codex_wsl_target_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    file_name: &str,
) -> String {
    match get_codex_runtime_location_async(db).await {
        Ok(location) => location
            .wsl
            .map(|wsl| format!("{}/{}", wsl.linux_path.trim_end_matches('/'), file_name))
            .unwrap_or_else(|| format!("~/.codex/{}", file_name)),
        Err(_) => format!("~/.codex/{}", file_name),
    }
}

pub fn get_openclaw_runtime_location_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<RuntimeLocationInfo, String> {
    let (path, source) = resolve_openclaw_config_path_sync(db)?;
    Ok(build_runtime_location(path, source))
}

pub async fn get_openclaw_runtime_location_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<RuntimeLocationInfo, String> {
    let (path, source) = resolve_openclaw_config_path_async(db).await?;
    Ok(build_runtime_location(path, source))
}

pub fn get_openclaw_wsl_target_path(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> String {
    match get_openclaw_runtime_location_sync(db) {
        Ok(location) => location
            .wsl
            .map(|wsl| wsl.linux_path)
            .unwrap_or_else(|| "~/.openclaw/openclaw.json".to_string()),
        Err(_) => "~/.openclaw/openclaw.json".to_string(),
    }
}

pub async fn get_openclaw_wsl_target_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> String {
    match get_openclaw_runtime_location_async(db).await {
        Ok(location) => location
            .wsl
            .map(|wsl| wsl.linux_path)
            .unwrap_or_else(|| "~/.openclaw/openclaw.json".to_string()),
        Err(_) => "~/.openclaw/openclaw.json".to_string(),
    }
}

pub fn get_tool_skills_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    tool_key: &str,
) -> Option<PathBuf> {
    match tool_key {
        "claude_code" => get_claude_runtime_location_sync(db).ok().map(|location| {
            if let Some(wsl) = location.wsl {
                build_windows_unc_path(
                    &wsl.distro,
                    &expand_home_from_user_root(wsl.linux_user_root.as_deref(), "~/.claude/skills"),
                )
            } else {
                location.host_path.join("skills")
            }
        }),
        "codex" => get_codex_runtime_location_sync(db).ok().map(|location| {
            if let Some(wsl) = location.wsl {
                build_windows_unc_path(
                    &wsl.distro,
                    &expand_home_from_user_root(wsl.linux_user_root.as_deref(), "~/.codex/skills"),
                )
            } else {
                location.host_path.join("skills")
            }
        }),
        "opencode" => get_opencode_runtime_location_sync(db).ok().map(|location| {
            if let Some(wsl) = location.wsl {
                build_windows_unc_path(
                    &wsl.distro,
                    &expand_home_from_user_root(
                        wsl.linux_user_root.as_deref(),
                        "~/.config/opencode/skills",
                    ),
                )
            } else {
                location
                    .host_path
                    .parent()
                    .unwrap_or_else(|| location.host_path.as_path())
                    .join("skills")
            }
        }),
        "openclaw" => get_openclaw_runtime_location_sync(db).ok().map(|location| {
            if let Some(wsl) = location.wsl {
                build_windows_unc_path(
                    &wsl.distro,
                    &expand_home_from_user_root(
                        wsl.linux_user_root.as_deref(),
                        "~/.openclaw/skills",
                    ),
                )
            } else {
                location
                    .host_path
                    .parent()
                    .unwrap_or_else(|| location.host_path.as_path())
                    .join("skills")
            }
        }),
        _ => None,
    }
}

pub async fn get_tool_skills_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    tool_key: &str,
) -> Option<PathBuf> {
    match tool_key {
        "claude_code" => get_claude_runtime_location_async(db)
            .await
            .ok()
            .map(|location| {
                if let Some(wsl) = location.wsl {
                    build_windows_unc_path(
                        &wsl.distro,
                        &expand_home_from_user_root(
                            wsl.linux_user_root.as_deref(),
                            "~/.claude/skills",
                        ),
                    )
                } else {
                    location.host_path.join("skills")
                }
            }),
        "codex" => get_codex_runtime_location_async(db)
            .await
            .ok()
            .map(|location| {
                if let Some(wsl) = location.wsl {
                    build_windows_unc_path(
                        &wsl.distro,
                        &expand_home_from_user_root(
                            wsl.linux_user_root.as_deref(),
                            "~/.codex/skills",
                        ),
                    )
                } else {
                    location.host_path.join("skills")
                }
            }),
        "opencode" => get_opencode_runtime_location_async(db)
            .await
            .ok()
            .map(|location| {
                if let Some(wsl) = location.wsl {
                    build_windows_unc_path(
                        &wsl.distro,
                        &expand_home_from_user_root(
                            wsl.linux_user_root.as_deref(),
                            "~/.config/opencode/skills",
                        ),
                    )
                } else {
                    location
                        .host_path
                        .parent()
                        .unwrap_or_else(|| location.host_path.as_path())
                        .join("skills")
                }
            }),
        "openclaw" => get_openclaw_runtime_location_async(db)
            .await
            .ok()
            .map(|location| {
                if let Some(wsl) = location.wsl {
                    build_windows_unc_path(
                        &wsl.distro,
                        &expand_home_from_user_root(
                            wsl.linux_user_root.as_deref(),
                            "~/.openclaw/skills",
                        ),
                    )
                } else {
                    location
                        .host_path
                        .parent()
                        .unwrap_or_else(|| location.host_path.as_path())
                        .join("skills")
                }
            }),
        _ => None,
    }
}

pub fn get_tool_mcp_config_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    tool_key: &str,
) -> Option<PathBuf> {
    match tool_key {
        "claude_code" => get_claude_mcp_config_path_sync(db).ok(),
        "codex" => get_codex_config_path_sync(db).ok(),
        "opencode" => get_opencode_runtime_location_sync(db)
            .ok()
            .map(|location| location.host_path),
        _ => None,
    }
}

pub async fn get_tool_mcp_config_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    tool_key: &str,
) -> Option<PathBuf> {
    match tool_key {
        "claude_code" => get_claude_mcp_config_path_async(db).await.ok(),
        "codex" => get_codex_config_path_async(db).await.ok(),
        "opencode" => get_opencode_runtime_location_async(db)
            .await
            .ok()
            .map(|location| location.host_path),
        _ => None,
    }
}

fn build_runtime_location(path: PathBuf, source: String) -> RuntimeLocationInfo {
    let wsl = path.to_str().and_then(parse_wsl_unc_path);

    RuntimeLocationInfo {
        mode: if wsl.is_some() {
            RuntimeLocationMode::WslDirect
        } else {
            RuntimeLocationMode::LocalWindows
        },
        source,
        host_path: path,
        wsl,
    }
}

fn resolve_opencode_config_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<(PathBuf, String), String> {
    let custom_path = get_custom_path_blocking(
        db,
        "SELECT *, type::string(id) as id FROM opencode_common_config:`common` LIMIT 1",
        |value| {
            open_code::adapter::from_db_value(value)
                .config_path
                .filter(|path| !path.trim().is_empty())
        },
    );

    if let Some(path) = custom_path {
        return Ok((PathBuf::from(path), "custom".to_string()));
    }

    if let Ok(env_path) = std::env::var("OPENCODE_CONFIG") {
        if !env_path.trim().is_empty() {
            return Ok((PathBuf::from(env_path), "env".to_string()));
        }
    }

    if let Some(shell_path) = shell_env::get_env_from_shell_config("OPENCODE_CONFIG") {
        if !shell_path.trim().is_empty() {
            return Ok((PathBuf::from(shell_path), "shell".to_string()));
        }
    }

    Ok((
        PathBuf::from(open_code::get_default_config_path()?),
        "default".to_string(),
    ))
}

async fn resolve_opencode_config_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<(PathBuf, String), String> {
    let custom_path = get_custom_path_from_query(
        db,
        "SELECT *, type::string(id) as id FROM opencode_common_config:`common` LIMIT 1",
        |value| {
            open_code::adapter::from_db_value(value)
                .config_path
                .filter(|path| !path.trim().is_empty())
        },
    )
    .await;

    if let Some(path) = custom_path {
        return Ok((PathBuf::from(path), "custom".to_string()));
    }

    if let Ok(env_path) = std::env::var("OPENCODE_CONFIG") {
        if !env_path.trim().is_empty() {
            return Ok((PathBuf::from(env_path), "env".to_string()));
        }
    }

    if let Some(shell_path) = shell_env::get_env_from_shell_config("OPENCODE_CONFIG") {
        if !shell_path.trim().is_empty() {
            return Ok((PathBuf::from(shell_path), "shell".to_string()));
        }
    }

    Ok((
        PathBuf::from(open_code::get_default_config_path()?),
        "default".to_string(),
    ))
}

fn resolve_openclaw_config_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<(PathBuf, String), String> {
    let custom_path = get_custom_path_blocking(
        db,
        "SELECT *, type::string(id) as id FROM openclaw_common_config:`common` LIMIT 1",
        |value| {
            open_claw::adapter::from_db_value(value)
                .config_path
                .filter(|path| !path.trim().is_empty())
        },
    );

    if let Some(path) = custom_path {
        return Ok((PathBuf::from(path), "custom".to_string()));
    }

    let path = open_claw::get_default_config_path_for_runtime()?;
    Ok((PathBuf::from(path), "default".to_string()))
}

async fn resolve_openclaw_config_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<(PathBuf, String), String> {
    let custom_path = get_custom_path_from_query(
        db,
        "SELECT *, type::string(id) as id FROM openclaw_common_config:`common` LIMIT 1",
        |value| {
            open_claw::adapter::from_db_value(value)
                .config_path
                .filter(|path| !path.trim().is_empty())
        },
    )
    .await;

    if let Some(path) = custom_path {
        return Ok((PathBuf::from(path), "custom".to_string()));
    }

    let path = open_claw::get_default_config_path_for_runtime()?;
    Ok((PathBuf::from(path), "default".to_string()))
}

fn get_custom_path_blocking<F>(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    query: &str,
    extractor: F,
) -> Option<String>
where
    F: Fn(Value) -> Option<String>,
{
    let result: Result<Vec<Value>, _> =
        tauri::async_runtime::block_on(async { db.query(query).await })
            .ok()?
            .take(0);
    let record = result.ok()?.into_iter().next()?;
    extractor(record)
}

async fn get_custom_path_from_query<F>(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    query: &str,
    extractor: F,
) -> Option<String>
where
    F: Fn(Value) -> Option<String>,
{
    let mut result = db.query(query).await.ok()?;
    let records: Vec<Value> = result.take(0).ok()?;
    let record = records.into_iter().next()?;
    extractor(record)
}
