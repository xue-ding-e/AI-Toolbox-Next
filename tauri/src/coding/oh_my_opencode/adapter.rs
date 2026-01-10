use serde_json::{json, Value};
use super::types::{OhMyOpenCodeConfig, OhMyOpenCodeConfigContent, OhMyOpenCodeGlobalConfig, OhMyOpenCodeGlobalConfigContent};
use std::collections::HashMap;

// ============================================================================
// Helper Functions
// ============================================================================

/// Helper function to get string value with backward compatibility (camelCase and snake_case)
fn get_str_compat(value: &Value, snake_key: &str, camel_key: &str, default: &str) -> String {
    value
        .get(snake_key)
        .or_else(|| value.get(camel_key))
        .and_then(|v| v.as_str())
        .unwrap_or(default)
        .to_string()
}

/// Helper function to get optional string with backward compatibility
fn get_opt_str_compat(value: &Value, snake_key: &str, camel_key: &str) -> Option<String> {
    value
        .get(snake_key)
        .or_else(|| value.get(camel_key))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Helper function to get bool with backward compatibility
fn get_bool_compat(value: &Value, snake_key: &str, camel_key: &str, default: bool) -> bool {
    value
        .get(snake_key)
        .or_else(|| value.get(camel_key))
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

/// Merge snake_case and camelCase Sisyphus config values for backward compatibility
/// Prefers snake_case values, fills missing fields from camelCase
fn merge_sisyphus_config(snake: Value, camel: Value) -> Option<Value> {
    let snake_obj = snake.as_object()?;
    let camel_obj = camel.as_object()?;

    let mut merged = serde_json::Map::new();

    // Map of camelCase to snake_case field names
    let field_map = [
        ("disabled", "disabled"),
        ("defaultBuilderEnabled", "default_builder_enabled"),
        ("plannerEnabled", "planner_enabled"),
        ("replacePlan", "replace_plan"),
    ];

    for (camel_key, snake_key) in field_map {
        // Prefer snake_case value
        if let Some(value) = snake_obj.get(snake_key) {
            merged.insert(snake_key.to_string(), value.clone());
        } else if let Some(value) = camel_obj.get(camel_key) {
            // Fall back to camelCase value
            merged.insert(snake_key.to_string(), value.clone());
        }
    }

    if merged.is_empty() {
        None
    } else {
        Some(Value::Object(merged))
    }
}

/// Deep merge two JSON Values recursively
/// Overlay values will overwrite base values for the same keys
pub fn deep_merge_json(base: &mut Value, overlay: &Value) {
    if let (Some(base_obj), Some(overlay_obj)) = (base.as_object_mut(), overlay.as_object()) {
        for (key, value) in overlay_obj {
            if let Some(base_value) = base_obj.get_mut(key) {
                if base_value.is_object() && value.is_object() {
                    deep_merge_json(base_value, value);
                } else {
                    *base_value = value.clone();
                }
            } else {
                base_obj.insert(key.clone(), value.clone());
            }
        }
    }
}

// ============================================================================
// Adapter Functions
// ============================================================================

/// Convert database Value to OhMyOpenCodeConfig (AgentsProfile) with fault tolerance
/// 简化版：只包含 agents 和 other_fields
pub fn from_db_value(value: Value) -> OhMyOpenCodeConfig {
    let agents_value = value
        .get("agents")
        .cloned()
        .unwrap_or(json!({}));
    
    let agents: HashMap<String, serde_json::Value> = 
        serde_json::from_value(agents_value).unwrap_or_default();

    OhMyOpenCodeConfig {
        id: get_str_compat(&value, "config_id", "configId", ""),
        name: get_str_compat(&value, "name", "name", "Unnamed Config"),
        is_applied: get_bool_compat(&value, "is_applied", "isApplied", false),
        agents: agents.into_iter().map(|(k, v)| {
            (k, serde_json::from_value(v).unwrap_or_default())
        }).collect(),
        other_fields: value
            .get("other_fields")
            .or_else(|| value.get("otherFields"))
            .cloned(),
        created_at: get_opt_str_compat(&value, "created_at", "createdAt"),
        updated_at: get_opt_str_compat(&value, "updated_at", "updatedAt"),
    }
}

/// Convert OhMyOpenCodeConfigContent to database Value
pub fn to_db_value(content: &OhMyOpenCodeConfigContent) -> Value {
    serde_json::to_value(content).unwrap_or_else(|e| {
        eprintln!("Failed to serialize oh-my-opencode config content: {}", e);
        json!({})
    })
}

/// Convert database Value to OhMyOpenCodeGlobalConfig with fault tolerance
pub fn global_config_from_db_value(value: Value) -> OhMyOpenCodeGlobalConfig {
    OhMyOpenCodeGlobalConfig {
        id: get_str_compat(&value, "config_id", "configId", "global"),
        schema: value
            .get("schema")
            .or_else(|| value.get("schema"))
            .and_then(|v| v.as_str())
            .map(String::from),
        // Try snake_case first, then camelCase for backward compatibility
        sisyphus_agent: {
            let snake_case_value = value.get("sisyphus_agent").cloned();
            let camel_case_value = value.get("sisyphusAgent").cloned();
            let merged = match (snake_case_value, camel_case_value) {
                (Some(snake), Some(camel)) => {
                    // Merge: prefer snake_case values, fill missing with camelCase
                    merge_sisyphus_config(snake, camel)
                }
                (Some(snake), None) => Some(snake),
                (None, Some(camel)) => Some(camel),
                (None, None) => None,
            };
            merged.and_then(|v| serde_json::from_value(v).ok())
        },
        disabled_agents: value
            .get("disabled_agents")
            .or_else(|| value.get("disabledAgents"))
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        disabled_mcps: value
            .get("disabled_mcps")
            .or_else(|| value.get("disabledMcps"))
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        disabled_hooks: value
            .get("disabled_hooks")
            .or_else(|| value.get("disabledHooks"))
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        lsp: value
            .get("lsp")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        experimental: value
            .get("experimental")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        other_fields: value
            .get("other_fields")
            .or_else(|| value.get("otherFields"))
            .cloned(),
        updated_at: get_opt_str_compat(&value, "updated_at", "updatedAt"),
    }
}

/// Convert OhMyOpenCodeGlobalConfigContent to database Value
pub fn global_config_to_db_value(content: &OhMyOpenCodeGlobalConfigContent) -> Value {
    serde_json::to_value(content).unwrap_or_else(|e| {
        eprintln!("Failed to serialize oh-my-opencode global config content: {}", e);
        json!({})
    })
}
