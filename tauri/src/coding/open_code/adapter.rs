use super::types::{
    OpenCodeCommonConfig, OpenCodeDiagnosticsConfig, OpenCodeFavoritePlugin,
    OpenCodeFavoriteProvider, OpenCodeProvider,
};
use crate::coding::db_id::db_extract_id;
use chrono::Local;
use serde_json::{json, Value};

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
        show_plugins_in_tray: value
            .get("show_plugins_in_tray")
            .or_else(|| value.get("showPluginsInTray"))
            .or_else(|| value.get("show_plugins_in_menu"))
            .or_else(|| value.get("showPluginsInMenu"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
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
        "show_plugins_in_tray": config.show_plugins_in_tray,
        "updated_at": config.updated_at
    })
}

// ============================================================================
// OpenCode Favorite Plugin Adapter Functions
// ============================================================================

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
    let provider_config: OpenCodeProvider =
        serde_json::from_value(value.get("provider_config")?.clone()).ok()?;
    let diagnostics: Option<OpenCodeDiagnosticsConfig> = value
        .get("diagnostics")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
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
        diagnostics,
        created_at,
        updated_at,
    })
}
