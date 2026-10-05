use chrono::Local;
use serde_json::{json, Value};

use crate::coding::db_id::db_extract_id;

use super::types::{
    ZcodeCommonConfig, ZcodeCommonConfigRecord, ZcodePromptConfig, ZcodeProvider,
    ZcodeProviderContent,
};

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

fn get_i64_compat(value: &Value, snake_key: &str, camel_key: &str) -> i64 {
    value
        .get(snake_key)
        .or_else(|| value.get(camel_key))
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
}

fn get_bool_compat(value: &Value, snake_key: &str, camel_key: &str, default: bool) -> bool {
    value
        .get(snake_key)
        .or_else(|| value.get(camel_key))
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

pub fn from_db_value_provider(value: Value) -> ZcodeProvider {
    ZcodeProvider {
        id: db_extract_id(&value),
        name: get_str_compat(&value, "name", "name", "Unnamed Provider"),
        category: get_str_compat(&value, "category", "category", "custom"),
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

pub fn to_db_value_provider(content: &ZcodeProviderContent) -> Value {
    serde_json::to_value(content).unwrap_or_else(|error| {
        eprintln!("Failed to serialize ZCode provider content: {error}");
        json!({})
    })
}

pub fn from_db_value_common(value: Value) -> ZcodeCommonConfigRecord {
    ZcodeCommonConfigRecord {
        id: db_extract_id(&value),
        config: get_str_compat(&value, "config", "config", "{}"),
        root_dir: get_opt_str_compat(&value, "root_dir", "rootDir"),
        updated_at: get_str_compat(&value, "updated_at", "updatedAt", ""),
    }
}

pub fn to_db_value_common(config: &str, root_dir: Option<&str>) -> Value {
    json!({
        "config": config,
        "root_dir": root_dir,
        "updated_at": Local::now().to_rfc3339(),
    })
}

pub fn from_db_value_prompt(value: Value) -> ZcodePromptConfig {
    ZcodePromptConfig {
        id: db_extract_id(&value),
        name: get_str_compat(&value, "name", "name", "Unnamed Prompt"),
        content: get_str_compat(&value, "content", "content", ""),
        is_applied: get_bool_compat(&value, "is_applied", "isApplied", false),
        sort_index: get_i64_compat(&value, "sort_index", "sortIndex"),
        created_at: get_str_compat(&value, "created_at", "createdAt", ""),
        updated_at: get_str_compat(&value, "updated_at", "updatedAt", ""),
    }
}

pub fn to_db_value_prompt(prompt: &ZcodePromptConfig) -> Value {
    serde_json::to_value(prompt).unwrap_or_else(|error| {
        eprintln!("Failed to serialize ZCode prompt config: {error}");
        json!({})
    })
}

/// Convenience conversion used when a caller already holds the record type.
impl From<ZcodeCommonConfigRecord> for ZcodeCommonConfig {
    fn from(record: ZcodeCommonConfigRecord) -> Self {
        Self {
            config: record.config,
            root_dir: record.root_dir,
            updated_at: record.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_round_trips_through_db_value() {
        let content = ZcodeProviderContent {
            name: "DeepSeek".to_string(),
            category: "custom".to_string(),
            settings_config: "{\"providerId\":\"custom:deepseek\"}".to_string(),
            source_provider_id: None,
            website_url: None,
            notes: Some("note".to_string()),
            icon: None,
            icon_color: None,
            sort_index: 3,
            meta: Some(json!({ "costMultiplier": 0.5 })),
            is_disabled: false,
        };
        let value = to_db_value_provider(&content);
        assert_eq!(value["settings_config"], json!("{\"providerId\":\"custom:deepseek\"}"));
        assert_eq!(value["meta"]["costMultiplier"], json!(0.5));

        let mut with_id = value.clone();
        with_id["id"] = json!("row-1");
        with_id["is_applied"] = json!(true);
        let parsed = from_db_value_provider(with_id);
        assert_eq!(parsed.id, "row-1");
        assert_eq!(parsed.sort_index, 3);
        assert!(parsed.is_applied);
        assert_eq!(parsed.notes.as_deref(), Some("note"));
    }

    #[test]
    fn common_config_tolerates_missing_root_dir() {
        let parsed = from_db_value_common(json!({ "id": "common", "config": "{}" }));
        assert_eq!(parsed.id, "common");
        assert_eq!(parsed.root_dir, None);
    }
}
