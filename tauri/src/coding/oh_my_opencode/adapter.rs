use serde_json::{json, Value};
use super::types::{OhMyOpenCodeConfig, OhMyOpenCodeConfigContent};
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

// ============================================================================
// Adapter Functions
// ============================================================================

/// Convert database Value to OhMyOpenCodeConfig with fault tolerance
/// Supports both snake_case (new) and camelCase (legacy) field names
pub fn from_db_value(value: Value) -> OhMyOpenCodeConfig {
    let agents_value = value
        .get("agents")
        .or_else(|| value.get("agents"))
        .cloned()
        .unwrap_or(json!({}));
    
    let agents: HashMap<String, serde_json::Value> = 
        serde_json::from_value(agents_value).unwrap_or_default();

    OhMyOpenCodeConfig {
        id: get_str_compat(&value, "config_id", "configId", ""),
        name: get_str_compat(&value, "name", "name", "Unnamed Config"),
        is_applied: get_bool_compat(&value, "is_applied", "isApplied", false),
        schema: get_opt_str_compat(&value, "schema", "schema"),
        agents: agents.into_iter().map(|(k, v)| {
            (k, serde_json::from_value(v).unwrap_or_default())
        }).collect(),
        sisyphus_agent: value
            .get("sisyphus_agent")
            .or_else(|| value.get("sisyphusAgent"))
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
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
        disabled_skills: value
            .get("disabled_skills")
            .or_else(|| value.get("disabledSkills"))
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        disabled_commands: value
            .get("disabled_commands")
            .or_else(|| value.get("disabledCommands"))
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
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
