use chrono::Local;
use std::fs;
use serde_json::Value;

use crate::db::DbState;
use super::adapter;
use super::types::*;
use tauri::Emitter;

// ============================================================================
// Oh My OpenCode Config Commands
// ============================================================================

/// List all oh-my-opencode configs ordered by name
#[tauri::command]
pub async fn list_oh_my_opencode_configs(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<OhMyOpenCodeConfig>, String> {
    let db = state.0.lock().await;

    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM oh_my_opencode_config")
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
            
            let mut result: Vec<OhMyOpenCodeConfig> = records
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

/// Helper function to get oh-my-opencode config path
/// Priority: .jsonc (if exists) → .json (if exists) → default .jsonc
pub fn get_oh_my_opencode_config_path() -> Result<std::path::PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or("Failed to get home directory")?;

    let opencode_dir = home_dir.join(".config").join("opencode");

    // Check for .jsonc first, then .json
    let jsonc_path = opencode_dir.join("oh-my-opencode.jsonc");
    let json_path = opencode_dir.join("oh-my-opencode.json");

    if jsonc_path.exists() {
        Ok(jsonc_path)
    } else if json_path.exists() {
        Ok(json_path)
    } else {
        // Return default path for new file
        Ok(jsonc_path)
    }
}

/// 从本地配置文件导入配置（如果存在）
async fn import_local_config_if_exists(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<OhMyOpenCodeConfig, String> {
    let config_path = get_oh_my_opencode_config_path()
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
    // 全局配置字段会被同时导入到 Global Config 中
    let mut other_fields = json_value.clone();
    if let Some(obj) = other_fields.as_object_mut() {
        obj.remove("agents");
        obj.remove("$schema"); // 移除 schema 字段，因为它不是配置内容
        // 移除属于 Global Config 的字段，这些字段不应该放在 Agents Profile 的 other_fields 中
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
    // 检查 Global Config 是否已存在
    let global_exists: Result<Vec<Value>, _> = db
        .query("SELECT id FROM oh_my_opencode_global_config:`global` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to check global config: {}", e))?
        .take(0);

    let should_import_global = match global_exists {
        Ok(records) => records.is_empty(),
        Err(_) => true,
    };

    if should_import_global {
        // 提取全局配置字段
        let schema = json_value
            .get("$schema")
            .and_then(|v| v.as_str())
            .map(String::from);

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

        let global_content = OhMyOpenCodeGlobalConfigContent {
            schema,
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
        if let Err(e) = db.query("UPSERT oh_my_opencode_global_config:`global` CONTENT $data")
            .bind(("data", global_json_data))
            .await
        {
            eprintln!("[WARN] Failed to import global config: {}", e);
            // 不中断流程，继续导入 Agents Profile
        }
    }

    // 创建配置内容（不包含 config_id，让 SurrealDB 自动生成 ID）
    let content = OhMyOpenCodeConfigContent {
        name: "本地配置".to_string(),
        is_applied: true, // 标记为已应用，因为这是从当前使用的配置导入的
        agents,
        other_fields: other_fields_value,
        created_at: now.clone(),
        updated_at: now,
    };

    let json_data = adapter::to_db_value(&content);

    // 使用 UPSERT 模式，让 SurrealDB 自动生成 ID
    db.query("UPSERT oh_my_opencode_config CONTENT $data")
        .bind(("data", json_data))
        .await
        .map_err(|e| format!("Failed to import config: {}", e))?;

    // 从数据库读取刚导入的配置
    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM oh_my_opencode_config WHERE is_applied = true ORDER BY created_at DESC LIMIT 1")
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

/// Create a new oh-my-opencode config
#[tauri::command]
pub async fn create_oh_my_opencode_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: OhMyOpenCodeConfigInput,
) -> Result<OhMyOpenCodeConfig, String> {
    let db = state.0.lock().await;

    let now = Local::now().to_rfc3339();
    let content = OhMyOpenCodeConfigContent {
        name: input.name.clone(),
        is_applied: false,
        agents: input.agents.clone(),
        other_fields: input.other_fields.clone(),
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    let json_data = adapter::to_db_value(&content);

    // Use CREATE to let SurrealDB auto-generate ID (like ClaudeCode)
    db.query("CREATE oh_my_opencode_config CONTENT $data")
        .bind(("data", json_data))
        .await
        .map_err(|e| format!("Failed to create config: {}", e))?;

    // Fetch the created record to get the auto-generated ID
    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM oh_my_opencode_config ORDER BY created_at DESC LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query new config: {}", e))?
        .take(0);

    // Notify to refresh tray menu
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

/// Update an existing oh-my-opencode config
#[tauri::command]
pub async fn update_oh_my_opencode_config(
    state: tauri::State<'_, DbState>,
    _app: tauri::AppHandle,
    input: OhMyOpenCodeConfigInput,
) -> Result<OhMyOpenCodeConfig, String> {
    let db = state.0.lock().await;

    // ID is required for update
    let config_id = input.id.ok_or_else(|| "ID is required for update".to_string())?;

    // Check if config exists using type::thing
    let check_result: Result<Vec<Value>, _> = db
        .query("SELECT * FROM type::thing('oh_my_opencode_config', $id) LIMIT 1")
        .bind(("id", config_id.clone()))
        .await
        .map_err(|e| format!("Failed to check config existence: {}", e))?
        .take(0);

    if let Ok(records) = check_result {
        if records.is_empty() {
            return Err(format!(
                "Oh-my-opencode config with ID '{}' not found",
                config_id
            ));
        }
    }

    let now = Local::now().to_rfc3339();

    // Get the existing config to preserve created_at and is_applied
    // Use direct ID format like ClaudeCode does to avoid type::thing serialization issues
    // Only select the fields we need to avoid enum type issues
    let existing_result: Result<Vec<serde_json::Value>, _> = db
        .query(format!(
            "SELECT created_at, type::bool(is_applied) as is_applied FROM oh_my_opencode_config:`{}` LIMIT 1",
            config_id
        ))
        .await
        .map_err(|e| format!("Failed to query config: {}", e))?
        .take(0);

    // Extract fields from the query result
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

    let content = OhMyOpenCodeConfigContent {
        name: input.name,
        is_applied: is_applied_value,
        agents: input.agents,
        other_fields: input.other_fields,
        created_at,
        updated_at: now,
    };

    let json_data = adapter::to_db_value(&content);

    // Inline JSON into query to avoid SurrealDB parameter binding serialization issues
    // This is necessary because SurrealDB may have enum<bool> type issues with is_applied field
    let json_str = serde_json::to_string(&json_data)
        .map_err(|e| format!("Failed to serialize json_data: {}", e))?;
    
    db.query(format!("UPDATE oh_my_opencode_config:`{}` CONTENT {}", config_id, json_str))
        .await
        .map_err(|e| format!("Failed to update config: {}", e))?;

    // 如果该配置当前是应用状态，立即重新写入到配置文件
    if is_applied_value {
        if let Err(e) = apply_config_to_file(&db, &config_id).await {
            eprintln!("Failed to auto-apply updated config: {}", e);
            // 不中断更新流程，只记录错误
        } else {
            // Trigger WSL sync via event (Windows only)
            #[cfg(target_os = "windows")]
            let _ = app.emit("wsl-sync-request-opencode", ());
        }
    }

    // Return the config we just wrote - no need to query it back
    // This avoids any potential enum serialization issues from SurrealDB
    Ok(OhMyOpenCodeConfig {
        id: config_id,
        name: content.name,
        is_applied: is_applied_value,
        agents: content.agents,
        other_fields: content.other_fields,
        created_at: Some(content.created_at),
        updated_at: Some(content.updated_at),
    })
}

/// Delete an oh-my-opencode config
#[tauri::command]
pub async fn delete_oh_my_opencode_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let db = state.0.lock().await;

    db.query(format!("DELETE oh_my_opencode_config:`{}`", id))
        .await
        .map_err(|e| format!("Failed to delete config: {}", e))?;

    // Notify to refresh tray menu
    let _ = app.emit("config-changed", "window");

    Ok(())
}

/// 内部函数：将指定配置应用到配置文件（不改变数据库中的 is_applied 状态）
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
    // Get the config from database using direct ID format (like ClaudeCode)
    let records_result: Result<Vec<Value>, _> = db
        .query(format!(
            "SELECT *, type::string(id) as id FROM oh_my_opencode_config:`{}` LIMIT 1",
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

    // Get config path using unified function
    let config_path = get_oh_my_opencode_config_path()?;

    // Ensure parent directory exists
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create opencode config directory: {}", e))?;
        }
    }

    // 获取 Global Config
    let global_records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM oh_my_opencode_global_config:`global` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query global config: {}", e))?
        .take(0);

    let global_config = match global_records_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                adapter::global_config_from_db_value(record.clone())
            } else {
                // 使用默认空配置
                OhMyOpenCodeGlobalConfig {
                    id: "global".to_string(),
                    schema: None,
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
            // 使用默认空配置
            OhMyOpenCodeGlobalConfig {
                id: "global".to_string(),
                schema: None,
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

    // 合并配置的优先级顺序（从低到高）：
    // 1. 全局配置的明确字段（最低优先级）
    // 2. 全局配置的 other_fields
    // 3. Agents Profile 的 agents
    // 4. Agents Profile 的 other_fields（最高优先级，可以覆盖所有）

    let mut final_json = serde_json::Map::new();

    // 使用保存的 schema 或默认 schema
    let schema_url = global_config.schema
        .unwrap_or_else(|| "https://raw.githubusercontent.com/code-yeongyu/oh-my-opencode/master/assets/oh-my-opencode.schema.json".to_string());
    final_json.insert("$schema".to_string(), serde_json::json!(schema_url));

    // 1. 先设置全局配置的明确字段（优先级最低）
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

    // 2. 然后平铺全局配置的 other_fields（会覆盖上面的明确字段）
    if let Some(global_others) = global_config.other_fields {
        if let Some(others_obj) = global_others.as_object() {
            for (key, value) in others_obj {
                final_json.insert(key.clone(), value.clone());
            }
        }
    }

    // 3. 设置 Agents Profile 的 agents（会覆盖前面的 agents）
    if let Some(agents) = agents_profile.agents {
        final_json.insert("agents".to_string(), agents);
    }

    // 4. 最后平铺 Agents Profile 的 other_fields（最高优先级，可以覆盖所有字段）
    if let Some(profile_others) = agents_profile.other_fields {
        if let Some(others_obj) = profile_others.as_object() {
            for (key, value) in others_obj {
                final_json.insert(key.clone(), value.clone());
            }
        }
    }

    let mut final_json = Value::Object(final_json);

    // 清理空值：删除空对象和空数组
    adapter::clean_empty_values(&mut final_json);

    // Write to file with pretty formatting
    let json_content = serde_json::to_string_pretty(&final_json)
        .map_err(|e| format!("Failed to serialize final config: {}", e))?;

    fs::write(&config_path, json_content)
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    Ok(())
}
/// Apply an oh-my-opencode config to the JSON file
#[tauri::command]
pub async fn apply_oh_my_opencode_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    config_id: String,
) -> Result<(), String> {
    let db = state.0.lock().await;
    apply_config_internal(&db, &app, &config_id, false).await?;
    Ok(())
}

/// Internal function to apply config: writes to file and updates database
/// This is the single source of truth for applying an Oh My OpenCode config
pub async fn apply_config_internal<R: tauri::Runtime>(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    app: &tauri::AppHandle<R>,
    config_id: &str,
    from_tray: bool,
) -> Result<(), String> {
    // 应用配置到文件
    apply_config_to_file(db, config_id).await?;

    // Update database - set all configs to not applied, then set this one to applied
    let now = Local::now().to_rfc3339();

    // Clear applied flag (only update the currently applied one)
    db.query("UPDATE oh_my_opencode_config SET is_applied = false, updated_at = $now WHERE is_applied = true")
        .bind(("now", now.clone()))
        .await
        .map_err(|e| format!("Failed to clear applied flags: {}", e))?;

    // Set this config as applied using WHERE clause with type::thing (like ClaudeCode)
    db.query("UPDATE oh_my_opencode_config SET is_applied = true, updated_at = $now WHERE id = type::thing('oh_my_opencode_config', $id)")
        .bind(("id", config_id.to_string()))
        .bind(("now", now))
        .await
        .map_err(|e| format!("Failed to update applied flag: {}", e))?;

    // Notify based on source
    let payload = if from_tray { "tray" } else { "window" };
    let _ = app.emit("config-changed", payload);

    // Trigger WSL sync via event (Windows only)
    #[cfg(target_os = "windows")]
    let _ = app.emit("wsl-sync-request-opencode", ());

    Ok(())
}

/// Reorder oh-my-opencode configs (by name for now)
#[tauri::command]
pub async fn reorder_oh_my_opencode_configs(
    state: tauri::State<'_, DbState>,
    ids: Vec<String>,
) -> Result<(), String> {
    let db = state.0.lock().await;

    for (index, id) in ids.iter().enumerate() {
        db.query(format!(
            "UPDATE oh_my_opencode_config:`{}` SET sort_index = $index",
            id
        ))
        .bind(("index", index as i32))
        .await
        .map_err(|e| format!("Failed to update sort index: {}", e))?;
    }

    Ok(())
}

/// Get oh-my-opencode config file path info
#[tauri::command]
pub async fn get_oh_my_opencode_config_path_info() -> Result<ConfigPathInfo, String> {
    let config_path = get_oh_my_opencode_config_path()?;
    let path = config_path.to_string_lossy().to_string();

    Ok(ConfigPathInfo {
        path,
        source: "default".to_string(),
    })
}

/// Check if local oh-my-opencode config file exists
/// Returns true if ~/.config/opencode/oh-my-opencode.jsonc or .json exists
#[tauri::command]
pub async fn check_oh_my_opencode_config_exists() -> Result<bool, String> {
    let home_dir = dirs::home_dir()
        .ok_or("Failed to get home directory")?;

    let opencode_dir = home_dir.join(".config").join("opencode");
    let jsonc_path = opencode_dir.join("oh-my-opencode.jsonc");
    let json_path = opencode_dir.join("oh-my-opencode.json");

    Ok(jsonc_path.exists() || json_path.exists())
}

// ============================================================================
// Oh My OpenCode Global Config Commands
// ============================================================================

/// Get oh-my-opencode global config (固定 ID 为 "global")
#[tauri::command]
pub async fn get_oh_my_opencode_global_config(
    state: tauri::State<'_, DbState>,
) -> Result<OhMyOpenCodeGlobalConfig, String> {
    let db = state.0.lock().await;

    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM oh_my_opencode_global_config:`global` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query global config: {}", e))?
        .take(0);

    match records_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                Ok(adapter::global_config_from_db_value(record.clone()))
            } else {
                // 数据库为空，尝试从本地文件导入全局配置
                if let Ok(imported_config) = import_local_global_config_if_exists(&db).await {
                    return Ok(imported_config);
                }

                // 使用默认配置并保存到数据库
                let default_config = OhMyOpenCodeGlobalConfig {
                    id: "global".to_string(),
                    schema: None,
                    sisyphus_agent: None,
                    disabled_agents: None,
                    disabled_mcps: None,
                    disabled_hooks: None,
                    lsp: None,
                    experimental: None,
                    other_fields: None,
                    updated_at: None,
                };

                // 保存默认配置到数据库
                let now = Local::now().to_rfc3339();
                let content = OhMyOpenCodeGlobalConfigContent {
                    schema: None,
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
                if db.query("UPSERT oh_my_opencode_global_config:`global` CONTENT $data")
                    .bind(("data", json_data))
                    .await
                    .is_ok() {
                }

                Ok(default_config)
            }
        }
        Err(e) => {
            eprintln!("Failed to get global config: {}", e);
            // 返回默认配置
            Ok(OhMyOpenCodeGlobalConfig {
                id: "global".to_string(),
                schema: None,
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

/// 从本地配置文件导入全局配置（如果存在）
async fn import_local_global_config_if_exists(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<OhMyOpenCodeGlobalConfig, String> {
    let config_path = get_oh_my_opencode_config_path()
        .map_err(|_| "Local config file not found".to_string())?;

    // 读取文件内容
    let file_content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read local config file: {}", e))?;

    // 解析 JSON（使用 json5 支持带注释的 JSONC 格式）
    let json_value: Value = json5::from_str(&file_content)
        .map_err(|e| format!("Failed to parse local config file: {}", e))?;
    
    // 提取全局配置字段
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

    // 提取 schema
    let schema = json_value
        .get("$schema")
        .and_then(|v| v.as_str())
        .map(String::from);

    // 提取 other_fields（除了已知字段之外的所有字段）
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

    // 创建全局配置内容（不包含 config_id）
    let content = OhMyOpenCodeGlobalConfigContent {
        schema,
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

    // Use UPSERT to handle both update and create
    db.query("UPSERT oh_my_opencode_global_config:`global` CONTENT $data")
        .bind(("data", json_data))
        .await
        .map_err(|e| format!("Failed to import global config: {}", e))?;

    // 从数据库读取刚导入的配置
    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM oh_my_opencode_global_config:`global` LIMIT 1")
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

/// Save oh-my-opencode global config
#[tauri::command]
pub async fn save_oh_my_opencode_global_config(
    state: tauri::State<'_, DbState>,
    _app: tauri::AppHandle,
    input: OhMyOpenCodeGlobalConfigInput,
) -> Result<OhMyOpenCodeGlobalConfig, String> {
    let db = state.0.lock().await;

    let now = Local::now().to_rfc3339();
    let content = OhMyOpenCodeGlobalConfigContent {
        schema: input.schema,
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

    // Use UPSERT to handle both update and create
    db.query("UPSERT oh_my_opencode_global_config:`global` CONTENT $data")
        .bind(("data", json_data))
        .await
        .map_err(|e| format!("Failed to save global config: {}", e))?;

    // 查找当前应用的配置，如果存在则重新应用到文件
    let applied_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM oh_my_opencode_config WHERE is_applied = true LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query applied config: {}", e))?
        .take(0);

    if let Ok(records) = applied_result {
        if let Some(record) = records.first() {
            let applied_config = adapter::from_db_value(record.clone());
            // 重新应用配置到文件（不改变数据库中的 is_applied 状态）
            if apply_config_to_file(&db, &applied_config.id).await.is_ok() {
                // Trigger WSL sync via event (Windows only)
                #[cfg(target_os = "windows")]
                let _ = app.emit("wsl-sync-request-opencode", ());
            }
        }
    }

    // 从数据库读取刚保存的配置
    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM oh_my_opencode_global_config:`global` LIMIT 1")
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
