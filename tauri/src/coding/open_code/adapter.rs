use serde_json::{json, Value};
use super::types::OpenCodeCommonConfig;
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

