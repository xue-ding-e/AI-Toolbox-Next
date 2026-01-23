use chrono::Local;
use std::fs;
use serde_json::Value;

use crate::db::DbState;
use super::adapter;
use super::types::*;
use tauri::Emitter;

// ============================================================================
// Oh My OpenCode Slim Config Commands
// ============================================================================

/// List all oh-my-opencode-slim configs ordered by name
#[tauri::command]
pub async fn list_oh_my_opencode_slim_configs(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<OhMyOpenCodeSlimConfig>, String> {
    let db = state.0.lock().await;

    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM oh_my_opencode_slim_config")
        .await
        .map_err(|e| format!("Failed to query configs: {}", e))?
        .take(0);

    match records_result {
        Ok(records) => {
            // 如果数据库为空，尝试从本地配置文件导入
            if records.is_empty() {
                if let Ok(imported_config) = import_local_config_if_exists(&db).await {
                    // 成功导入，返回包含这个配置的列表
                    return Ok(vec![imported_config]);
                }
            }

            let mut result: Vec<OhMyOpenCodeSlimConfig> = records
                .into_iter()
                .map(adapter::from_db_value)
                .collect();
            // Sort by name
            result.sort_by_key(|c| c.name.clone());
            Ok(result)
        }
        Err(e) => {
            eprintln!("Failed to deserialize configs: {}", e);
            Ok(Vec::new())
        }
    }
}

/// Helper function to get oh-my-opencode-slim config path
/// omos 只支持 .json 格式（不支持 jsonc）
pub fn get_oh_my_opencode_slim_config_path() -> Result<std::path::PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or("Failed to get home directory")?;

    let opencode_dir = home_dir.join(".config").join("opencode");
    let json_path = opencode_dir.join("oh-my-opencode-slim.json");

    Ok(json_path)
}

/// 从本地配置文件导入配置（如果存在）
async fn import_local_config_if_exists(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<OhMyOpenCodeSlimConfig, String> {
    let config_path = get_oh_my_opencode_slim_config_path()
        .map_err(|_| "Local config file not found".to_string())?;

    // 读取文件内容
    let file_content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read local config file: {}", e))?;

    // 解析 JSON（使用 json5 支持带注释的 JSONC 格式）
    let json_value: Value = json5::from_str(&file_content)
        .map_err(|e| format!("Failed to parse local config file: {}", e))?;

    // 提取 agents 配置
    let agents = json_value
        .get("agents")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    // 提取 other_fields（除了 agents 和全局配置字段之外的所有字段）
    let mut other_fields = json_value.clone();
    if let Some(obj) = other_fields.as_object_mut() {
        obj.remove("agents");
        obj.remove("$schema");
        obj.remove("sisyphus_agent");
        obj.remove("sisyphusAgent");
        obj.remove("disabled_agents");
        obj.remove("disabledAgents");
        obj.remove("disabled_mcps");
        obj.remove("disabledMcps");
        obj.remove("disabled_hooks");
        obj.remove("disabledHooks");
        obj.remove("lsp");
        obj.remove("experimental");
    }

    let other_fields_value = if other_fields.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        None
    } else {
        Some(other_fields)
    };

    let now = Local::now().to_rfc3339();

    // 同时导入 Global Config（如果数据库中不存在）
    let global_exists: Result<Vec<Value>, _> = db
        .query("SELECT id FROM oh_my_opencode_slim_global_config:`global` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to check global config: {}", e))?
        .take(0);

    let should_import_global = match global_exists {
        Ok(records) => records.is_empty(),
        Err(_) => true,
    };

    if should_import_global {
        // 提取全局配置字段
        let sisyphus_agent = json_value
            .get("sisyphus_agent")
            .or_else(|| json_value.get("sisyphusAgent"))
            .cloned();

        let disabled_agents: Option<Vec<String>> = json_value
            .get("disabled_agents")
            .or_else(|| json_value.get("disabledAgents"))
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let disabled_mcps: Option<Vec<String>> = json_value
            .get("disabled_mcps")
            .or_else(|| json_value.get("disabledMcps"))
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let disabled_hooks: Option<Vec<String>> = json_value
            .get("disabled_hooks")
            .or_else(|| json_value.get("disabledHooks"))
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let lsp = json_value.get("lsp").cloned();
        let experimental = json_value.get("experimental").cloned();

        let global_content = OhMyOpenCodeSlimGlobalConfigContent {
            sisyphus_agent,
            disabled_agents,
            disabled_mcps,
            disabled_hooks,
            lsp,
            experimental,
            other_fields: None,
            updated_at: now.clone(),
        };

        let global_json_data = adapter::global_config_to_db_value(&global_content);

        // 保存 Global Config
        if let Err(e) = db.query("UPSERT oh_my_opencode_slim_global_config:`global` CONTENT $data")
            .bind(("data", global_json_data))
            .await
        {
            eprintln!("[WARN] Failed to import global config: {}", e);
        }
    }

    // 创建配置内容
    let content = OhMyOpenCodeSlimConfigContent {
        name: "本地配置".to_string(),
        is_applied: true,
        agents,
        other_fields: other_fields_value,
        created_at: now.clone(),
        updated_at: now,
    };

    let json_data = adapter::to_db_value(&content);

    // 使用 UPSERT 模式
    db.query("UPSERT oh_my_opencode_slim_config CONTENT $data")
        .bind(("data", json_data))
        .await
        .map_err(|e| format!("Failed to import config: {}", e))?;

    // 从数据库读取刚导入的配置
    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM oh_my_opencode_slim_config WHERE is_applied = true ORDER BY created_at DESC LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query imported config: {}", e))?
        .take(0);

    match records_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                Ok(adapter::from_db_value(record.clone()))
            } else {
                Err("Failed to retrieve imported config".to_string())
            }
        }
        Err(e) => Err(format!("Failed to import config: {}", e)),
    }
}

/// Create a new oh-my-opencode-slim config
#[tauri::command]
pub async fn create_oh_my_opencode_slim_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: OhMyOpenCodeSlimConfigInput,
) -> Result<OhMyOpenCodeSlimConfig, String> {
    let db = state.0.lock().await;

    let now = Local::now().to_rfc3339();
    let content = OhMyOpenCodeSlimConfigContent {
        name: input.name.clone(),
        is_applied: false,
        agents: input.agents.clone(),
        other_fields: input.other_fields.clone(),
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    let json_data = adapter::to_db_value(&content);

    db.query("CREATE oh_my_opencode_slim_config CONTENT $data")
        .bind(("data", json_data))
        .await
        .map_err(|e| format!("Failed to create config: {}", e))?;

    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM oh_my_opencode_slim_config ORDER BY created_at DESC LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query new config: {}", e))?
        .take(0);

    let _ = app.emit("config-changed", "window");

    match records_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                Ok(adapter::from_db_value(record.clone()))
            } else {
                Err("Failed to retrieve created config".to_string())
            }
        }
        Err(e) => Err(format!("Failed to create config: {}", e)),
    }
}

/// Update an existing oh-my-opencode-slim config
#[tauri::command]
#[allow(unused_variables)]
pub async fn update_oh_my_opencode_slim_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: OhMyOpenCodeSlimConfigInput,
) -> Result<OhMyOpenCodeSlimConfig, String> {
    let db = state.0.lock().await;

    let config_id = input.id.ok_or_else(|| "ID is required for update".to_string())?;

    let check_result: Result<Vec<Value>, _> = db
        .query("SELECT * FROM type::thing('oh_my_opencode_slim_config', $id) LIMIT 1")
        .bind(("id", config_id.clone()))
        .await
        .map_err(|e| format!("Failed to check config existence: {}", e))?
        .take(0);

    if let Ok(records) = check_result {
        if records.is_empty() {
            return Err(format!(
                "Oh-my-opencode-slim config with ID '{}' not found",
                config_id
            ));
        }
    }

    let now = Local::now().to_rfc3339();

    let existing_result: Result<Vec<serde_json::Value>, _> = db
        .query(format!(
            "SELECT created_at, type::bool(is_applied) as is_applied FROM oh_my_opencode_slim_config:`{}` LIMIT 1",
            config_id
        ))
        .await
        .map_err(|e| format!("Failed to query config: {}", e))?
        .take(0);

    let (is_applied_value, created_at) = match existing_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                let is_applied = record
                    .get("is_applied")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let created = record
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_else(|| Local::now().to_rfc3339());
                (is_applied, created)
            } else {
                (false, Local::now().to_rfc3339())
            }
        }
        Err(_) => {
            (false, Local::now().to_rfc3339())
        }
    };

    let content = OhMyOpenCodeSlimConfigContent {
        name: input.name,
        is_applied: is_applied_value,
        agents: input.agents,
        other_fields: input.other_fields,
        created_at,
        updated_at: now,
    };

    let json_data = adapter::to_db_value(&content);

    let json_str = serde_json::to_string(&json_data)
        .map_err(|e| format!("Failed to serialize json_data: {}", e))?;

    db.query(format!("UPDATE oh_my_opencode_slim_config:`{}` CONTENT {}", config_id, json_str))
        .await
        .map_err(|e| format!("Failed to update config: {}", e))?;

    if is_applied_value {
        if let Err(e) = apply_config_to_file(&db, &config_id).await {
            eprintln!("Failed to auto-apply updated config: {}", e);
        } else {
            #[cfg(target_os = "windows")]
            let _ = app.emit("wsl-sync-request-opencode", ());
        }
    }

    Ok(OhMyOpenCodeSlimConfig {
        id: config_id,
        name: content.name,
        is_applied: is_applied_value,
        agents: content.agents,
        other_fields: content.other_fields,
        created_at: Some(content.created_at),
        updated_at: Some(content.updated_at),
    })
}

/// Delete an oh-my-opencode-slim config
#[tauri::command]
pub async fn delete_oh_my_opencode_slim_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let db = state.0.lock().await;

    db.query(format!("DELETE oh_my_opencode_slim_config:`{}`", id))
        .await
        .map_err(|e| format!("Failed to delete config: {}", e))?;

    let _ = app.emit("config-changed", "window");

    Ok(())
}

/// 内部函数：将指定配置应用到配置文件
async fn apply_config_to_file(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    config_id: &str,
) -> Result<(), String> {
    apply_config_to_file_public(db, config_id).await
}

/// Public version of apply_config_to_file for tray module
pub async fn apply_config_to_file_public(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    config_id: &str,
) -> Result<(), String> {
    let records_result: Result<Vec<Value>, _> = db
        .query(format!(
            "SELECT *, type::string(id) as id FROM oh_my_opencode_slim_config:`{}` LIMIT 1",
            config_id
        ))
        .await
        .map_err(|e| format!("Failed to query config: {}", e))?
        .take(0);

    let agents_profile = match records_result {
        Ok(records) => {
            if records.is_empty() {
                return Err(format!("Config '{}' not found", config_id));
            }
            adapter::from_db_value(records[0].clone())
        }
        Err(e) => return Err(format!("Failed to get config: {}", e)),
    };

    let config_path = get_oh_my_opencode_slim_config_path()?;

    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create opencode config directory: {}", e))?;
        }
    }

    // 获取 Global Config
    let global_records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM oh_my_opencode_slim_global_config:`global` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query global config: {}", e))?
        .take(0);

    let global_config = match global_records_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                adapter::global_config_from_db_value(record.clone())
            } else {
                OhMyOpenCodeSlimGlobalConfig {
                    id: "global".to_string(),
                    sisyphus_agent: None,
                    disabled_agents: None,
                    disabled_mcps: None,
                    disabled_hooks: None,
                    lsp: None,
                    experimental: None,
                    other_fields: None,
                    updated_at: None,
                }
            }
        }
        Err(_) => {
            OhMyOpenCodeSlimGlobalConfig {
                id: "global".to_string(),
                sisyphus_agent: None,
                disabled_agents: None,
                disabled_mcps: None,
                disabled_hooks: None,
                lsp: None,
                experimental: None,
                other_fields: None,
                updated_at: None,
            }
        }
    };

    let mut final_json = serde_json::Map::new();

    // omos 不需要 $schema 字段

    if let Some(sisyphus) = global_config.sisyphus_agent {
        final_json.insert("sisyphus_agent".to_string(), sisyphus);
    }
    if let Some(disabled_agents) = global_config.disabled_agents {
        final_json.insert("disabled_agents".to_string(), serde_json::json!(disabled_agents));
    }
    if let Some(disabled_mcps) = global_config.disabled_mcps {
        final_json.insert("disabled_mcps".to_string(), serde_json::json!(disabled_mcps));
    }
    if let Some(disabled_hooks) = global_config.disabled_hooks {
        final_json.insert("disabled_hooks".to_string(), serde_json::json!(disabled_hooks));
    }
    if let Some(lsp) = global_config.lsp {
        final_json.insert("lsp".to_string(), lsp);
    }
    if let Some(experimental) = global_config.experimental {
        final_json.insert("experimental".to_string(), experimental);
    }

    if let Some(global_others) = global_config.other_fields {
        if let Some(others_obj) = global_others.as_object() {
            for (key, value) in others_obj {
                final_json.insert(key.clone(), value.clone());
            }
        }
    }

    if let Some(agents) = agents_profile.agents {
        final_json.insert("agents".to_string(), agents);
    }

    if let Some(profile_others) = agents_profile.other_fields {
        if let Some(others_obj) = profile_others.as_object() {
            for (key, value) in others_obj {
                final_json.insert(key.clone(), value.clone());
            }
        }
    }

    let mut final_json = Value::Object(final_json);

    adapter::clean_empty_values(&mut final_json);

    let json_content = serde_json::to_string_pretty(&final_json)
        .map_err(|e| format!("Failed to serialize final config: {}", e))?;

    fs::write(&config_path, json_content)
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    Ok(())
}

/// Apply an oh-my-opencode-slim config to the JSON file
#[tauri::command]
pub async fn apply_oh_my_opencode_slim_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    config_id: String,
) -> Result<(), String> {
    let db = state.0.lock().await;
    apply_config_internal(&db, &app, &config_id, false).await?;
    Ok(())
}

/// Internal function to apply config
pub async fn apply_config_internal<R: tauri::Runtime>(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    app: &tauri::AppHandle<R>,
    config_id: &str,
    from_tray: bool,
) -> Result<(), String> {
    apply_config_to_file(db, config_id).await?;

    let now = Local::now().to_rfc3339();

    db.query("UPDATE oh_my_opencode_slim_config SET is_applied = false, updated_at = $now WHERE is_applied = true")
        .bind(("now", now.clone()))
        .await
        .map_err(|e| format!("Failed to clear applied flags: {}", e))?;

    db.query("UPDATE oh_my_opencode_slim_config SET is_applied = true, updated_at = $now WHERE id = type::thing('oh_my_opencode_slim_config', $id)")
        .bind(("id", config_id.to_string()))
        .bind(("now", now))
        .await
        .map_err(|e| format!("Failed to update applied flag: {}", e))?;

    let payload = if from_tray { "tray" } else { "window" };
    let _ = app.emit("config-changed", payload);

    #[cfg(target_os = "windows")]
    let _ = app.emit("wsl-sync-request-opencode", ());

    Ok(())
}

/// Reorder oh-my-opencode-slim configs
#[tauri::command]
pub async fn reorder_oh_my_opencode_slim_configs(
    state: tauri::State<'_, DbState>,
    ids: Vec<String>,
) -> Result<(), String> {
    let db = state.0.lock().await;

    for (index, id) in ids.iter().enumerate() {
        db.query(format!(
            "UPDATE oh_my_opencode_slim_config:`{}` SET sort_index = $index",
            id
        ))
        .bind(("index", index as i32))
        .await
        .map_err(|e| format!("Failed to update sort index: {}", e))?;
    }

    Ok(())
}

/// Get oh-my-opencode-slim config file path info
#[tauri::command]
pub async fn get_oh_my_opencode_slim_config_path_info() -> Result<ConfigPathInfo, String> {
    let config_path = get_oh_my_opencode_slim_config_path()?;
    let path = config_path.to_string_lossy().to_string();

    Ok(ConfigPathInfo {
        path,
        source: "default".to_string(),
    })
}

/// Check if local oh-my-opencode-slim config file exists
/// omos 只支持 .json 格式
#[tauri::command]
pub async fn check_oh_my_opencode_slim_config_exists() -> Result<bool, String> {
    let home_dir = dirs::home_dir()
        .ok_or("Failed to get home directory")?;

    let opencode_dir = home_dir.join(".config").join("opencode");
    let json_path = opencode_dir.join("oh-my-opencode-slim.json");

    Ok(json_path.exists())
}

// ============================================================================
// Oh My OpenCode Slim Global Config Commands
// ============================================================================

/// Get oh-my-opencode-slim global config
#[tauri::command]
pub async fn get_oh_my_opencode_slim_global_config(
    state: tauri::State<'_, DbState>,
) -> Result<OhMyOpenCodeSlimGlobalConfig, String> {
    let db = state.0.lock().await;

    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM oh_my_opencode_slim_global_config:`global` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query global config: {}", e))?
        .take(0);

    match records_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                Ok(adapter::global_config_from_db_value(record.clone()))
            } else {
                if let Ok(imported_config) = import_local_global_config_if_exists(&db).await {
                    return Ok(imported_config);
                }

                let default_config = OhMyOpenCodeSlimGlobalConfig {
                    id: "global".to_string(),
                    sisyphus_agent: None,
                    disabled_agents: None,
                    disabled_mcps: None,
                    disabled_hooks: None,
                    lsp: None,
                    experimental: None,
                    other_fields: None,
                    updated_at: None,
                };

                let now = Local::now().to_rfc3339();
                let content = OhMyOpenCodeSlimGlobalConfigContent {
                    sisyphus_agent: None,
                    disabled_agents: None,
                    disabled_mcps: None,
                    disabled_hooks: None,
                    lsp: None,
                    experimental: None,
                    other_fields: None,
                    updated_at: now,
                };
                let json_data = adapter::global_config_to_db_value(&content);
                if db.query("UPSERT oh_my_opencode_slim_global_config:`global` CONTENT $data")
                    .bind(("data", json_data))
                    .await
                    .is_ok() {
                }

                Ok(default_config)
            }
        }
        Err(e) => {
            eprintln!("Failed to get global config: {}", e);
            Ok(OhMyOpenCodeSlimGlobalConfig {
                id: "global".to_string(),
                sisyphus_agent: None,
                disabled_agents: None,
                disabled_mcps: None,
                disabled_hooks: None,
                lsp: None,
                experimental: None,
                other_fields: None,
                updated_at: None,
            })
        }
    }
}

/// 从本地配置文件导入全局配置
async fn import_local_global_config_if_exists(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<OhMyOpenCodeSlimGlobalConfig, String> {
    let config_path = get_oh_my_opencode_slim_config_path()
        .map_err(|_| "Local config file not found".to_string())?;

    let file_content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read local config file: {}", e))?;

    let json_value: Value = json5::from_str(&file_content)
        .map_err(|e| format!("Failed to parse local config file: {}", e))?;

    let sisyphus_agent = json_value
        .get("sisyphus_agent")
        .or_else(|| json_value.get("sisyphusAgent"))
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let disabled_agents = json_value
        .get("disabled_agents")
        .or_else(|| json_value.get("disabledAgents"))
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let disabled_mcps = json_value
        .get("disabled_mcps")
        .or_else(|| json_value.get("disabledMcps"))
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let disabled_hooks = json_value
        .get("disabled_hooks")
        .or_else(|| json_value.get("disabledHooks"))
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let lsp = json_value
        .get("lsp")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let experimental = json_value
        .get("experimental")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let mut other_fields = json_value.clone();
    if let Some(obj) = other_fields.as_object_mut() {
        obj.remove("agents");
        obj.remove("$schema");
        obj.remove("sisyphus_agent");
        obj.remove("sisyphusAgent");
        obj.remove("disabled_agents");
        obj.remove("disabledAgents");
        obj.remove("disabled_mcps");
        obj.remove("disabledMcps");
        obj.remove("disabled_hooks");
        obj.remove("disabledHooks");
        obj.remove("lsp");
        obj.remove("experimental");
    }

    let other_fields_value = if other_fields.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        None
    } else {
        Some(other_fields)
    };

    let now = Local::now().to_rfc3339();

    let content = OhMyOpenCodeSlimGlobalConfigContent {
        sisyphus_agent,
        disabled_agents,
        disabled_mcps,
        disabled_hooks,
        lsp,
        experimental,
        other_fields: other_fields_value,
        updated_at: now,
    };

    let json_data = adapter::global_config_to_db_value(&content);

    db.query("UPSERT oh_my_opencode_slim_global_config:`global` CONTENT $data")
        .bind(("data", json_data))
        .await
        .map_err(|e| format!("Failed to import global config: {}", e))?;

    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM oh_my_opencode_slim_global_config:`global` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query imported global config: {}", e))?
        .take(0);

    match records_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                Ok(adapter::global_config_from_db_value(record.clone()))
            } else {
                Err("Failed to retrieve imported global config".to_string())
            }
        }
        Err(e) => Err(format!("Failed to import global config: {}", e)),
    }
}

/// Save oh-my-opencode-slim global config
#[tauri::command]
#[allow(unused_variables)]
pub async fn save_oh_my_opencode_slim_global_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: OhMyOpenCodeSlimGlobalConfigInput,
) -> Result<OhMyOpenCodeSlimGlobalConfig, String> {
    let db = state.0.lock().await;

    let now = Local::now().to_rfc3339();
    let content = OhMyOpenCodeSlimGlobalConfigContent {
        sisyphus_agent: input.sisyphus_agent,
        disabled_agents: input.disabled_agents,
        disabled_mcps: input.disabled_mcps,
        disabled_hooks: input.disabled_hooks,
        lsp: input.lsp,
        experimental: input.experimental,
        other_fields: input.other_fields,
        updated_at: now.clone(),
    };

    let json_data = adapter::global_config_to_db_value(&content);

    db.query("UPSERT oh_my_opencode_slim_global_config:`global` CONTENT $data")
        .bind(("data", json_data))
        .await
        .map_err(|e| format!("Failed to save global config: {}", e))?;

    let applied_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM oh_my_opencode_slim_config WHERE is_applied = true LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query applied config: {}", e))?
        .take(0);

    if let Ok(records) = applied_result {
        if let Some(record) = records.first() {
            let applied_config = adapter::from_db_value(record.clone());
            if apply_config_to_file(&db, &applied_config.id).await.is_ok() {
                #[cfg(target_os = "windows")]
                let _ = app.emit("wsl-sync-request-opencode", ());
            }
        }
    }

    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM oh_my_opencode_slim_global_config:`global` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query saved global config: {}", e))?
        .take(0);

    match records_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                Ok(adapter::global_config_from_db_value(record.clone()))
            } else {
                Err("Failed to retrieve saved global config".to_string())
            }
        }
        Err(e) => Err(format!("Failed to save global config: {}", e)),
    }
}
