use serde_json::{json, Value};
use super::types::{OpenCodeCommonConfig, OpenCodeFavoritePlugin, OpenCodeFavoriteProvider, OpenCodeProvider};
use chrono::Local;

// ============================================================================
// OpenCode Common Config Adapter Functions
// ============================================================================

/// Convert database Value to OpenCodeCommonConfig with fault tolerance
/// Supports both snake_case (new) and camelCase (legacy) field names
pub fn from_db_value(value: Value) -> OpenCodeCommonConfig {
    OpenCodeCommonConfig {
        config_path: value
            .get("config_path")
            .or_else(|| value.get("configPath"))
            .and_then(|v| v.as_str())
            .map(String::from),
        updated_at: value
            .get("updated_at")
            .or_else(|| value.get("updatedAt"))
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| {
                let now = Local::now().to_rfc3339();
                Box::leak(now.into_boxed_str())
            })
            .to_string(),
    }
}

/// Convert OpenCodeCommonConfig to database Value
pub fn to_db_value(config: &OpenCodeCommonConfig) -> Value {
    json!({
        "config_path": config.config_path,
        "updated_at": config.updated_at
    })
}

// ============================================================================
// OpenCode Favorite Plugin Adapter Functions
// ============================================================================

/// Extract record ID from database Value
fn db_extract_id(value: &Value) -> String {
    value
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Convert database Value to OpenCodeFavoritePlugin
pub fn from_db_value_favorite_plugin(value: Value) -> OpenCodeFavoritePlugin {
    let id = db_extract_id(&value);
    OpenCodeFavoritePlugin {
        id,
        plugin_name: value
            .get("plugin_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        created_at: value
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    }
}

// ============================================================================
// OpenCode Favorite Provider Adapter Functions
// ============================================================================

/// Convert database Value to OpenCodeFavoriteProvider
pub fn from_db_value_favorite_provider(value: Value) -> Option<OpenCodeFavoriteProvider> {
    let id = db_extract_id(&value);
    let provider_id = value.get("provider_id")?.as_str()?.to_string();
    let npm = value
        .get("npm")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let base_url = value
        .get("base_url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let provider_config: OpenCodeProvider = serde_json::from_value(
        value.get("provider_config")?.clone()
    ).ok()?;
    let created_at = value
        .get("created_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let updated_at = value
        .get("updated_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Some(OpenCodeFavoriteProvider {
        id,
        provider_id,
        npm,
        base_url,
        provider_config,
        created_at,
        updated_at,
    })
}

