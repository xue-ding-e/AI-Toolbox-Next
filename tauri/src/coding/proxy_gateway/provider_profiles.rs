use crate::coding::proxy_gateway::transformer::AiProtocol;
use crate::db::SqliteDbState;
use crate::http_client;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

const CACHE_FILE_NAME: &str = "gateway_provider_profiles.json";
const DEFAULT_GATEWAY_PROVIDER_PROFILES_JSON: &str =
    include_str!("../../../resources/gateway_provider_profiles.json");

static CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_cache_dir(dir: PathBuf) {
    let _ = CACHE_DIR.set(dir);
}

fn get_cache_file_path() -> Option<PathBuf> {
    CACHE_DIR.get().map(|dir| dir.join(CACHE_FILE_NAME))
}

pub fn get_gateway_provider_profiles_cache_path() -> Option<PathBuf> {
    get_cache_file_path()
}

fn get_bundled_gateway_provider_profiles() -> Option<Value> {
    let data: Value = serde_json::from_str(DEFAULT_GATEWAY_PROVIDER_PROFILES_JSON).ok()?;
    if is_valid_gateway_provider_profiles(&data) {
        Some(data)
    } else {
        None
    }
}

fn read_cache_file() -> Option<Value> {
    let path = get_cache_file_path()?;
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn write_cache_file(data: &Value) -> Result<(), String> {
    let path =
        get_cache_file_path().ok_or_else(|| "Cache directory not initialized".to_string())?;
    let tmp_path = path.with_extension("json.tmp");
    let json = serde_json::to_string(data)
        .map_err(|error| format!("Failed to serialize provider profiles cache: {error}"))?;

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("Failed to create provider profiles cache dir: {error}")
            })?;
        }
    }

    fs::write(&tmp_path, json)
        .map_err(|error| format!("Failed to write provider profiles cache tmp file: {error}"))?;
    fs::rename(&tmp_path, &path)
        .map_err(|error| format!("Failed to replace provider profiles cache file: {error}"))?;
    Ok(())
}

fn text_field_is_empty(object: &serde_json::Map<String, Value>, key: &str) -> bool {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .is_none_or(str::is_empty)
}

fn endpoint_has_valid_api_format(endpoint_object: &serde_json::Map<String, Value>) -> bool {
    endpoint_object
        .get("apiFormat")
        .and_then(Value::as_str)
        .and_then(AiProtocol::from_api_format)
        .is_some()
}

fn tool_has_valid_endpoints(tool_object: &serde_json::Map<String, Value>) -> bool {
    let Some(default_endpoint_id) = tool_object
        .get("defaultEndpointId")
        .and_then(Value::as_str)
        .map(str::trim)
    else {
        return false;
    };
    if default_endpoint_id.is_empty() {
        return false;
    }

    let Some(endpoints) = tool_object.get("endpoints").and_then(Value::as_array) else {
        return false;
    };
    if endpoints.is_empty() {
        return false;
    }

    let mut endpoint_ids = HashSet::new();
    for endpoint in endpoints {
        let Some(endpoint_object) = endpoint.as_object() else {
            return false;
        };
        let Some(endpoint_id) = endpoint_object
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
        else {
            return false;
        };
        if endpoint_id.is_empty() || !endpoint_ids.insert(endpoint_id.to_string()) {
            return false;
        }

        if text_field_is_empty(endpoint_object, "label")
            || text_field_is_empty(endpoint_object, "baseUrl")
            || !endpoint_has_valid_api_format(endpoint_object)
        {
            return false;
        }
    }

    endpoint_ids.contains(default_endpoint_id)
}

fn profile_has_valid_tool(tools: Option<&Value>) -> bool {
    let Some(tools_object) = tools.and_then(Value::as_object) else {
        return false;
    };

    let mut has_supported_tool = false;
    for tool_key in ["claude", "codex"] {
        let Some(tool_value) = tools_object.get(tool_key) else {
            continue;
        };
        let Some(tool_object) = tool_value.as_object() else {
            return false;
        };
        if !tool_has_valid_endpoints(tool_object) {
            return false;
        }
        has_supported_tool = true;
    }

    has_supported_tool
}

pub(crate) fn is_valid_gateway_provider_profiles(data: &Value) -> bool {
    let Some(object) = data.as_object() else {
        return false;
    };
    if object
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .is_none_or(|version| version != 1)
    {
        return false;
    }
    let Some(profiles) = object.get("profiles").and_then(Value::as_array) else {
        return false;
    };
    if profiles.is_empty() {
        return false;
    }

    let mut seen_ids = HashSet::new();
    for profile in profiles {
        let Some(profile_object) = profile.as_object() else {
            return false;
        };
        let Some(id) = profile_object
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
        else {
            return false;
        };
        if id.is_empty() || !seen_ids.insert(id.to_string()) {
            return false;
        }
        if text_field_is_empty(profile_object, "providerType")
            || text_field_is_empty(profile_object, "label")
            || !profile_has_valid_tool(profile_object.get("tools"))
        {
            return false;
        }
    }

    true
}

#[tauri::command]
pub fn load_cached_gateway_provider_profiles() -> Result<Option<Value>, String> {
    if let Some(data) = read_cache_file() {
        if is_valid_gateway_provider_profiles(&data) {
            return Ok(Some(data));
        }
    }
    Ok(get_bundled_gateway_provider_profiles())
}

#[tauri::command]
pub async fn fetch_remote_gateway_provider_profiles(
    state: tauri::State<'_, SqliteDbState>,
    url: String,
) -> Result<Value, String> {
    let client = http_client::client_with_timeout(&state, 30).await?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|error| format!("Failed to fetch remote provider profiles: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Remote provider profiles request failed: {}",
            response.status()
        ));
    }

    let json: Value = response
        .json()
        .await
        .map_err(|error| format!("Failed to parse remote provider profiles JSON: {error}"))?;

    if !is_valid_gateway_provider_profiles(&json) {
        return Err("Remote provider profiles JSON is invalid".to_string());
    }

    if let Err(error) = write_cache_file(&json) {
        log::warn!("[GatewayProviderProfiles] Failed to write cache: {error}");
    } else {
        log::info!("[GatewayProviderProfiles] Cache updated from remote");
    }

    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn valid_catalog() -> Value {
        json!({
            "schemaVersion": 1,
            "profiles": [
                {
                    "id": "deepseek",
                    "providerType": "deepseek",
                    "label": "DeepSeek",
                    "tools": {
                        "claude": {
                            "defaultEndpointId": "anthropic",
                            "endpoints": [
                                {
                                    "id": "anthropic",
                                    "label": "Anthropic",
                                    "apiFormat": "anthropic",
                                    "baseUrl": "https://api.deepseek.com/anthropic"
                                }
                            ]
                        }
                    }
                }
            ]
        })
    }

    #[test]
    fn bundled_gateway_provider_profiles_are_valid() {
        let bundled = get_bundled_gateway_provider_profiles();
        assert!(bundled.is_some());
    }

    #[test]
    fn empty_profiles_are_rejected() {
        assert!(!is_valid_gateway_provider_profiles(&json!({
            "schemaVersion": 1,
            "profiles": []
        })));
    }

    #[test]
    fn duplicate_profile_ids_are_rejected() {
        let mut catalog = valid_catalog();
        let duplicate = catalog["profiles"][0].clone();
        catalog["profiles"].as_array_mut().unwrap().push(duplicate);
        assert!(!is_valid_gateway_provider_profiles(&catalog));
    }

    #[test]
    fn missing_provider_type_is_rejected() {
        let mut catalog = valid_catalog();
        catalog["profiles"][0]
            .as_object_mut()
            .unwrap()
            .remove("providerType");
        assert!(!is_valid_gateway_provider_profiles(&catalog));
    }

    #[test]
    fn missing_tool_endpoints_are_rejected() {
        let mut catalog = valid_catalog();
        catalog["profiles"][0]["tools"]["claude"]
            .as_object_mut()
            .unwrap()
            .remove("endpoints");
        assert!(!is_valid_gateway_provider_profiles(&catalog));
    }

    #[test]
    fn invalid_endpoint_api_format_is_rejected() {
        let mut catalog = valid_catalog();
        catalog["profiles"][0]["tools"]["claude"]["endpoints"][0]["apiFormat"] =
            json!("unknown_format");
        assert!(!is_valid_gateway_provider_profiles(&catalog));
    }

    #[test]
    fn default_endpoint_must_exist() {
        let mut catalog = valid_catalog();
        catalog["profiles"][0]["tools"]["claude"]["defaultEndpointId"] = json!("missing");
        assert!(!is_valid_gateway_provider_profiles(&catalog));
    }

    #[test]
    fn valid_cache_is_loaded_before_bundled_defaults() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        set_cache_dir(temp_dir.path().to_path_buf());
        let mut catalog = valid_catalog();
        catalog["profiles"][0]["label"] = json!("Cached DeepSeek");
        write_cache_file(&catalog).expect("write cache");

        let loaded = load_cached_gateway_provider_profiles()
            .expect("load")
            .expect("catalog");
        assert_eq!(loaded["profiles"][0]["label"], "Cached DeepSeek");
    }
}
