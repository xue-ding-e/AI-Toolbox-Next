use chrono::Local;
use serde_json::{json, Map, Value};

use super::types::{PiPromptConfig, PiPromptConfigContent, PiSettingsConfig};
use crate::coding::db_id::db_extract_id;

pub fn settings_from_db_value(value: Value) -> PiSettingsConfig {
    PiSettingsConfig {
        root_dir: value
            .get("root_dir")
            .and_then(Value::as_str)
            .map(str::to_string),
        updated_at: value
            .get("updated_at")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| Local::now().to_rfc3339()),
    }
}

pub fn settings_to_db_value(root_dir: Option<&str>) -> Value {
    let now = Local::now().to_rfc3339();
    let mut value = json!({ "updated_at": now });
    if let Some(root_dir) = root_dir.filter(|dir| !dir.trim().is_empty()) {
        value["root_dir"] = json!(root_dir);
    }
    value
}

pub fn prompt_from_db_value(value: Value) -> PiPromptConfig {
    PiPromptConfig {
        id: db_extract_id(&value),
        name: value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        content: value
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        is_applied: value
            .get("is_applied")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        sort_index: value
            .get("sort_index")
            .and_then(Value::as_i64)
            .map(|value| value as i32),
        created_at: value
            .get("created_at")
            .and_then(Value::as_str)
            .map(str::to_string),
        updated_at: value
            .get("updated_at")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

pub fn prompt_to_db_value(content: &PiPromptConfigContent) -> Value {
    let mut map = Map::new();
    map.insert("name".to_string(), Value::String(content.name.clone()));
    map.insert(
        "content".to_string(),
        Value::String(content.content.clone()),
    );
    map.insert("is_applied".to_string(), Value::Bool(content.is_applied));
    if let Some(sort_index) = content.sort_index {
        map.insert("sort_index".to_string(), json!(sort_index));
    }
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
