use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::coding::open_code::shell_env;
use crate::coding::{claude_code, codex, open_claw, open_code};

const MODULE_KEYS: [&str; 4] = ["opencode", "claude", "codex", "openclaw"];
const OMO_LEGACY_BASENAME: &str = "oh-my-opencode";
const OMO_CANONICAL_BASENAME: &str = "oh-my-openagent";

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
    let _ = db;
    Ok(MODULE_KEYS
        .iter()
        .map(|module| module_status_from_location(module, &get_runtime_location_without_db(module)))
        .collect())
}

fn module_status_from_runtime_result(
    module: &str,
    runtime_result: Result<RuntimeLocationInfo, String>,
    fallback_location: &RuntimeLocationInfo,
) -> WslDirectModuleStatus {
    match runtime_result {
        Ok(location) => module_status_from_location(module, &location),
        Err(error) => {
            log::warn!(
                "Failed to resolve runtime location for module '{}' while building WSL direct status: {}. Falling back to non-database runtime resolution.",
                module,
                error
            );
            module_status_from_location(module, fallback_location)
        }
    }
}

async fn get_supported_runtime_location_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    module: &str,
) -> Result<RuntimeLocationInfo, String> {
    match module {
        "opencode" => get_opencode_runtime_location_async(db).await,
        "claude" => get_claude_runtime_location_async(db).await,
        "codex" => get_codex_runtime_location_async(db).await,
        "openclaw" => get_openclaw_runtime_location_async(db).await,
        other => Err(format!("Unsupported runtime module: {}", other)),
    }
}

async fn get_wsl_direct_status_with_fallback(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    module: &str,
) -> Result<WslDirectModuleStatus, String> {
    if !MODULE_KEYS.contains(&module) {
        return Err(format!("Unsupported runtime module: {}", module));
    }

    let fallback_location = get_runtime_location_without_db(module);
    let runtime_result = get_supported_runtime_location_async(db, module).await;

    Ok(module_status_from_runtime_result(
        module,
        runtime_result,
        &fallback_location,
    ))
}

pub async fn get_wsl_direct_status_map_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<Vec<WslDirectModuleStatus>, String> {
    let mut statuses = Vec::with_capacity(MODULE_KEYS.len());

    for module in MODULE_KEYS {
        statuses.push(get_wsl_direct_status_with_fallback(db, module).await?);
    }

    Ok(statuses)
}

pub fn get_wsl_direct_status_for_module(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    module: &str,
) -> Result<WslDirectModuleStatus, String> {
    let _ = db;
    match module {
        "opencode" | "claude" | "codex" | "openclaw" => Ok(module_status_from_location(
            module,
            &get_runtime_location_without_db(module),
        )),
        other => Err(format!("Unsupported runtime module: {}", other)),
    }
}

pub async fn get_wsl_direct_status_for_module_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    module: &str,
) -> Result<WslDirectModuleStatus, String> {
    get_wsl_direct_status_with_fallback(db, module).await
}

pub fn get_opencode_runtime_location_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<RuntimeLocationInfo, String> {
    let _ = db;
    Ok(get_runtime_location_without_db("opencode"))
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
    Ok(resolve_omo_config_path_from_dir(&dir))
}

pub async fn get_omo_config_path_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    let dir = get_opencode_config_dir_async(db).await?;
    Ok(resolve_omo_config_path_from_dir(&dir))
}

fn resolve_omo_config_path_from_dir(dir: &Path) -> PathBuf {
    let candidates = [
        dir.join(format!("{OMO_CANONICAL_BASENAME}.jsonc")),
        dir.join(format!("{OMO_CANONICAL_BASENAME}.json")),
        dir.join(format!("{OMO_LEGACY_BASENAME}.jsonc")),
        dir.join(format!("{OMO_LEGACY_BASENAME}.json")),
    ];

    candidates
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(|| dir.join(format!("{OMO_CANONICAL_BASENAME}.jsonc")))
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
    let _ = db;
    Ok(get_runtime_location_without_db("claude"))
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
    let _ = db;
    Ok(get_runtime_location_without_db("codex"))
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
    let _ = db;
    Ok(get_runtime_location_without_db("openclaw"))
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
        "openclaw" => get_openclaw_runtime_location_sync(db)
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
        "openclaw" => get_openclaw_runtime_location_async(db)
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

fn get_runtime_location_without_db(module: &str) -> RuntimeLocationInfo {
    let (path, source) = resolve_config_path_without_db(module);
    build_runtime_location(path, source)
}

fn resolve_config_path_without_db(module: &str) -> (PathBuf, String) {
    match module {
        "opencode" => resolve_opencode_path_without_db(),
        "claude" => resolve_claude_path_without_db(),
        "codex" => resolve_codex_path_without_db(),
        "openclaw" => resolve_openclaw_path_without_db(),
        _ => (PathBuf::new(), "default".to_string()),
    }
}

fn resolve_opencode_path_without_db() -> (PathBuf, String) {
    if let Ok(env_path) = std::env::var("OPENCODE_CONFIG") {
        if !env_path.trim().is_empty() {
            return (PathBuf::from(env_path), "env".to_string());
        }
    }

    if let Some(shell_path) = shell_env::get_env_from_shell_config("OPENCODE_CONFIG") {
        if !shell_path.trim().is_empty() {
            return (PathBuf::from(shell_path), "shell".to_string());
        }
    }

    (
        PathBuf::from(
            open_code::get_default_config_path()
                .unwrap_or_else(|_| "~/.config/opencode/opencode.jsonc".to_string()),
        ),
        "default".to_string(),
    )
}

fn resolve_claude_path_without_db() -> (PathBuf, String) {
    if let Ok(env_path) = std::env::var("CLAUDE_CONFIG_DIR") {
        if !env_path.trim().is_empty() {
            return (PathBuf::from(env_path), "env".to_string());
        }
    }

    if let Some(shell_path) = shell_env::get_env_from_shell_config("CLAUDE_CONFIG_DIR") {
        if !shell_path.trim().is_empty() {
            return (PathBuf::from(shell_path), "shell".to_string());
        }
    }

    (
        claude_code::get_claude_default_root_dir().unwrap_or_else(|_| PathBuf::from("~/.claude")),
        "default".to_string(),
    )
}

fn resolve_codex_path_without_db() -> (PathBuf, String) {
    if let Ok(env_path) = std::env::var("CODEX_HOME") {
        if !env_path.trim().is_empty() {
            return (PathBuf::from(env_path), "env".to_string());
        }
    }

    if let Some(shell_path) = shell_env::get_env_from_shell_config("CODEX_HOME") {
        if !shell_path.trim().is_empty() {
            return (PathBuf::from(shell_path), "shell".to_string());
        }
    }

    (
        codex::get_codex_default_root_dir().unwrap_or_else(|_| PathBuf::from("~/.codex")),
        "default".to_string(),
    )
}

fn resolve_openclaw_path_without_db() -> (PathBuf, String) {
    (
        PathBuf::from(
            open_claw::get_default_config_path_for_runtime()
                .unwrap_or_else(|_| "~/.openclaw/openclaw.json".to_string()),
        ),
        "default".to_string(),
    )
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

#[cfg(test)]
mod tests {
    use super::{
        module_status_from_runtime_result, RuntimeLocationInfo, RuntimeLocationMode,
        WslLocationInfo,
    };
    use std::path::PathBuf;

    #[test]
    fn runtime_result_uses_resolved_wsl_location_when_available() {
        let fallback_location = RuntimeLocationInfo {
            mode: RuntimeLocationMode::LocalWindows,
            source: "default".to_string(),
            host_path: PathBuf::from("C:\\Users\\tester\\.codex"),
            wsl: None,
        };
        let resolved_location = RuntimeLocationInfo {
            mode: RuntimeLocationMode::WslDirect,
            source: "custom".to_string(),
            host_path: PathBuf::from("\\\\wsl.localhost\\Ubuntu\\home\\tester\\.codex"),
            wsl: Some(WslLocationInfo {
                distro: "Ubuntu".to_string(),
                linux_path: "/home/tester/.codex".to_string(),
                linux_user_root: Some("/home/tester".to_string()),
            }),
        };

        let status =
            module_status_from_runtime_result("codex", Ok(resolved_location), &fallback_location);

        assert!(status.is_wsl_direct);
        assert_eq!(status.module, "codex");
        assert_eq!(status.distro.as_deref(), Some("Ubuntu"));
        assert_eq!(status.linux_path.as_deref(), Some("/home/tester/.codex"));
    }

    #[test]
    fn runtime_result_falls_back_to_non_database_location_on_error() {
        let fallback_location = RuntimeLocationInfo {
            mode: RuntimeLocationMode::LocalWindows,
            source: "default".to_string(),
            host_path: PathBuf::from("C:\\Users\\tester\\.codex"),
            wsl: None,
        };

        let status = module_status_from_runtime_result(
            "codex",
            Err("simulated db decode failure".to_string()),
            &fallback_location,
        );

        assert_eq!(status.module, "codex");
        assert!(!status.is_wsl_direct);
        assert_eq!(
            status.source_path.as_deref(),
            Some("C:\\Users\\tester\\.codex")
        );
        assert_eq!(status.reason, None);
    }
}
