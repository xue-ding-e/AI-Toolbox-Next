use chrono::Local;
use std::fs;
use std::path::Path;
use serde_json::Value;

use crate::db::DbState;
use super::adapter;
use super::types::*;
use tauri::Emitter;

// ============================================================================
// Codex Config Path Commands
// ============================================================================

/// Get Codex config directory path (~/.codex/)
fn get_codex_config_dir() -> Result<std::path::PathBuf, String> {
    let home_dir = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| "Failed to get home directory".to_string())?;
    
    Ok(Path::new(&home_dir).join(".codex"))
}

/// Get Codex auth.json path
fn get_codex_auth_path() -> Result<std::path::PathBuf, String> {
    Ok(get_codex_config_dir()?.join("auth.json"))
}

/// Get Codex config.toml path
fn get_codex_config_path() -> Result<std::path::PathBuf, String> {
    Ok(get_codex_config_dir()?.join("config.toml"))
}

/// Get Codex config directory path
#[tauri::command]
pub fn get_codex_config_dir_path() -> Result<String, String> {
    let config_dir = get_codex_config_dir()?;
    Ok(config_dir.to_string_lossy().to_string())
}

/// Reveal Codex config folder in file explorer
#[tauri::command]
pub fn reveal_codex_config_folder() -> Result<(), String> {
    let config_dir = get_codex_config_dir()?;

    // Ensure directory exists
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create .codex directory: {}", e))?;
    }

    // Open in file explorer (platform-specific)
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&config_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&config_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&config_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    Ok(())
}

// ============================================================================
// Codex Provider Commands
// ============================================================================

/// List all Codex providers ordered by sort_index
#[tauri::command]
pub async fn list_codex_providers(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<CodexProvider>, String> {
    let db = state.0.lock().await;

    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT * OMIT id FROM codex_provider")
        .await
        .map_err(|e| format!("Failed to query providers: {}", e))?
        .take(0);

    match records_result {
        Ok(records) => {
            let mut result: Vec<CodexProvider> = records
                .into_iter()
                .map(adapter::from_db_value_provider)
                .collect();
            result.sort_by_key(|p| p.sort_index.unwrap_or(0));
            Ok(result)
        }
        Err(e) => {
            eprintln!("Failed to deserialize providers: {}", e);
            Ok(Vec::new())
        }
    }
}

/// Create a new Codex provider
#[tauri::command]
pub async fn create_codex_provider(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    provider: CodexProviderInput,
) -> Result<CodexProvider, String> {
    let db = state.0.lock().await;

    // Check if ID already exists
    let provider_id = provider.id.clone();
    let check_result: Result<Vec<Value>, _> = db
        .query("SELECT * OMIT id FROM codex_provider WHERE provider_id = $id LIMIT 1")
        .bind(("id", provider_id.clone()))
        .await
        .map_err(|e| format!("Failed to check provider existence: {}", e))?
        .take(0);

    if let Ok(records) = check_result {
        if !records.is_empty() {
            return Err(format!("Codex provider with ID '{}' already exists", provider.id));
        }
    }

    let now = Local::now().to_rfc3339();
    let content = CodexProviderContent {
        provider_id: provider.id.clone(),
        name: provider.name,
        category: provider.category,
        settings_config: provider.settings_config,
        source_provider_id: provider.source_provider_id,
        website_url: provider.website_url,
        notes: provider.notes,
        icon: provider.icon,
        icon_color: provider.icon_color,
        sort_index: provider.sort_index,
        is_applied: false,
        created_at: now.clone(),
        updated_at: now,
    };

    let json_data = adapter::to_db_value_provider(&content);

    db.query(format!("CREATE codex_provider:`{}` CONTENT $data", provider.id))
        .bind(("data", json_data))
        .await
        .map_err(|e| format!("Failed to create provider: {}", e))?;

    // Notify to refresh tray menu
    let _ = app.emit("config-changed", "window");

    Ok(CodexProvider {
        id: content.provider_id,
        name: content.name,
        category: content.category,
        settings_config: content.settings_config,
        source_provider_id: content.source_provider_id,
        website_url: content.website_url,
        notes: content.notes,
        icon: content.icon,
        icon_color: content.icon_color,
        sort_index: content.sort_index,
        is_applied: content.is_applied,
        created_at: content.created_at,
        updated_at: content.updated_at,
    })
}

/// Update an existing Codex provider
#[tauri::command]
pub async fn update_codex_provider(
    state: tauri::State<'_, DbState>,
    provider: CodexProvider,
) -> Result<CodexProvider, String> {
    let db = state.0.lock().await;

    // Get existing record to preserve created_at
    let provider_id = provider.id.clone();
    let existing_result: Result<Vec<Value>, _> = db
        .query("SELECT * OMIT id FROM codex_provider WHERE provider_id = $id LIMIT 1")
        .bind(("id", provider_id.clone()))
        .await
        .map_err(|e| format!("Failed to query existing provider: {}", e))?
        .take(0);

    let now = Local::now().to_rfc3339();
    let created_at = if !provider.created_at.is_empty() {
        provider.created_at
    } else if let Ok(records) = existing_result {
        if let Some(record) = records.first() {
            record.get("created_at").and_then(|v| v.as_str()).unwrap_or(&now).to_string()
        } else {
            return Err("Provider not found".to_string());
        }
    } else {
        return Err("Provider not found".to_string());
    };

    let content = CodexProviderContent {
        provider_id: provider.id.clone(),
        name: provider.name,
        category: provider.category,
        settings_config: provider.settings_config,
        source_provider_id: provider.source_provider_id,
        website_url: provider.website_url,
        notes: provider.notes,
        icon: provider.icon,
        icon_color: provider.icon_color,
        sort_index: provider.sort_index,
        is_applied: provider.is_applied,
        created_at,
        updated_at: now,
    };

    let json_data = adapter::to_db_value_provider(&content);

    db.query(format!("DELETE codex_provider:`{}`", provider.id))
        .await
        .map_err(|e| format!("Failed to delete old provider: {}", e))?;

    db.query(format!("CREATE codex_provider:`{}` CONTENT $data", provider.id))
        .bind(("data", json_data))
        .await
        .map_err(|e| format!("Failed to create updated provider: {}", e))?;

    // If this provider is applied, re-apply to config file
    if content.is_applied {
        if let Err(e) = apply_config_to_file(&db, &provider.id).await {
            eprintln!("Failed to auto-apply updated config: {}", e);
        }
    }

    Ok(CodexProvider {
        id: content.provider_id,
        name: content.name,
        category: content.category,
        settings_config: content.settings_config,
        source_provider_id: content.source_provider_id,
        website_url: content.website_url,
        notes: content.notes,
        icon: content.icon,
        icon_color: content.icon_color,
        sort_index: content.sort_index,
        is_applied: content.is_applied,
        created_at: content.created_at,
        updated_at: content.updated_at,
    })
}

/// Delete a Codex provider
#[tauri::command]
pub async fn delete_codex_provider(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let db = state.0.lock().await;

    db.query(format!("DELETE codex_provider:`{}`", id))
        .await
        .map_err(|e| format!("Failed to delete codex provider: {}", e))?;

    let _ = app.emit("config-changed", "window");
    Ok(())
}

/// Reorder Codex providers
#[tauri::command]
pub async fn reorder_codex_providers(
    state: tauri::State<'_, DbState>,
    ids: Vec<String>,
) -> Result<(), String> {
    let db = state.0.lock().await;
    let now = Local::now().to_rfc3339();

    for (index, id) in ids.iter().enumerate() {
        db.query("UPDATE codex_provider SET sort_index = $index, updated_at = $now WHERE provider_id = $id")
            .bind(("index", index as i32))
            .bind(("now", now.clone()))
            .bind(("id", id.clone()))
            .await
            .map_err(|e| format!("Failed to update provider {}: {}", id, e))?;
    }

    Ok(())
}

/// Select a Codex provider (mark as applied in database)
#[tauri::command]
pub async fn select_codex_provider(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let db = state.0.lock().await;
    let now = Local::now().to_rfc3339();

    // Mark all providers as not applied
    db.query("UPDATE codex_provider SET is_applied = false, updated_at = $now")
        .bind(("now", now.clone()))
        .await
        .map_err(|e| format!("Failed to reset applied status: {}", e))?;

    // Mark target provider as applied
    db.query("UPDATE codex_provider SET is_applied = true, updated_at = $now WHERE provider_id = $id")
        .bind(("id", id))
        .bind(("now", now))
        .await
        .map_err(|e| format!("Failed to set applied status: {}", e))?;

    let _ = app.emit("config-changed", "window");
    Ok(())
}

// ============================================================================
// Codex Config File Commands
// ============================================================================

/// Internal function: apply provider config to files
async fn apply_config_to_file(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    provider_id: &str,
) -> Result<(), String> {
    apply_config_to_file_public(db, provider_id).await
}

/// Public version for tray module
pub async fn apply_config_to_file_public(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    provider_id: &str,
) -> Result<(), String> {
    // Get the provider
    let provider_result: Result<Vec<Value>, _> = db
        .query("SELECT * OMIT id FROM codex_provider WHERE provider_id = $id LIMIT 1")
        .bind(("id", provider_id.to_string()))
        .await
        .map_err(|e| format!("Failed to query provider: {}", e))?
        .take(0);

    let provider = match provider_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                adapter::from_db_value_provider(record.clone())
            } else {
                return Err("Provider not found".to_string());
            }
        }
        Err(e) => return Err(format!("Failed to deserialize provider: {}", e)),
    };

    // Parse provider settings_config
    let provider_config: serde_json::Value = serde_json::from_str(&provider.settings_config)
        .map_err(|e| format!("Failed to parse provider config: {}", e))?;

    // Get common config
    let common_config_result: Result<Vec<Value>, _> = db
        .query("SELECT * OMIT id FROM codex_common_config:`common` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query common config: {}", e))?
        .take(0);

    let common_toml: Option<String> = match common_config_result {
        Ok(records) => records.first().and_then(|r| {
            r.get("config").and_then(|v| v.as_str()).map(|s| s.to_string())
        }),
        Err(_) => None,
    };

    // Extract auth and config
    let auth = provider_config.get("auth").cloned().unwrap_or(serde_json::json!({}));
    let mut config_toml = provider_config
        .get("config")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Merge common config
    if let Some(common) = common_toml {
        if !common.trim().is_empty() {
            config_toml = merge_toml_configs(&common, &config_toml)?;
        }
    }

    write_codex_config_files(&auth, &config_toml)?;
    Ok(())
}

/// Merge two TOML configs (common + provider, provider takes precedence)
fn merge_toml_configs(common: &str, provider: &str) -> Result<String, String> {
    if common.trim().is_empty() {
        return Ok(provider.to_string());
    }
    if provider.trim().is_empty() {
        return Ok(common.to_string());
    }

    let common_table: toml::Table = toml::from_str(common)
        .map_err(|e| format!("Failed to parse common TOML: {}", e))?;
    let provider_table: toml::Table = toml::from_str(provider)
        .map_err(|e| format!("Failed to parse provider TOML: {}", e))?;

    let mut merged = common_table;
    for (key, value) in provider_table {
        merged.insert(key, value);
    }

    toml::to_string_pretty(&merged)
        .map_err(|e| format!("Failed to serialize merged TOML: {}", e))
}

/// Write auth.json and config.toml files
fn write_codex_config_files(auth: &serde_json::Value, config_toml: &str) -> Result<(), String> {
    let config_dir = get_codex_config_dir()?;
    
    // Ensure directory exists
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create .codex directory: {}", e))?;
    }

    // Write auth.json
    let auth_path = config_dir.join("auth.json");
    let auth_content = serde_json::to_string_pretty(auth)
        .map_err(|e| format!("Failed to serialize auth: {}", e))?;
    fs::write(&auth_path, auth_content)
        .map_err(|e| format!("Failed to write auth.json: {}", e))?;

    // Write config.toml
    let config_path = config_dir.join("config.toml");
    fs::write(&config_path, config_toml)
        .map_err(|e| format!("Failed to write config.toml: {}", e))?;

    Ok(())
}

/// Apply Codex config to files
#[tauri::command]
pub async fn apply_codex_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    provider_id: String,
) -> Result<(), String> {
    let db = state.0.lock().await;
    apply_config_internal(&db, &app, &provider_id, false).await
}

/// Internal function to apply config
pub async fn apply_config_internal<R: tauri::Runtime>(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    app: &tauri::AppHandle<R>,
    provider_id: &str,
    from_tray: bool,
) -> Result<(), String> {
    // Apply config to files
    apply_config_to_file(db, provider_id).await?;

    // Update is_applied status
    let now = Local::now().to_rfc3339();

    db.query("UPDATE codex_provider SET is_applied = false, updated_at = $now")
        .bind(("now", now.clone()))
        .await
        .map_err(|e| format!("Failed to reset applied status: {}", e))?;

    db.query("UPDATE codex_provider SET is_applied = true, updated_at = $now WHERE provider_id = $id")
        .bind(("id", provider_id.to_string()))
        .bind(("now", now))
        .await
        .map_err(|e| format!("Failed to set applied status: {}", e))?;

    let payload = if from_tray { "tray" } else { "window" };
    let _ = app.emit("config-changed", payload);

    Ok(())
}

/// Read current Codex settings from files
#[tauri::command]
pub async fn read_codex_settings() -> Result<CodexSettings, String> {
    let auth_path = get_codex_auth_path()?;
    let config_path = get_codex_config_path()?;

    let auth = if auth_path.exists() {
        let content = fs::read_to_string(&auth_path)
            .map_err(|e| format!("Failed to read auth.json: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse auth.json: {}", e))?
    } else {
        None
    };

    let config = if config_path.exists() {
        Some(fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config.toml: {}", e))?)
    } else {
        None
    };

    Ok(CodexSettings { auth, config })
}

// ============================================================================
// Codex Common Config Commands
// ============================================================================

/// Get Codex common config
#[tauri::command]
pub async fn get_codex_common_config(
    state: tauri::State<'_, DbState>,
) -> Result<Option<CodexCommonConfig>, String> {
    let db = state.0.lock().await;

    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT * OMIT id FROM codex_common_config:`common` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query common config: {}", e))?
        .take(0);

    match records_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                Ok(Some(adapter::from_db_value_common(record.clone())))
            } else {
                Ok(None)
            }
        }
        Err(e) => {
            eprintln!("Failed to deserialize common config: {}", e);
            Ok(None)
        }
    }
}

/// Save Codex common config
#[tauri::command]
pub async fn save_codex_common_config(
    state: tauri::State<'_, DbState>,
    config: String,
) -> Result<(), String> {
    let db = state.0.lock().await;

    // Validate TOML if not empty
    if !config.trim().is_empty() {
        let _: toml::Table = toml::from_str(&config)
            .map_err(|e| format!("Invalid TOML: {}", e))?;
    }

    let json_data = adapter::to_db_value_common(&config);

    db.query("DELETE codex_common_config:`common`")
        .await
        .map_err(|e| format!("Failed to delete old config: {}", e))?;

    db.query("CREATE codex_common_config:`common` CONTENT $data")
        .bind(("data", json_data))
        .await
        .map_err(|e| format!("Failed to create config: {}", e))?;

    // Re-apply current provider if exists
    let applied_result: Result<Vec<Value>, _> = db
        .query("SELECT * OMIT id FROM codex_provider WHERE is_applied = true LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query applied provider: {}", e))?
        .take(0);

    if let Ok(records) = applied_result {
        if let Some(record) = records.first() {
            let provider = adapter::from_db_value_provider(record.clone());
            if let Err(e) = apply_config_to_file(&db, &provider.id).await {
                eprintln!("Failed to re-apply config: {}", e);
            }
        }
    }

    Ok(())
}

// ============================================================================
// Codex Initialization
// ============================================================================

/// Initialize Codex provider from existing config files
pub async fn init_codex_provider_from_settings(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<(), String> {
    // Check if any providers exist
    let count_result: Result<Vec<Value>, _> = db
        .query("SELECT count() FROM codex_provider GROUP ALL")
        .await
        .map_err(|e| format!("Failed to count providers: {}", e))?
        .take(0);

    let has_providers = match count_result {
        Ok(records) => records.first()
            .and_then(|r| r.get("count"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0) > 0,
        Err(_) => false,
    };

    if has_providers {
        return Ok(());
    }

    // Check if config files exist
    let auth_path = get_codex_auth_path()?;
    if !auth_path.exists() {
        return Ok(());
    }

    // Read auth.json
    let auth_content = fs::read_to_string(&auth_path)
        .map_err(|e| format!("Failed to read auth.json: {}", e))?;
    let auth: serde_json::Value = serde_json::from_str(&auth_content)
        .map_err(|e| format!("Failed to parse auth.json: {}", e))?;

    // Read config.toml
    let config_path = get_codex_config_path()?;
    let config_toml = if config_path.exists() {
        fs::read_to_string(&config_path).unwrap_or_default()
    } else {
        String::new()
    };

    // Build settings_config
    let settings = serde_json::json!({
        "auth": auth,
        "config": config_toml
    });

    let now = Local::now().to_rfc3339();
    let content = CodexProviderContent {
        provider_id: "default-config".to_string(),
        name: "默认配置".to_string(),
        category: String::new(),
        settings_config: serde_json::to_string(&settings).unwrap_or_default(),
        source_provider_id: None,
        website_url: None,
        notes: Some("从配置文件自动导入".to_string()),
        icon: None,
        icon_color: None,
        sort_index: Some(0),
        is_applied: true,
        created_at: now.clone(),
        updated_at: now,
    };

    let json_data = adapter::to_db_value_provider(&content);
    db.query("CREATE codex_provider:`default-config` CONTENT $data")
        .bind(("data", json_data))
        .await
        .map_err(|e| format!("Failed to create provider: {}", e))?;

    println!("✅ Imported Codex settings as default provider");
    Ok(())
}
