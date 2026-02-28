use std::fs;
use std::path::Path;
use serde_json::Value;
use tauri::Emitter;

use super::adapter;
use super::types::*;
use crate::db::DbState;

// ============================================================================
// Helper Functions
// ============================================================================

/// Get default config path: ~/.openclaw/openclaw.json
fn get_default_config_path() -> Result<String, String> {
    let home_dir = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| "Failed to get home directory".to_string())?;

    let config_path = Path::new(&home_dir)
        .join(".openclaw")
        .join("openclaw.json");

    Ok(config_path.to_string_lossy().to_string())
}

/// Internal function to save config and emit events
pub async fn apply_config_internal<R: tauri::Runtime>(
    state: tauri::State<'_, DbState>,
    app: &tauri::AppHandle<R>,
    config: OpenClawConfig,
    from_tray: bool,
) -> Result<(), String> {
    let config_path_str = get_openclaw_config_path(state).await?;
    let config_path = Path::new(&config_path_str);

    // Ensure directory exists
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }
    }

    // Serialize with pretty printing
    let json_content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    fs::write(config_path, json_content)
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    let payload = if from_tray {
        "tray"
    } else {
        "window"
    };
    let _ = app.emit("openclaw-config-changed", payload);

    Ok(())
}

/// Read and parse the config file, returning the OpenClawConfig
async fn read_and_parse_config(
    state: tauri::State<'_, DbState>,
) -> Result<OpenClawConfig, String> {
    let result = read_openclaw_config(state).await?;
    match result {
        ReadOpenClawConfigResult::Success { config } => Ok(config),
        ReadOpenClawConfigResult::NotFound { path: _ } => {
            // Return empty config for non-existent file
            Ok(OpenClawConfig {
                models: None,
                agents: None,
                env: None,
                tools: None,
                other: serde_json::Map::new(),
            })
        }
        ReadOpenClawConfigResult::ParseError { error, .. } => {
            Err(format!("Config parse error: {}", error))
        }
        ReadOpenClawConfigResult::Error { error } => Err(error),
    }
}

// ============================================================================
// Config Path Commands
// ============================================================================

/// Get OpenClaw config file path with priority: common config > default
#[tauri::command]
pub async fn get_openclaw_config_path(
    state: tauri::State<'_, DbState>,
) -> Result<String, String> {
    // 1. Check common config for custom path
    if let Some(common_config) = get_openclaw_common_config(state.clone()).await? {
        if let Some(custom_path) = common_config.config_path {
            if !custom_path.is_empty() {
                return Ok(custom_path);
            }
        }
    }

    // 2. Return default path
    get_default_config_path()
}

/// Get OpenClaw config path info including source
#[tauri::command]
pub async fn get_openclaw_config_path_info(
    state: tauri::State<'_, DbState>,
) -> Result<OpenClawConfigPathInfo, String> {
    // 1. Check common config for custom path
    if let Some(common_config) = get_openclaw_common_config(state.clone()).await? {
        if let Some(custom_path) = common_config.config_path {
            if !custom_path.is_empty() {
                return Ok(OpenClawConfigPathInfo {
                    path: custom_path,
                    source: "custom".to_string(),
                });
            }
        }
    }

    // 2. Return default path
    let default_path = get_default_config_path()?;
    Ok(OpenClawConfigPathInfo {
        path: default_path,
        source: "default".to_string(),
    })
}

// ============================================================================
// Config Read/Write Commands
// ============================================================================

/// Read OpenClaw configuration file with detailed result
#[tauri::command]
pub async fn read_openclaw_config(
    state: tauri::State<'_, DbState>,
) -> Result<ReadOpenClawConfigResult, String> {
    let config_path_str = get_openclaw_config_path(state).await?;
    let config_path = Path::new(&config_path_str);

    if !config_path.exists() {
        return Ok(ReadOpenClawConfigResult::NotFound {
            path: config_path_str,
        });
    }

    let content = match fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(e) => {
            return Ok(ReadOpenClawConfigResult::Error {
                error: format!("Failed to read config file: {}", e),
            })
        }
    };

    match json5::from_str::<OpenClawConfig>(&content) {
        Ok(config) => Ok(ReadOpenClawConfigResult::Success { config }),
        Err(e) => {
            let preview = if content.len() > 500 {
                format!("{}...", &content[..500])
            } else {
                content
            };

            Ok(ReadOpenClawConfigResult::ParseError {
                path: config_path_str,
                error: e.to_string(),
                content_preview: Some(preview),
            })
        }
    }
}

/// Save OpenClaw configuration file (full replacement)
#[tauri::command]
pub async fn save_openclaw_config<R: tauri::Runtime>(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle<R>,
    config: OpenClawConfig,
) -> Result<(), String> {
    apply_config_internal(state, &app, config, false).await
}

/// Backup OpenClaw configuration file
#[tauri::command]
pub async fn backup_openclaw_config(
    state: tauri::State<'_, DbState>,
) -> Result<String, String> {
    let config_path_str = get_openclaw_config_path(state).await?;
    let config_path = Path::new(&config_path_str);

    if !config_path.exists() {
        return Err("Config file does not exist".to_string());
    }

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_path_str = format!("{}.bak.{}", config_path_str, timestamp);

    fs::copy(config_path, &backup_path_str)
        .map_err(|e| format!("Failed to backup config file: {}", e))?;

    Ok(backup_path_str)
}

// ============================================================================
// Common Config Commands (DB)
// ============================================================================

/// Get OpenClaw common config from database
#[tauri::command]
pub async fn get_openclaw_common_config(
    state: tauri::State<'_, DbState>,
) -> Result<Option<OpenClawCommonConfig>, String> {
    let db = state.0.lock().await;

    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM openclaw_common_config:`common` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query openclaw common config: {}", e))?
        .take(0);

    match records_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                Ok(Some(adapter::from_db_value(record.clone())))
            } else {
                Ok(None)
            }
        }
        Err(e) => {
            eprintln!(
                "OpenClaw common config has incompatible format, cleaning up: {}",
                e
            );
            let _ = db
                .query("DELETE openclaw_common_config:`common`")
                .await;
            Ok(None)
        }
    }
}

/// Save OpenClaw common config to database
#[tauri::command]
pub async fn save_openclaw_common_config(
    state: tauri::State<'_, DbState>,
    config: OpenClawCommonConfig,
) -> Result<(), String> {
    let db = state.0.lock().await;

    let json_data = adapter::to_db_value(&config);

    db.query("UPSERT openclaw_common_config:`common` CONTENT $data")
        .bind(("data", json_data))
        .await
        .map_err(|e| format!("Failed to save openclaw common config: {}", e))?;

    Ok(())
}

// ============================================================================
// Agents Defaults Commands
// ============================================================================

/// Get agents.defaults from config file
#[tauri::command]
pub async fn get_openclaw_agents_defaults(
    state: tauri::State<'_, DbState>,
) -> Result<Option<OpenClawAgentsDefaults>, String> {
    let config = read_and_parse_config(state).await?;
    Ok(config
        .agents
        .and_then(|a| a.defaults))
}

/// Set agents.defaults in config file (read-modify-write)
#[tauri::command]
pub async fn set_openclaw_agents_defaults<R: tauri::Runtime>(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle<R>,
    defaults: OpenClawAgentsDefaults,
) -> Result<(), String> {
    let mut config = read_and_parse_config(state.clone()).await?;

    // Ensure agents section exists
    let mut agents = config.agents.unwrap_or(OpenClawAgentsSection {
        defaults: None,
        extra: std::collections::HashMap::new(),
    });
    agents.defaults = Some(defaults);
    config.agents = Some(agents);

    apply_config_internal(state, &app, config, false).await
}

// ============================================================================
// Env Commands
// ============================================================================

/// Get env section from config file
#[tauri::command]
pub async fn get_openclaw_env(
    state: tauri::State<'_, DbState>,
) -> Result<Option<OpenClawEnvConfig>, String> {
    let config = read_and_parse_config(state).await?;
    Ok(config.env)
}

/// Set env section in config file (read-modify-write)
#[tauri::command]
pub async fn set_openclaw_env<R: tauri::Runtime>(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle<R>,
    env: OpenClawEnvConfig,
) -> Result<(), String> {
    let mut config = read_and_parse_config(state.clone()).await?;
    config.env = Some(env);
    apply_config_internal(state, &app, config, false).await
}

// ============================================================================
// Tools Commands
// ============================================================================

/// Get tools section from config file
#[tauri::command]
pub async fn get_openclaw_tools(
    state: tauri::State<'_, DbState>,
) -> Result<Option<OpenClawToolsConfig>, String> {
    let config = read_and_parse_config(state).await?;
    Ok(config.tools)
}

/// Set tools section in config file (read-modify-write)
#[tauri::command]
pub async fn set_openclaw_tools<R: tauri::Runtime>(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle<R>,
    tools: OpenClawToolsConfig,
) -> Result<(), String> {
    let mut config = read_and_parse_config(state.clone()).await?;
    config.tools = Some(tools);
    apply_config_internal(state, &app, config, false).await
}
