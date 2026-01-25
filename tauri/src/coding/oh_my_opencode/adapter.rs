use super::types::{
    OhMyOpenCodeConfig, OhMyOpenCodeConfigContent, OhMyOpenCodeGlobalConfig,
    OhMyOpenCodeGlobalConfigContent,
};
use crate::coding::db_id::db_extract_id;
use serde_json::{json, Value};

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
pub fn get_bool_compat(value: &Value, snake_key: &str, camel_key: &str, default: bool) -> bool {
    value
        .get(snake_key)
        .or_else(|| value.get(camel_key))
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
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

/// Recursively remove empty objects and null values from a JSON value
/// This is useful for cleaning up config files before writing
pub fn clean_empty_values(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|_key, v| {
                clean_empty_values(v);
                // 删除空对象和 null 值，保留空数组
                !(v.is_object() && v.as_object().unwrap().is_empty()) && !v.is_null()
            });
        }
        _ => {}
    }
}

// ============================================================================
// Adapter Functions
// ============================================================================

/// Convert database Value to OhMyOpenCodeConfig (AgentsProfile) with fault tolerance
pub fn from_db_value(value: Value) -> OhMyOpenCodeConfig {
    let is_applied = get_bool_compat(&value, "is_applied", "isApplied", false);
    let is_disabled = get_bool_compat(&value, "is_disabled", "isDisabled", false);
    OhMyOpenCodeConfig {
        id: db_extract_id(&value),
        name: get_str_compat(&value, "name", "name", "Unnamed Config"),
        is_applied,
        is_disabled,
        agents: value.get("agents").cloned(),
        categories: value.get("categories").cloned(),
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

/// Helper function to safely convert Value to Option<Vec<String>>, handling SurrealDB types
fn safe_to_string_array(value: &Value) -> Option<Vec<String>> {
    match value {
        // Already an array of strings
        Value::Array(arr) => {
            let mut result = Vec::new();
            for item in arr {
                if let Some(s) = item.as_str() {
                    result.push(s.to_string());
                } else {
                    // Non-string item, try to convert
                    if let Ok(s) = serde_json::from_value(item.clone()) {
                        result.push(s);
                    } else {
                        return None;
                    }
                }
            }
            Some(result)
        }
        // SurrealDB enum - try to parse
        Value::String(s) if s.starts_with("enum(") => {
            // Try to extract the value from enum format
            let inner = s.trim_start_matches("enum(").trim_end_matches(')');
            Some(vec![inner.to_string()])
        }
        _ => {
            // Try generic conversion
            serde_json::from_value(value.clone()).ok()
        }
    }
}

/// Convert database Value to OhMyOpenCodeGlobalConfig with fault tolerance
pub fn global_config_from_db_value(value: Value) -> OhMyOpenCodeGlobalConfig {
    OhMyOpenCodeGlobalConfig {
        id: db_extract_id(&value),
        schema: value
            .get("schema")
            .or_else(|| value.get("schema"))
            .and_then(|v| v.as_str())
            .map(String::from),
        sisyphus_agent: value
            .get("sisyphus_agent")
            .or_else(|| value.get("sisyphusAgent"))
            .cloned(),
        disabled_agents: value
            .get("disabled_agents")
            .or_else(|| value.get("disabledAgents"))
            .and_then(|v| safe_to_string_array(v)),
        disabled_mcps: value
            .get("disabled_mcps")
            .or_else(|| value.get("disabledMcps"))
            .and_then(|v| safe_to_string_array(v)),
        disabled_hooks: value
            .get("disabled_hooks")
            .or_else(|| value.get("disabledHooks"))
            .and_then(|v| safe_to_string_array(v)),
        disabled_skills: value
            .get("disabled_skills")
            .or_else(|| value.get("disabledSkills"))
            .and_then(|v| safe_to_string_array(v)),
        lsp: value.get("lsp").cloned(),
        experimental: value.get("experimental").cloned(),
        background_task: value
            .get("background_task")
            .or_else(|| value.get("backgroundTask"))
            .cloned(),
        browser_automation_engine: value
            .get("browser_automation_engine")
            .or_else(|| value.get("browserAutomationEngine"))
            .cloned(),
        claude_code: value
            .get("claude_code")
            .or_else(|| value.get("claudeCode"))
            .cloned(),
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
        eprintln!(
            "Failed to serialize oh-my-opencode global config content: {}",
            e
        );
        json!({})
    })
}
