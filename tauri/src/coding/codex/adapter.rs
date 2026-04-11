use chrono::Local;
use serde_json::Value;

use super::commands::infer_codex_provider_category_from_settings;
use super::types::{
    CodexCommonConfig, CodexPromptConfig, CodexPromptConfigContent, CodexProvider,
    CodexProviderContent,
};
use crate::coding::db_id::db_extract_id;

// ============================================================================
// Provider Adapter Functions
// ============================================================================

/// Convert database value to CodexProvider
pub fn from_db_value_provider(value: Value) -> CodexProvider {
    // Use common utility to extract and clean the record ID
    let id = db_extract_id(&value);
    let settings_config = value
        .get("settings_config")
        .and_then(|v| v.as_str())
        .unwrap_or("{}")
        .to_string();
    let inferred_category = serde_json::from_str::<Value>(&settings_config)
        .map(|parsed| infer_codex_provider_category_from_settings(&parsed))
        .unwrap_or_else(|_| "custom".to_string());
    let stored_category = value
        .get("category")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let category = match stored_category.as_deref() {
        Some("official") => "official".to_string(),
        Some("custom") if inferred_category == "official" => inferred_category,
        Some("custom") => "custom".to_string(),
        Some(other) => other.to_string(),
        None => inferred_category,
    };

    CodexProvider {
        id,
        name: value
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        category,
        settings_config,
        source_provider_id: value
            .get("source_provider_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        website_url: value
            .get("website_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        notes: value
            .get("notes")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        icon: value
            .get("icon")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        icon_color: value
            .get("icon_color")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        sort_index: value
            .get("sort_index")
            .and_then(|v| v.as_i64())
            .map(|n| n as i32),
        is_applied: value
            .get("is_applied")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        is_disabled: value
            .get("is_disabled")
            .or_else(|| value.get("isDisabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        created_at: value
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        updated_at: value
            .get("updated_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    }
}

/// Convert CodexProviderContent to database value
pub fn to_db_value_provider(content: &CodexProviderContent) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("name".to_string(), Value::String(content.name.clone()));
    map.insert(
        "category".to_string(),
        Value::String(content.category.clone()),
    );
    map.insert(
        "settings_config".to_string(),
        Value::String(content.settings_config.clone()),
    );

    if let Some(ref source_id) = content.source_provider_id {
        map.insert(
            "source_provider_id".to_string(),
            Value::String(source_id.clone()),
        );
    }
    if let Some(ref url) = content.website_url {
        map.insert("website_url".to_string(), Value::String(url.clone()));
    }
    if let Some(ref notes) = content.notes {
        map.insert("notes".to_string(), Value::String(notes.clone()));
    }
    if let Some(ref icon) = content.icon {
        map.insert("icon".to_string(), Value::String(icon.clone()));
    }
    if let Some(ref color) = content.icon_color {
        map.insert("icon_color".to_string(), Value::String(color.clone()));
    }
    if let Some(index) = content.sort_index {
        map.insert("sort_index".to_string(), Value::Number(index.into()));
    }

    map.insert("is_applied".to_string(), Value::Bool(content.is_applied));
    map.insert("is_disabled".to_string(), Value::Bool(content.is_disabled));
    map.insert(
        "created_at".to_string(),
        Value::String(content.created_at.clone()),
    );
    map.insert(
        "updated_at".to_string(),
        Value::String(content.updated_at.clone()),
    );

    Value::Object(map)
}

// ============================================================================
// Common Config Adapter Functions
// ============================================================================

/// Convert database value to CodexCommonConfig
pub fn from_db_value_common(value: Value) -> CodexCommonConfig {
    let updated_at_value = value
        .get("updated_at")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    CodexCommonConfig {
        config: value
            .get("config")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        root_dir: value
            .get("root_dir")
            .or_else(|| value.get("rootDir"))
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
        updated_at: updated_at_value.unwrap_or_else(|| Local::now().to_rfc3339()),
    }
}

/// Convert config string to database value
pub fn to_db_value_common(config: &str, root_dir: Option<&str>) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("config".to_string(), Value::String(config.to_string()));
    if let Some(root_dir) = root_dir.filter(|dir| !dir.trim().is_empty()) {
        map.insert("root_dir".to_string(), Value::String(root_dir.to_string()));
    }
    map.insert(
        "updated_at".to_string(),
        Value::String(Local::now().to_rfc3339()),
    );
    Value::Object(map)
}

// ============================================================================
// Prompt Adapter Functions
// ============================================================================

pub fn from_db_value_prompt(value: Value) -> CodexPromptConfig {
    let id = db_extract_id(&value);

    CodexPromptConfig {
        id,
        name: value
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unnamed Prompt")
            .to_string(),
        content: value
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        is_applied: value
            .get("is_applied")
            .or_else(|| value.get("isApplied"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        sort_index: value
            .get("sort_index")
            .or_else(|| value.get("sortIndex"))
            .and_then(|v| v.as_i64())
            .map(|n| n as i32),
        created_at: value
            .get("created_at")
            .or_else(|| value.get("createdAt"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        updated_at: value
            .get("updated_at")
            .or_else(|| value.get("updatedAt"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    }
}

pub fn to_db_value_prompt(content: &CodexPromptConfigContent) -> Value {
    serde_json::to_value(content).unwrap_or_else(|e| {
        eprintln!("Failed to serialize Codex prompt content: {}", e);
        Value::Object(serde_json::Map::new())
    })
}
