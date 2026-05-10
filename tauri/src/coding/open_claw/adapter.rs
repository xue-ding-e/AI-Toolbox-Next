use super::types::OpenClawCommonConfig;
use chrono::Local;
use serde_json::{json, Value};

/// Convert database Value to OpenClawCommonConfig with fault tolerance
pub fn from_db_value(value: Value) -> OpenClawCommonConfig {
    OpenClawCommonConfig {
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

/// Convert OpenClawCommonConfig to database Value
pub fn to_db_value(config: &OpenClawCommonConfig) -> Value {
    json!({
        "config_path": config.config_path,
        "updated_at": config.updated_at
    })
}
