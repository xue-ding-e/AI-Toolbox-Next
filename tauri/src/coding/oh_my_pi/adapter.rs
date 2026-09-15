use chrono::Local;
use serde_json::{json, Map, Value};

use super::types::{
    OmpAgentsConfig, OmpAgentsConfigContent, OmpPromptConfig, OmpPromptConfigContent,
    OmpSettingsConfig,
};
use crate::coding::db_id::db_extract_id;

pub fn settings_from_db_value(value: Value) -> OmpSettingsConfig {
    OmpSettingsConfig {
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
    // 注意:这是整条记录重建,不是 patch。当前记录只有 root_dir/updated_at
    // 两个字段,所以无损;但若日后给 OhMyPiSettingsConfig 增加字段(例如上游
    // OMP_PROFILE 对应的 profile),这里会把存量记录里未随之写入的字段静默
    // 清掉。届时应改用 db_patch_fields 做局部更新。(Pi 的 settings 同写法。)
    let now = Local::now().to_rfc3339();
    let mut value = json!({ "updated_at": now });
    if let Some(root_dir) = root_dir.filter(|dir| !dir.trim().is_empty()) {
        value["root_dir"] = json!(root_dir);
    }
    value
}

pub fn prompt_from_db_value(value: Value) -> OmpPromptConfig {
    OmpPromptConfig {
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

pub fn prompt_to_db_value(content: &OmpPromptConfigContent) -> Value {
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

// ============================================================================
// OMP subagent 方案(oh_my_pi_agents_config)adapter
// ============================================================================

pub fn agents_from_db_value(value: Value) -> OmpAgentsConfig {
    OmpAgentsConfig {
        id: db_extract_id(&value),
        name: value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Unnamed Config")
            .to_string(),
        is_applied: value
            .get("is_applied")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        is_disabled: value
            .get("is_disabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        model_roles: value.get("model_roles").cloned(),
        agents: value.get("agents").cloned(),
        other_fields: value.get("other_fields").cloned(),
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

pub fn agents_to_db_value(content: &OmpAgentsConfigContent) -> Value {
    let mut map = Map::new();
    map.insert("name".to_string(), Value::String(content.name.clone()));
    map.insert("is_applied".to_string(), Value::Bool(content.is_applied));
    map.insert("is_disabled".to_string(), Value::Bool(content.is_disabled));
    if let Some(model_roles) = &content.model_roles {
        map.insert("model_roles".to_string(), model_roles.clone());
    }
    if let Some(agents) = &content.agents {
        map.insert("agents".to_string(), agents.clone());
    }
    if let Some(other_fields) = &content.other_fields {
        map.insert("other_fields".to_string(), other_fields.clone());
    }
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
