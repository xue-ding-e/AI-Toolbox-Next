use chrono::Local;
use std::fs;
use std::path::Path;

use crate::db::DbState;
use super::types::*;

// ============================================================================
// Claude Code Provider Commands
// ============================================================================

/// List all Claude Code providers ordered by sort_index
#[tauri::command]
pub async fn list_claude_providers(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<ClaudeCodeProvider>, String> {
    let db = state.0.lock().await;

    let records: Vec<ClaudeCodeProviderRecord> = db
        .select("claude_provider")
        .await
        .map_err(|e| format!("Failed to list claude providers: {}", e))?;

    let mut result: Vec<ClaudeCodeProvider> =
        records.into_iter().map(ClaudeCodeProvider::from).collect();
    result.sort_by_key(|p| p.sort_index.unwrap_or(0));
    Ok(result)
}

/// Create a new Claude Code provider
#[tauri::command]
pub async fn create_claude_provider(
    state: tauri::State<'_, DbState>,
    provider: ClaudeCodeProviderInput,
) -> Result<ClaudeCodeProvider, String> {
    let db = state.0.lock().await;

    // Check if ID already exists
    let existing: Option<ClaudeCodeProviderRecord> = db
        .select(("claude_provider", &provider.id))
        .await
        .map_err(|e| format!("Failed to check provider existence: {}", e))?;

    if existing.is_some() {
        return Err(format!(
            "Claude provider with ID '{}' already exists",
            provider.id
        ));
    }

    let now = Local::now().to_rfc3339();
    let content = ClaudeCodeProviderContent {
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
        is_current: false,
        is_applied: false,
        created_at: now.clone(),
        updated_at: now,
    };

    let created: Option<ClaudeCodeProviderRecord> = db
        .create(("claude_provider", &provider.id))
        .content(content)
        .await
        .map_err(|e| format!("Failed to create claude provider: {}", e))?;

    created
        .map(ClaudeCodeProvider::from)
        .ok_or_else(|| "Failed to create claude provider".to_string())
}

/// Update an existing Claude Code provider
#[tauri::command]
pub async fn update_claude_provider(
    state: tauri::State<'_, DbState>,
    provider: ClaudeCodeProvider,
) -> Result<ClaudeCodeProvider, String> {
    let db = state.0.lock().await;

    // Get existing record to preserve created_at if not provided
    let existing: Option<ClaudeCodeProviderRecord> = db
        .select(("claude_provider", &provider.id))
        .await
        .map_err(|e| format!("Failed to get existing provider: {}", e))?;

    let now = Local::now().to_rfc3339();
    let created_at = if !provider.created_at.is_empty() {
        provider.created_at
    } else if let Some(ref existing_record) = existing {
        existing_record.created_at.clone()
    } else {
        now.clone()
    };

    let content = ClaudeCodeProviderContent {
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
        is_current: provider.is_current,
        is_applied: provider.is_applied,
        created_at,
        updated_at: now,
    };

    let updated: Option<ClaudeCodeProviderRecord> = db
        .update(("claude_provider", &provider.id))
        .content(content)
        .await
        .map_err(|e| format!("Failed to update claude provider: {}", e))?;

    updated
        .map(ClaudeCodeProvider::from)
        .ok_or_else(|| "Claude provider not found".to_string())
}

/// Delete a Claude Code provider
#[tauri::command]
pub async fn delete_claude_provider(
    state: tauri::State<'_, DbState>,
    id: String,
) -> Result<(), String> {
    let db = state.0.lock().await;

    let _: Option<ClaudeCodeProviderRecord> = db
        .delete(("claude_provider", &id))
        .await
        .map_err(|e| format!("Failed to delete claude provider: {}", e))?;

    Ok(())
}

/// Select a Claude Code provider as current (deselect others)
#[tauri::command]
pub async fn select_claude_provider(
    state: tauri::State<'_, DbState>,
    id: String,
) -> Result<(), String> {
    let db = state.0.lock().await;

    let records: Vec<ClaudeCodeProviderRecord> = db
        .select("claude_provider")
        .await
        .map_err(|e| format!("Failed to list providers: {}", e))?;

    for record in records {
        let is_selected = record.provider_id == id;

        let content = ClaudeCodeProviderContent {
            provider_id: record.provider_id.clone(),
            name: record.name,
            category: record.category,
            settings_config: record.settings_config,
            source_provider_id: record.source_provider_id,
            website_url: record.website_url,
            notes: record.notes,
            icon: record.icon,
            icon_color: record.icon_color,
            sort_index: record.sort_index,
            is_current: is_selected,
            is_applied: record.is_applied,
            created_at: record.created_at,
            updated_at: Local::now().to_rfc3339(),
        };

        let thing_id = record.provider_id.clone();
        let _: Option<ClaudeCodeProviderRecord> = db
            .update(("claude_provider", thing_id))
            .content(content)
            .await
            .map_err(|e| format!("Failed to update provider: {}", e))?;
    }

    Ok(())
}

/// Reorder Claude Code providers
#[tauri::command]
pub async fn reorder_claude_providers(
    state: tauri::State<'_, DbState>,
    ids: Vec<String>,
) -> Result<(), String> {
    let db = state.0.lock().await;

    for (index, id) in ids.iter().enumerate() {
        let record: Option<ClaudeCodeProviderRecord> = db
            .select(("claude_provider", id))
            .await
            .map_err(|e| format!("Failed to get provider: {}", e))?;

        if let Some(r) = record {
            let content = ClaudeCodeProviderContent {
                provider_id: r.provider_id,
                name: r.name,
                category: r.category,
                settings_config: r.settings_config,
                source_provider_id: r.source_provider_id,
                website_url: r.website_url,
                notes: r.notes,
                icon: r.icon,
                icon_color: r.icon_color,
                sort_index: Some(index as i32),
                is_current: r.is_current,
                is_applied: r.is_applied,
                created_at: r.created_at,
                updated_at: Local::now().to_rfc3339(),
            };

            let _: Option<ClaudeCodeProviderRecord> = db
                .update(("claude_provider", id))
                .content(content)
                .await
                .map_err(|e| format!("Failed to update provider order: {}", e))?;
        }
    }

    Ok(())
}

// ============================================================================
// Claude Config File Commands
// ============================================================================

/// Get Claude config file path (~/.claude/settings.json)
#[tauri::command]
pub fn get_claude_config_path() -> Result<String, String> {
    let home_dir = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| "Failed to get home directory".to_string())?;

    let config_path = Path::new(&home_dir).join(".claude").join("settings.json");
    Ok(config_path.to_string_lossy().to_string())
}

/// Reveal Claude config folder in file explorer
#[tauri::command]
pub fn reveal_claude_config_folder() -> Result<(), String> {
    let home_dir = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| "Failed to get home directory".to_string())?;

    let config_dir = Path::new(&home_dir).join(".claude");

    // Ensure directory exists
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create .claude directory: {}", e))?;
    }

    // Open in file explorer (platform-specific)
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(config_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(config_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(config_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    Ok(())
}

/// Read Claude settings.json file
#[tauri::command]
pub async fn read_claude_settings() -> Result<ClaudeSettings, String> {
    let config_path_str = get_claude_config_path()?;
    let config_path = Path::new(&config_path_str);

    if !config_path.exists() {
        // Return empty settings if file doesn't exist
        return Ok(ClaudeSettings {
            env: None,
            other: serde_json::Map::new(),
        });
    }

    let content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read settings file: {}", e))?;

    let settings: ClaudeSettings = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse settings file: {}", e))?;

    Ok(settings)
}

/// Apply Claude Code provider configuration to settings.json
#[tauri::command]
pub async fn apply_claude_config(
    state: tauri::State<'_, DbState>,
    provider_id: String,
) -> Result<(), String> {
    let db = state.0.lock().await;

    // Get the provider
    let provider: Option<ClaudeCodeProviderRecord> = db
        .select(("claude_provider", &provider_id))
        .await
        .map_err(|e| format!("Failed to get provider: {}", e))?;

    let provider = provider.ok_or_else(|| "Provider not found".to_string())?;

    // Parse provider settings_config
    let provider_config: serde_json::Value = serde_json::from_str(&provider.settings_config)
        .map_err(|e| format!("Failed to parse provider config: {}", e))?;

    // Get common config
    let common_config_record: Option<ClaudeCommonConfigRecord> = db
        .select(("claude_common_config", "common"))
        .await
        .map_err(|e| format!("Failed to get common config: {}", e))?;

    let common_config: serde_json::Value = if let Some(record) = common_config_record {
        serde_json::from_str(&record.config)
            .map_err(|e| format!("Failed to parse common config: {}", e))?
    } else {
        serde_json::json!({})
    };

    // Build env section from provider config
    let mut env = serde_json::Map::new();

    // Get env section from provider config
    if let Some(env_config) = provider_config.get("env").and_then(|v| v.as_object()) {
        if let Some(api_key) = env_config.get("ANTHROPIC_API_KEY").and_then(|v| v.as_str()) {
            env.insert(
                "ANTHROPIC_API_KEY".to_string(),
                serde_json::json!(api_key),
            );
        }

        if let Some(base_url) = env_config.get("ANTHROPIC_BASE_URL").and_then(|v| v.as_str()) {
            env.insert(
                "ANTHROPIC_BASE_URL".to_string(),
                serde_json::json!(base_url),
            );
        }

        if let Some(auth_token) = env_config
            .get("ANTHROPIC_AUTH_TOKEN")
            .and_then(|v| v.as_str())
        {
            env.insert(
                "ANTHROPIC_AUTH_TOKEN".to_string(),
                serde_json::json!(auth_token),
            );
        }
    }

    if let Some(model) = provider_config.get("model").and_then(|v| v.as_str()) {
        env.insert("ANTHROPIC_MODEL".to_string(), serde_json::json!(model));
    }

    if let Some(haiku) = provider_config.get("haikuModel").and_then(|v| v.as_str()) {
        env.insert(
            "ANTHROPIC_DEFAULT_HAIKU_MODEL".to_string(),
            serde_json::json!(haiku),
        );
    }

    if let Some(sonnet) = provider_config.get("sonnetModel").and_then(|v| v.as_str()) {
        env.insert(
            "ANTHROPIC_DEFAULT_SONNET_MODEL".to_string(),
            serde_json::json!(sonnet),
        );
    }

    if let Some(opus) = provider_config.get("opusModel").and_then(|v| v.as_str()) {
        env.insert(
            "ANTHROPIC_DEFAULT_OPUS_MODEL".to_string(),
            serde_json::json!(opus),
        );
    }

    // Merge common config and provider env
    let mut final_settings = if let serde_json::Value::Object(map) = common_config {
        map
    } else {
        serde_json::Map::new()
    };

    // Get or create env from common config
    let mut merged_env = final_settings
        .get("env")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    // Merge provider env into common env (provider takes precedence)
    for (key, value) in env {
        merged_env.insert(key, value);
    }

    // Remove old env and insert merged env at the end (env should be at the bottom)
    final_settings.remove("env");
    final_settings.insert("env".to_string(), serde_json::json!(merged_env));

    // Write to settings.json
    let config_path_str = get_claude_config_path()?;
    let config_path = Path::new(&config_path_str);

    // Ensure directory exists
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create .claude directory: {}", e))?;
        }
    }

    let json_content = serde_json::to_string_pretty(&final_settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    fs::write(config_path, json_content)
        .map_err(|e| format!("Failed to write settings file: {}", e))?;

    // Update provider's is_applied status
    let all_providers: Vec<ClaudeCodeProviderRecord> = db
        .select("claude_provider")
        .await
        .map_err(|e| format!("Failed to list providers: {}", e))?;

    for p in all_providers.iter() {
        let content = ClaudeCodeProviderContent {
            provider_id: p.provider_id.clone(),
            name: p.name.clone(),
            category: p.category.clone(),
            settings_config: p.settings_config.clone(),
            source_provider_id: p.source_provider_id.clone(),
            website_url: p.website_url.clone(),
            notes: p.notes.clone(),
            icon: p.icon.clone(),
            icon_color: p.icon_color.clone(),
            sort_index: p.sort_index,
            is_current: p.is_current,
            is_applied: p.provider_id == provider_id,
            created_at: p.created_at.clone(),
            updated_at: Local::now().to_rfc3339(),
        };

        let _: Option<ClaudeCodeProviderRecord> = db
            .update(("claude_provider", &p.provider_id))
            .content(content)
            .await
            .map_err(|e| format!("Failed to update provider: {}", e))?;
    }

    Ok(())
}

// ============================================================================
// Claude Common Config Commands
// ============================================================================

/// Get Claude common config
#[tauri::command]
pub async fn get_claude_common_config(
    state: tauri::State<'_, DbState>,
) -> Result<Option<ClaudeCommonConfig>, String> {
    let db = state.0.lock().await;

    let record: Option<ClaudeCommonConfigRecord> = db
        .select(("claude_common_config", "common"))
        .await
        .map_err(|e| format!("Failed to get common config: {}", e))?;

    Ok(record.map(|r| ClaudeCommonConfig {
        config: r.config,
        updated_at: r.updated_at.unwrap_or_else(|| Local::now().to_rfc3339()),
    }))
}

/// Save Claude common config
#[tauri::command]
pub async fn save_claude_common_config(
    state: tauri::State<'_, DbState>,
    config: String,
) -> Result<(), String> {
    let db = state.0.lock().await;

    // Validate JSON
    let _: serde_json::Value =
        serde_json::from_str(&config).map_err(|e| format!("Invalid JSON: {}", e))?;

    let now = Local::now().to_rfc3339();

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    struct CommonConfigContent {
        config: String,
        updated_at: String,
    }

    let content = CommonConfigContent {
        config,
        updated_at: now,
    };

    let _: Option<ClaudeCommonConfigRecord> = db
        .upsert(("claude_common_config", "common"))
        .content(content)
        .await
        .map_err(|e| format!("Failed to save common config: {}", e))?;

    Ok(())
}
