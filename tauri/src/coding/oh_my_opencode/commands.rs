use chrono::Local;
use std::fs;
use std::path::Path;
use serde_json::Value;

use crate::db::DbState;
use super::adapter;
use super::types::*;

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
        .query("SELECT * FROM oh_my_opencode_config")
        .await
        .map_err(|e| format!("Failed to query configs: {}", e))?
        .take(0);

    match records_result {
        Ok(records) => {
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

/// Create a new oh-my-opencode config
#[tauri::command]
pub async fn create_oh_my_opencode_config(
    state: tauri::State<'_, DbState>,
    input: OhMyOpenCodeConfigInput,
) -> Result<OhMyOpenCodeConfig, String> {
    let db = state.0.lock().await;

    // Check if ID already exists
    let config_id = input.id.clone();
    let check_result: Result<Vec<Value>, _> = db
        .query("SELECT * FROM oh_my_opencode_config WHERE config_id = $id OR configId = $id LIMIT 1")
        .bind(("id", config_id.clone()))
        .await
        .map_err(|e| format!("Failed to check config existence: {}", e))?
        .take(0);

    if let Ok(records) = check_result {
        if !records.is_empty() {
            return Err(format!(
                "Oh-my-opencode config with ID '{}' already exists",
                input.id
            ));
        }
    }

    let now = Local::now().to_rfc3339();
    let content = OhMyOpenCodeConfigContent {
        config_id: input.id.clone(),
        name: input.name,
        is_applied: false,
        schema: Some("https://raw.githubusercontent.com/code-yeongyu/oh-my-opencode/master/assets/oh-my-opencode.schema.json".to_string()),
        agents: input.agents,
        sisyphus_agent: input.sisyphus_agent,
        disabled_agents: input.disabled_agents,
        disabled_mcps: input.disabled_mcps,
        disabled_hooks: input.disabled_hooks,
        disabled_skills: input.disabled_skills,
        disabled_commands: input.disabled_commands,
        created_at: now.clone(),
        updated_at: now,
    };

    let json_data = adapter::to_db_value(&content);

    db.query(format!(
        "CREATE oh_my_opencode_config:`{}` CONTENT $data",
        input.id
    ))
    .bind(("data", json_data))
    .await
    .map_err(|e| format!("Failed to create config: {}", e))?;

    Ok(OhMyOpenCodeConfig {
        id: content.config_id,
        name: content.name,
        is_applied: content.is_applied,
        schema: content.schema,
        agents: content.agents,
        sisyphus_agent: content.sisyphus_agent,
        disabled_agents: content.disabled_agents,
        disabled_mcps: content.disabled_mcps,
        disabled_hooks: content.disabled_hooks,
        disabled_skills: content.disabled_skills,
        disabled_commands: content.disabled_commands,
        created_at: Some(content.created_at),
        updated_at: Some(content.updated_at),
    })
}

/// Update an existing oh-my-opencode config
#[tauri::command]
pub async fn update_oh_my_opencode_config(
    state: tauri::State<'_, DbState>,
    input: OhMyOpenCodeConfigInput,
) -> Result<OhMyOpenCodeConfig, String> {
    let db = state.0.lock().await;

    // Check if config exists
    let check_result: Result<Vec<Value>, _> = db
        .query("SELECT * FROM oh_my_opencode_config WHERE config_id = $id OR configId = $id LIMIT 1")
        .bind(("id", input.id.clone()))
        .await
        .map_err(|e| format!("Failed to check config existence: {}", e))?
        .take(0);

    if let Ok(records) = check_result {
        if records.is_empty() {
            return Err(format!(
                "Oh-my-opencode config with ID '{}' not found",
                input.id
            ));
        }
    }

    let now = Local::now().to_rfc3339();
    
    // Get the existing config to preserve created_at
    let existing_content: Option<OhMyOpenCodeConfigContent> = db
        .query("SELECT * FROM oh_my_opencode_config WHERE config_id = $id")
        .bind(("id", input.id.clone()))
        .await
        .map_err(|e| format!("Failed to query config: {}", e))?
        .take(1)
        .ok()
        .and_then(|records| records.first())
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let created_at = existing_content.as_ref()
        .and_then(|c| c.created_at.clone())
        .unwrap_or_else(|| Local::now().to_rfc3339());

    let content = OhMyOpenCodeConfigContent {
        config_id: input.id.clone(),
        name: input.name,
        is_applied: existing_content.as_ref()
            .map(|c| c.is_applied)
            .unwrap_or(false),
        schema: Some("https://raw.githubusercontent.com/code-yeongyu/oh-my-opencode/master/assets/oh-my-opencode.schema.json".to_string()),
        agents: input.agents,
        sisyphus_agent: input.sisyphus_agent,
        disabled_agents: input.disabled_agents,
        disabled_mcps: input.disabled_mcps,
        disabled_hooks: input.disabled_hooks,
        disabled_skills: input.disabled_skills,
        disabled_commands: input.disabled_commands,
        created_at,
        updated_at: now,
    };

    let json_data = adapter::to_db_value(&content);

    db.query(format!(
        "UPDATE oh_my_opencode_config:`{}` CONTENT $data",
        input.id
    ))
    .bind(("data", json_data))
    .await
    .map_err(|e| format!("Failed to update config: {}", e))?;

    Ok(OhMyOpenCodeConfig {
        id: content.config_id,
        name: content.name,
        is_applied: content.is_applied,
        schema: content.schema,
        agents: content.agents,
        sisyphus_agent: content.sisyphus_agent,
        disabled_agents: content.disabled_agents,
        disabled_mcps: content.disabled_mcps,
        disabled_hooks: content.disabled_hooks,
        disabled_skills: content.disabled_skills,
        disabled_commands: content.disabled_commands,
        created_at: Some(content.created_at),
        updated_at: Some(content.updated_at),
    })
}

/// Delete an oh-my-opencode config
#[tauri::command]
pub async fn delete_oh_my_opencode_config(
    state: tauri::State<'_, DbState>,
    id: String,
) -> Result<(), String> {
    let db = state.0.lock().await;

    db.query(format!("DELETE oh_my_opencode_config:`{}`", id))
        .await
        .map_err(|e| format!("Failed to delete config: {}", e))?;

    Ok(())
}

/// Apply an oh-my-opencode config to the JSON file
#[tauri::command]
pub async fn apply_oh_my_opencode_config(
    state: tauri::State<'_, DbState>,
    config_id: String,
) -> Result<(), String> {
    let db = state.0.lock().await;

    // Get the config from database
    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT * FROM oh_my_opencode_config WHERE config_id = $id OR configId = $id LIMIT 1")
        .bind(("id", config_id.clone()))
        .await
        .map_err(|e| format!("Failed to query config: {}", e))?
        .take(0);

    let config = match records_result {
        Ok(records) => {
            if records.is_empty() {
                return Err(format!("Config '{}' not found", config_id));
            }
            adapter::from_db_value(records[0].clone())
        }
        Err(e) => return Err(format!("Failed to get config: {}", e)),
    };

    // Get home directory and opencode config path
    let home_dir = dirs::home_dir()
        .ok_or("Failed to get home directory")?;
    
    let opencode_dir = home_dir.join(".config").join("opencode");
    if !opencode_dir.exists() {
        fs::create_dir_all(&opencode_dir)
            .map_err(|e| format!("Failed to create opencode config directory: {}", e))?;
    }

    let config_path = opencode_dir.join("oh-my-opencode.json");

    // Build JSON content
    let mut json_config = OhMyOpenCodeJsonConfig::default();
    json_config.schema = config.schema;
    json_config.agents = Some(config.agents);
    json_config.sisyphus_agent = config.sisyphus_agent;
    json_config.disabled_agents = config.disabled_agents;
    json_config.disabled_mcps = config.disabled_mcps;
    json_config.disabled_hooks = config.disabled_hooks;
    json_config.disabled_skills = config.disabled_skills;
    json_config.disabled_commands = config.disabled_commands;

    // Write to file with pretty formatting
    let json_content = serde_json::to_string_pretty(&json_config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    
    fs::write(&config_path, json_content)
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    // Update database - set all configs to not applied, then set this one to applied
    let now = Local::now().to_rfc3339();
    
    // Clear all applied flags
    db.query("UPDATE oh_my_opencode_config SET is_applied = false")
        .await
        .map_err(|e| format!("Failed to clear applied flags: {}", e))?;

    // Set this config as applied
    db.query(format!(
        "UPDATE oh_my_opencode_config:`{}` SET is_applied = true, updated_at = $now",
        config_id
    ))
    .bind(("now", now))
    .await
    .map_err(|e| format!("Failed to update applied flag: {}", e))?;

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
    let home_dir = dirs::home_dir()
        .ok_or("Failed to get home directory")?;
    
    let opencode_dir = home_dir.join(".config").join("opencode");
    let json_path = opencode_dir.join("oh-my-opencode.json");
    let jsonc_path = opencode_dir.join("oh-my-opencode.jsonc");

    let (path, source) = if jsonc_path.exists() {
        (jsonc_path.to_string_lossy().to_string(), "default")
    } else if json_path.exists() {
        (json_path.to_string_lossy().to_string(), "default")
    } else {
        (json_path.to_string_lossy().to_string(), "default")
    };

    Ok(ConfigPathInfo {
        path,
        source,
    })
}
