use super::types::{
    ClaudeDesktopCommonConfig, ClaudeDesktopPromptConfig, ClaudeDesktopPromptConfigContent,
    ClaudeDesktopProvider, ClaudeDesktopProviderContent,
};
use crate::coding::db_id::db_extract_id;
use chrono::Local;
use serde_json::{json, Value};

// ============================================================================
// Provider Adapter Functions
// ============================================================================

/// Get string value with backward compatibility (camelCase and snake_case).
fn get_str_compat(value: &Value, snake_key: &str, camel_key: &str, default: &str) -> String {
    value
        .get(snake_key)
        .or_else(|| value.get(camel_key))
        .and_then(|v| v.as_str())
        .unwrap_or(default)
        .to_string()
}

fn get_opt_str_compat(value: &Value, snake_key: &str, camel_key: &str) -> Option<String> {
    value
        .get(snake_key)
        .or_else(|| value.get(camel_key))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn get_i64_compat(value: &Value, snake_key: &str, camel_key: &str) -> Option<i32> {
    value
        .get(snake_key)
        .or_else(|| value.get(camel_key))
        .and_then(|v| v.as_i64())
        .map(|v| v as i32)
}

fn get_bool_compat(value: &Value, snake_key: &str, camel_key: &str, default: bool) -> bool {
    value
        .get(snake_key)
        .or_else(|| value.get(camel_key))
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

/// Convert database Value to ClaudeDesktopProvider with fault tolerance.
/// Supports both snake_case (new) and camelCase (legacy) field names.
pub fn from_db_value_provider(value: Value) -> ClaudeDesktopProvider {
    let id = db_extract_id(&value);

    ClaudeDesktopProvider {
        id,
        name: get_str_compat(&value, "name", "name", "Unnamed Provider"),
        category: get_str_compat(&value, "category", "category", "other"),
        settings_config: get_str_compat(&value, "settings_config", "settingsConfig", "{}"),
        source_provider_id: get_opt_str_compat(&value, "source_provider_id", "sourceProviderId"),
        website_url: get_opt_str_compat(&value, "website_url", "websiteUrl"),
        notes: get_opt_str_compat(&value, "notes", "notes"),
        icon: get_opt_str_compat(&value, "icon", "icon"),
        icon_color: get_opt_str_compat(&value, "icon_color", "iconColor"),
        sort_index: get_i64_compat(&value, "sort_index", "sortIndex"),
        meta: value.get("meta").cloned(),
        is_applied: get_bool_compat(&value, "is_applied", "isApplied", false),
        is_disabled: get_bool_compat(&value, "is_disabled", "isDisabled", false),
        created_at: get_str_compat(&value, "created_at", "createdAt", ""),
        updated_at: get_str_compat(&value, "updated_at", "updatedAt", ""),
    }
}

/// Convert ClaudeDesktopProviderContent to database Value.
pub fn to_db_value_provider(content: &ClaudeDesktopProviderContent) -> Value {
    serde_json::to_value(content).unwrap_or_else(|_| json!({}))
}

// ============================================================================
// Common Config Adapter Functions
// ============================================================================

/// Convert database Value to ClaudeDesktopCommonConfig.
pub fn from_db_value_common(value: Value) -> ClaudeDesktopCommonConfig {
    ClaudeDesktopCommonConfig {
        config: value
            .get("config")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
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

/// Convert common config string to database Value.
pub fn to_db_value_common(config: &str) -> Value {
    json!({
        "config": config,
        "updated_at": Local::now().to_rfc3339()
    })
}

// ============================================================================
// Prompt Config Adapter Functions
// ============================================================================

/// Convert database Value to ClaudeDesktopPromptConfig with fault tolerance.
/// Supports both snake_case (new) and camelCase (legacy) field names.
pub fn from_db_value_prompt(value: Value) -> ClaudeDesktopPromptConfig {
    ClaudeDesktopPromptConfig {
        id: db_extract_id(&value),
        name: get_str_compat(&value, "name", "name", "Unnamed Prompt"),
        content: get_str_compat(&value, "content", "content", ""),
        is_applied: get_bool_compat(&value, "is_applied", "isApplied", false),
        sort_index: get_i64_compat(&value, "sort_index", "sortIndex"),
        created_at: get_opt_str_compat(&value, "created_at", "createdAt"),
        updated_at: get_opt_str_compat(&value, "updated_at", "updatedAt"),
    }
}

/// Convert ClaudeDesktopPromptConfigContent to database Value.
pub fn to_db_value_prompt(content: &ClaudeDesktopPromptConfigContent) -> Value {
    serde_json::to_value(content).unwrap_or_else(|_| json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_reads_both_snake_and_camel_case() {
        let provider = from_db_value_provider(json!({
            "id": "p1",
            "name": "Direct",
            "category": "custom",
            "settingsConfig": "{}",
            "isApplied": true
        }));

        assert_eq!(provider.id, "p1");
        assert_eq!(provider.name, "Direct");
        assert_eq!(provider.settings_config, "{}");
        assert!(provider.is_applied);
    }

    #[test]
    fn common_config_reads_config_and_updated_at() {
        let common = from_db_value_common(json!({
            "config": "{\"mcpServers\":{}}",
            "updated_at": "2026-01-01T00:00:00Z"
        }));

        assert_eq!(common.config, "{\"mcpServers\":{}}");
        assert_eq!(common.updated_at, "2026-01-01T00:00:00Z");
    }
}
