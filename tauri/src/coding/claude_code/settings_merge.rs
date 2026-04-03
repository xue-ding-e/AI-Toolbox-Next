use serde_json::{Map, Value};

const PROTECTED_TOP_LEVEL_FIELDS: [&str; 3] = ["enabledPlugins", "extraKnownMarketplaces", "hooks"];

const PROVIDER_MODEL_FIELD_MAPPINGS: [(&str, &str); 5] = [
    ("model", "ANTHROPIC_MODEL"),
    ("haikuModel", "ANTHROPIC_DEFAULT_HAIKU_MODEL"),
    ("sonnetModel", "ANTHROPIC_DEFAULT_SONNET_MODEL"),
    ("opusModel", "ANTHROPIC_DEFAULT_OPUS_MODEL"),
    ("reasoningModel", "ANTHROPIC_REASONING_MODEL"),
];

fn value_as_object(value: &Value) -> Option<&Map<String, Value>> {
    value.as_object()
}

pub fn parse_json_object(raw_json: &str) -> Result<Map<String, Value>, String> {
    if raw_json.trim().is_empty() {
        return Ok(Map::new());
    }

    match serde_json::from_str::<Value>(raw_json)
        .map_err(|error| format!("Failed to parse JSON object: {}", error))?
    {
        Value::Object(object) => Ok(object),
        _ => Err("Expected JSON object".to_string()),
    }
}

pub fn build_provider_managed_env(
    provider_config: &Value,
    known_env_fields: &[&str],
) -> Map<String, Value> {
    let mut managed_env = Map::new();

    if let Some(provider_env) = provider_config.get("env").and_then(value_as_object) {
        let api_key_value = provider_env
            .get("ANTHROPIC_AUTH_TOKEN")
            .or_else(|| provider_env.get("ANTHROPIC_API_KEY"));
        if let Some(api_key_value) = api_key_value {
            managed_env.insert("ANTHROPIC_AUTH_TOKEN".to_string(), api_key_value.clone());
        }

        if let Some(base_url_value) = provider_env.get("ANTHROPIC_BASE_URL") {
            managed_env.insert("ANTHROPIC_BASE_URL".to_string(), base_url_value.clone());
        }
    }

    for (provider_field, env_field) in PROVIDER_MODEL_FIELD_MAPPINGS {
        if let Some(field_value) = provider_config.get(provider_field) {
            managed_env.insert(env_field.to_string(), field_value.clone());
        }
    }

    managed_env.retain(|key, value| {
        known_env_fields.contains(&key.as_str())
            && !value.is_null()
            && !value.as_str().is_some_and(str::is_empty)
    });

    managed_env
}

pub fn merge_claude_settings_for_provider(
    current_disk_settings: Option<&Value>,
    previous_common_config: Option<&Value>,
    next_common_config: &Value,
    provider_config: &Value,
    known_env_fields: &[&str],
) -> Result<Value, String> {
    let current_settings_object = match current_disk_settings {
        Some(Value::Object(object)) => object.clone(),
        Some(_) => return Err("Current Claude settings must be a JSON object".to_string()),
        None => Map::new(),
    };

    let next_common_config_object = match next_common_config {
        Value::Object(object) => object.clone(),
        Value::Null => Map::new(),
        _ => return Err("Claude common config must be a JSON object".to_string()),
    };
    let previous_common_config_object = match previous_common_config {
        Some(Value::Object(object)) => object.clone(),
        Some(Value::Null) => Map::new(),
        Some(_) => return Err("Previous Claude common config must be a JSON object".to_string()),
        None => next_common_config_object.clone(),
    };

    let mut merged_settings = current_settings_object;

    for field_key in previous_common_config_object.keys() {
        if field_key == "env" {
            continue;
        }

        if PROTECTED_TOP_LEVEL_FIELDS.contains(&field_key.as_str()) {
            continue;
        }

        merged_settings.remove(field_key);
    }

    for (field_key, field_value) in &next_common_config_object {
        if field_key == "env" {
            continue;
        }

        if PROTECTED_TOP_LEVEL_FIELDS.contains(&field_key.as_str()) {
            continue;
        }

        merged_settings.insert(field_key.clone(), field_value.clone());
    }

    let mut merged_env = merged_settings
        .get("env")
        .and_then(value_as_object)
        .cloned()
        .unwrap_or_default();

    if let Some(previous_common_env) = previous_common_config_object
        .get("env")
        .and_then(value_as_object)
    {
        for field_key in previous_common_env.keys() {
            if !known_env_fields.contains(&field_key.as_str()) {
                merged_env.remove(field_key);
            }
        }
    }

    if let Some(next_common_env) = next_common_config_object
        .get("env")
        .and_then(value_as_object)
    {
        for (field_key, field_value) in next_common_env {
            merged_env.insert(field_key.clone(), field_value.clone());
        }
    }

    for known_env_field in known_env_fields {
        merged_env.remove(*known_env_field);
    }

    for (field_key, field_value) in build_provider_managed_env(provider_config, known_env_fields) {
        merged_env.insert(field_key, field_value);
    }

    if merged_env.is_empty() {
        merged_settings.remove("env");
    } else {
        merged_settings.insert("env".to_string(), Value::Object(merged_env));
    }

    Ok(Value::Object(merged_settings))
}

pub fn split_settings_into_provider_and_common(
    settings_value: &Value,
    known_env_fields: &[&str],
) -> Result<(Value, Value), String> {
    let settings_object = settings_value
        .as_object()
        .ok_or_else(|| "Claude settings must be a JSON object".to_string())?;

    let mut provider_env = Map::new();
    let mut common_env = Map::new();

    if let Some(env_object) = settings_object.get("env").and_then(value_as_object) {
        for (field_key, field_value) in env_object {
            if known_env_fields.contains(&field_key.as_str()) {
                provider_env.insert(field_key.clone(), field_value.clone());
            } else {
                common_env.insert(field_key.clone(), field_value.clone());
            }
        }
    }

    let mut provider_settings = Map::new();
    let mut provider_settings_env = Map::new();

    let api_key_value = provider_env
        .get("ANTHROPIC_AUTH_TOKEN")
        .or_else(|| provider_env.get("ANTHROPIC_API_KEY"));
    if let Some(api_key_value) = api_key_value {
        provider_settings_env.insert("ANTHROPIC_AUTH_TOKEN".to_string(), api_key_value.clone());
    }
    if let Some(base_url_value) = provider_env.get("ANTHROPIC_BASE_URL") {
        provider_settings_env.insert("ANTHROPIC_BASE_URL".to_string(), base_url_value.clone());
    }
    if !provider_settings_env.is_empty() {
        provider_settings.insert("env".to_string(), Value::Object(provider_settings_env));
    }

    for (provider_field, env_field) in PROVIDER_MODEL_FIELD_MAPPINGS {
        if let Some(field_value) = provider_env.get(env_field) {
            provider_settings.insert(provider_field.to_string(), field_value.clone());
        }
    }

    let mut common_settings = Map::new();
    for (field_key, field_value) in settings_object {
        if field_key == "env" {
            continue;
        }
        common_settings.insert(field_key.clone(), field_value.clone());
    }

    if !common_env.is_empty() {
        common_settings.insert("env".to_string(), Value::Object(common_env));
    }

    Ok((
        Value::Object(provider_settings),
        Value::Object(common_settings),
    ))
}
