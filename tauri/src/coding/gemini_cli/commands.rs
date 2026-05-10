use chrono::Local;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use super::adapter;
use super::types::*;
use crate::coding::db_id::{db_new_id, db_record_id};
use crate::coding::open_code::shell_env;
use crate::coding::prompt_file::{read_prompt_content_file, write_prompt_content_file};
use crate::coding::runtime_location;
use crate::db::DbState;
use crate::http_client;
use tauri::Emitter;

const MANAGED_ENV_KEYS: [&str; 14] = [
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "GOOGLE_GEMINI_BASE_URL",
    "GOOGLE_VERTEX_BASE_URL",
    "GOOGLE_GENAI_USE_GCA",
    "GOOGLE_GENAI_USE_VERTEXAI",
    "GEMINI_CLI_USE_COMPUTE_ADC",
    "GEMINI_CLI_CUSTOM_HEADERS",
    "GEMINI_MODEL",
    "GEMINI_API_KEY_AUTH_MECHANISM",
    "GOOGLE_GENAI_API_VERSION",
    "GOOGLE_CLOUD_PROJECT",
    "GOOGLE_CLOUD_PROJECT_ID",
    "GOOGLE_CLOUD_LOCATION",
];

const OFFICIAL_PROVIDER_REMOVED_ENV_KEYS: [&str; 13] = [
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "GOOGLE_GEMINI_BASE_URL",
    "GOOGLE_VERTEX_BASE_URL",
    "GOOGLE_GENAI_USE_GCA",
    "GOOGLE_GENAI_USE_VERTEXAI",
    "GEMINI_CLI_USE_COMPUTE_ADC",
    "GEMINI_CLI_CUSTOM_HEADERS",
    "GEMINI_API_KEY_AUTH_MECHANISM",
    "GOOGLE_GENAI_API_VERSION",
    "GOOGLE_CLOUD_PROJECT",
    "GOOGLE_CLOUD_PROJECT_ID",
    "GOOGLE_CLOUD_LOCATION",
];

const GEMINI_CLI_OFFICIAL_AUTH_TYPE: &str = "oauth-personal";
const GEMINI_CLI_CUSTOM_AUTH_TYPE: &str = "gemini-api-key";
pub const GEMINI_CLI_HOME_ENV_KEY: &str = "GEMINI_CLI_HOME";
pub const DEFAULT_GEMINI_CLI_PROMPT_FILE: &str = "GEMINI.md";

const GEMINI_CLI_MODEL_CATALOG_URLS: [&str; 2] = [
    "https://raw.githubusercontent.com/router-for-me/models/refs/heads/main/models.json",
    "https://models.router-for.me/models.json",
];

const GEMINI_CLI_ALIAS_MODEL_IDS: [&str; 6] = [
    "auto",
    "auto-gemini-3",
    "auto-gemini-2.5",
    "pro",
    "flash",
    "flash-lite",
];

const GEMINI_CLI_SOURCE_MODEL_IDS: [&str; 10] = [
    "gemini-3.1-flash-lite-preview",
    "gemini-3.1-pro-preview",
    "gemini-3.1-pro-preview-customtools",
    "gemini-3-pro-preview",
    "gemini-3-flash-preview",
    "gemini-2.5-pro",
    "gemini-2.5-flash",
    "gemini-2.5-flash-lite",
    "gemma-4-31b-it",
    "gemma-4-26b-a4b-it",
];

#[derive(Debug, Deserialize)]
struct RemoteGeminiCliModelCatalog {
    #[serde(default, rename = "gemini-cli")]
    gemini_cli: Vec<RemoteGeminiCliModel>,
}

#[derive(Debug, Deserialize)]
struct RemoteGeminiCliModel {
    id: String,
    #[serde(default, alias = "displayName")]
    display_name: Option<String>,
    #[serde(default, alias = "ownedBy")]
    owned_by: Option<String>,
    #[serde(default)]
    created: Option<i64>,
}

fn get_home_dir() -> Result<PathBuf, String> {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| "Failed to get home directory".to_string())
}

pub fn get_gemini_cli_default_root_dir() -> Result<PathBuf, String> {
    Ok(get_home_dir()?.join(".gemini"))
}

pub fn get_gemini_cli_root_dir_from_home_override(home_dir: &str) -> Option<PathBuf> {
    let trimmed = home_dir.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed).join(".gemini"))
    }
}

pub fn get_gemini_cli_root_dir_from_env() -> Option<PathBuf> {
    std::env::var(GEMINI_CLI_HOME_ENV_KEY)
        .ok()
        .and_then(|home_dir| get_gemini_cli_root_dir_from_home_override(&home_dir))
}

pub(crate) fn get_gemini_cli_root_dir_without_db() -> Result<PathBuf, String> {
    if let Some(env_root_dir) = get_gemini_cli_root_dir_from_env() {
        return Ok(env_root_dir);
    }
    if let Some(shell_root_dir) = shell_env::get_env_from_shell_config(GEMINI_CLI_HOME_ENV_KEY)
        .and_then(|home_dir| get_gemini_cli_root_dir_from_home_override(&home_dir))
    {
        return Ok(shell_root_dir);
    }
    get_gemini_cli_default_root_dir()
}

pub fn get_gemini_cli_root_dir_from_db(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(runtime_location::get_gemini_cli_runtime_location_sync(db)?.host_path)
}

async fn get_gemini_cli_root_dir_from_db_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(runtime_location::get_gemini_cli_runtime_location_async(db)
        .await?
        .host_path)
}

pub fn get_gemini_cli_root_path_info_from_db(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<ConfigPathInfo, String> {
    let location = runtime_location::get_gemini_cli_runtime_location_sync(db)?;
    Ok(ConfigPathInfo {
        path: location.host_path.to_string_lossy().to_string(),
        source: location.source,
    })
}

async fn get_gemini_cli_root_path_info_from_db_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<ConfigPathInfo, String> {
    let location = runtime_location::get_gemini_cli_runtime_location_async(db).await?;
    Ok(ConfigPathInfo {
        path: location.host_path.to_string_lossy().to_string(),
        source: location.source,
    })
}

pub(crate) async fn get_gemini_cli_custom_root_dir_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Option<PathBuf> {
    let mut result = db
        .query("SELECT * OMIT id FROM gemini_cli_common_config:`common` LIMIT 1")
        .await
        .ok()?;
    let records: Vec<Value> = result.take(0).ok()?;
    let record = records.into_iter().next()?;
    let config = adapter::from_db_value_common(record);
    config
        .root_dir
        .filter(|dir| !dir.trim().is_empty())
        .map(PathBuf::from)
}

fn get_gemini_cli_env_path_from_root(root_dir: &Path) -> PathBuf {
    root_dir.join(".env")
}

fn get_gemini_cli_settings_path_from_root(root_dir: &Path) -> PathBuf {
    root_dir.join("settings.json")
}

fn normalize_gemini_cli_prompt_file_name(file_name: &str) -> Option<String> {
    let trimmed = file_name.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut components = Path::new(trimmed).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(name)), None) => name
            .to_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        _ => None,
    }
}

pub(crate) fn get_gemini_cli_prompt_file_name_from_settings_value(
    settings: &Value,
) -> Option<String> {
    match settings.pointer("/context/fileName") {
        Some(Value::String(file_name)) => normalize_gemini_cli_prompt_file_name(file_name),
        Some(Value::Array(file_names)) => file_names.iter().find_map(|value| {
            value
                .as_str()
                .and_then(normalize_gemini_cli_prompt_file_name)
        }),
        _ => None,
    }
}

pub fn get_gemini_cli_prompt_file_name_from_root(root_dir: &Path) -> String {
    let settings_path = get_gemini_cli_settings_path_from_root(root_dir);
    let prompt_file_name = settings_path
        .exists()
        .then(|| fs::read_to_string(&settings_path).ok())
        .flatten()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|settings| get_gemini_cli_prompt_file_name_from_settings_value(&settings));

    prompt_file_name.unwrap_or_else(|| DEFAULT_GEMINI_CLI_PROMPT_FILE.to_string())
}

pub fn get_gemini_cli_prompt_path_from_root(root_dir: &Path) -> PathBuf {
    root_dir.join(get_gemini_cli_prompt_file_name_from_root(root_dir))
}

pub fn get_gemini_cli_tmp_dir_from_root(root_dir: &Path) -> PathBuf {
    root_dir.join("tmp")
}

pub fn get_gemini_cli_oauth_creds_path_from_root(root_dir: &Path) -> PathBuf {
    root_dir.join("oauth_creds.json")
}

pub async fn get_gemini_cli_env_path_from_db_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_gemini_cli_env_path_from_root(
        &get_gemini_cli_root_dir_from_db_async(db).await?,
    ))
}

pub async fn get_gemini_cli_settings_path_from_db_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_gemini_cli_settings_path_from_root(
        &get_gemini_cli_root_dir_from_db_async(db).await?,
    ))
}

pub async fn get_gemini_cli_prompt_path_from_db_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_gemini_cli_prompt_path_from_root(
        &get_gemini_cli_root_dir_from_db_async(db).await?,
    ))
}

pub fn get_gemini_cli_env_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_gemini_cli_env_path_from_root(
        &get_gemini_cli_root_dir_from_db(db)?,
    ))
}

pub fn get_gemini_cli_settings_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_gemini_cli_settings_path_from_root(
        &get_gemini_cli_root_dir_from_db(db)?,
    ))
}

pub fn get_gemini_cli_prompt_path_sync(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(get_gemini_cli_prompt_path_from_root(
        &get_gemini_cli_root_dir_from_db(db)?,
    ))
}

fn parse_env_line_key(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let candidate = trimmed.strip_prefix("export ").unwrap_or(trimmed);
    let (key, _) = candidate.split_once('=')?;
    let key = key.trim();
    if key.is_empty() {
        None
    } else {
        Some(key.to_string())
    }
}

fn parse_env_content(content: &str) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let candidate = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let Some((key, raw_value)) = candidate.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let value = raw_value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        result.insert(key.to_string(), value);
    }
    result
}

fn serialize_env_value(value: &str) -> String {
    if value
        .chars()
        .any(|ch| ch.is_whitespace() || matches!(ch, '"' | '\'' | '#' | '='))
    {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

fn merge_env_content(existing_content: &str, provider_env: &BTreeMap<String, String>) -> String {
    let managed_keys: BTreeSet<&str> = MANAGED_ENV_KEYS.into_iter().collect();
    let mut lines: Vec<String> = existing_content
        .lines()
        .filter(|line| {
            parse_env_line_key(line)
                .map(|key| !managed_keys.contains(key.as_str()))
                .unwrap_or(true)
        })
        .map(str::to_string)
        .collect();

    if !lines.is_empty()
        && !lines
            .last()
            .map(|line| line.trim().is_empty())
            .unwrap_or(false)
    {
        lines.push(String::new());
    }

    for (key, value) in provider_env {
        if managed_keys.contains(key.as_str()) && !value.trim().is_empty() {
            lines.push(format!("{}={}", key, serialize_env_value(value.trim())));
        }
    }

    while lines
        .last()
        .map(|line| line.trim().is_empty())
        .unwrap_or(false)
    {
        lines.pop();
    }

    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

fn parse_provider_settings_config(settings_config: &str) -> Result<Value, String> {
    let parsed: Value = serde_json::from_str(settings_config)
        .map_err(|error| format!("Invalid Gemini CLI provider settings JSON: {}", error))?;
    if !parsed.is_object() {
        return Err("Gemini CLI provider settings must be a JSON object".to_string());
    }
    Ok(parsed)
}

fn ensure_json_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value
        .as_object_mut()
        .expect("value was normalized to object")
}

fn set_selected_auth_type(settings_config: &mut Value, selected_type: &str) {
    let root = ensure_json_object(settings_config);
    let config = root
        .entry("config")
        .or_insert_with(|| Value::Object(Map::new()));
    let config = ensure_json_object(config);
    let security = config
        .entry("security")
        .or_insert_with(|| Value::Object(Map::new()));
    let security = ensure_json_object(security);
    let auth = security
        .entry("auth")
        .or_insert_with(|| Value::Object(Map::new()));
    let auth = ensure_json_object(auth);
    auth.insert(
        "selectedType".to_string(),
        Value::String(selected_type.to_string()),
    );
}

fn normalize_official_provider_settings(settings_config: &mut Value) {
    let root = ensure_json_object(settings_config);
    if let Some(env) = root.get_mut("env").and_then(Value::as_object_mut) {
        for key in OFFICIAL_PROVIDER_REMOVED_ENV_KEYS {
            env.remove(key);
        }
    }

    set_selected_auth_type(settings_config, GEMINI_CLI_OFFICIAL_AUTH_TYPE);
}

fn normalize_custom_provider_settings(settings_config: &mut Value) {
    set_selected_auth_type(settings_config, GEMINI_CLI_CUSTOM_AUTH_TYPE);
}

fn normalize_provider_settings_for_category(settings_config: &mut Value, category: &str) {
    if category == "official" {
        normalize_official_provider_settings(settings_config);
    } else {
        normalize_custom_provider_settings(settings_config);
    }
}

fn normalize_provider_settings_for_storage(
    settings_config: &str,
    category: &str,
) -> Result<String, String> {
    let mut parsed = parse_provider_settings_config(settings_config)?;
    normalize_provider_settings_for_category(&mut parsed, category);
    serde_json::to_string(&parsed)
        .map_err(|error| format!("Failed to serialize provider settings: {}", error))
}

fn extract_env_object(settings_config: &Value) -> BTreeMap<String, String> {
    settings_config
        .get("env")
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn extract_config_object(settings_config: &Value) -> Value {
    settings_config
        .get("config")
        .filter(|value| value.is_object())
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()))
}

fn merge_json_value(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(base_map), Value::Object(patch_map)) => {
            for (key, patch_value) in patch_map {
                match base_map.get_mut(key) {
                    Some(base_value) => merge_json_value(base_value, patch_value),
                    None => {
                        base_map.insert(key.clone(), patch_value.clone());
                    }
                }
            }
        }
        (base_value, patch_value) => {
            *base_value = patch_value.clone();
        }
    }
}

fn remove_selected_auth_type(settings: &mut Value) {
    if let Some(auth) = settings
        .get_mut("security")
        .and_then(Value::as_object_mut)
        .and_then(|security| security.get_mut("auth"))
        .and_then(Value::as_object_mut)
    {
        auth.remove("selectedType");
    }
}

fn settings_value_without_managed_auth(settings: &Value) -> Value {
    let mut value = settings.clone();
    remove_selected_auth_type(&mut value);
    value
}

async fn read_env_map_from_db_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<BTreeMap<String, String>, String> {
    let env_path = get_gemini_cli_env_path_from_db_async(db).await?;
    if !env_path.exists() {
        return Ok(BTreeMap::new());
    }
    let content = fs::read_to_string(&env_path)
        .map_err(|error| format!("Failed to read Gemini CLI .env: {}", error))?;
    Ok(parse_env_content(&content))
}

async fn read_settings_value_from_db_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<Option<Value>, String> {
    let settings_path = get_gemini_cli_settings_path_from_db_async(db).await?;
    if !settings_path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&settings_path)
        .map_err(|error| format!("Failed to read Gemini CLI settings.json: {}", error))?;
    let parsed = serde_json::from_str::<Value>(&content)
        .map_err(|error| format!("Failed to parse Gemini CLI settings.json: {}", error))?;
    Ok(Some(parsed))
}

async fn write_settings_value_to_db_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    value: &Value,
) -> Result<(), String> {
    let settings_path = get_gemini_cli_settings_path_from_db_async(db).await?;
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create Gemini CLI directory: {}", error))?;
    }
    let serialized = serde_json::to_string_pretty(value)
        .map_err(|error| format!("Failed to serialize Gemini CLI settings: {}", error))?;
    fs::write(&settings_path, format!("{serialized}\n"))
        .map_err(|error| format!("Failed to write Gemini CLI settings.json: {}", error))
}

async fn write_env_to_db_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    provider_env: &BTreeMap<String, String>,
) -> Result<(), String> {
    let env_path = get_gemini_cli_env_path_from_db_async(db).await?;
    let existing_content = if env_path.exists() {
        fs::read_to_string(&env_path)
            .map_err(|error| format!("Failed to read Gemini CLI .env: {}", error))?
    } else {
        String::new()
    };
    let merged = merge_env_content(&existing_content, provider_env);
    if let Some(parent) = env_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create Gemini CLI directory: {}", error))?;
    }
    fs::write(&env_path, merged)
        .map_err(|error| format!("Failed to write Gemini CLI .env: {}", error))
}

async fn load_stored_common_config_value(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<Option<Value>, String> {
    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT * OMIT id FROM gemini_cli_common_config:`common` LIMIT 1")
        .await
        .map_err(|error| format!("Failed to query Gemini CLI common config: {}", error))?
        .take(0);
    let records = records_result.map_err(|error| {
        format!(
            "Failed to deserialize Gemini CLI common config records: {}",
            error
        )
    })?;
    let Some(record) = records.first() else {
        return Ok(None);
    };
    let common = adapter::from_db_value_common(record.clone());
    if common.config.trim().is_empty() {
        return Ok(None);
    }
    let parsed = serde_json::from_str::<Value>(&common.config)
        .map_err(|error| format!("Failed to parse Gemini CLI common config: {}", error))?;
    Ok(Some(parsed))
}

async fn load_temp_provider_from_files_with_db(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<GeminiCliProvider, String> {
    let env = read_env_map_from_db_async(db).await?;
    let managed_env: BTreeMap<String, String> = env
        .into_iter()
        .filter(|(key, _)| MANAGED_ENV_KEYS.contains(&key.as_str()))
        .collect();
    let settings = read_settings_value_from_db_async(db).await?;
    let selected_auth_config = settings
        .as_ref()
        .and_then(|value| value.pointer("/security/auth/selectedType"))
        .and_then(Value::as_str)
        .map(|selected_type| {
            serde_json::json!({
                "security": {
                    "auth": {
                        "selectedType": selected_type
                    }
                }
            })
        })
        .unwrap_or_else(|| Value::Object(Map::new()));

    if managed_env.is_empty()
        && selected_auth_config
            .as_object()
            .map(|m| m.is_empty())
            .unwrap_or(true)
    {
        return Err("No Gemini CLI local provider config found".to_string());
    }

    let settings_config = serde_json::json!({
        "env": managed_env,
        "config": selected_auth_config,
    });

    let now = Local::now().to_rfc3339();
    Ok(GeminiCliProvider {
        id: "__local__".to_string(),
        name: "default".to_string(),
        category: infer_gemini_cli_provider_category_from_settings(&settings_config),
        settings_config: serde_json::to_string(&settings_config)
            .map_err(|error| format!("Failed to serialize Gemini CLI provider: {}", error))?,
        source_provider_id: None,
        website_url: None,
        notes: None,
        icon: None,
        icon_color: None,
        sort_index: Some(0),
        is_applied: true,
        is_disabled: false,
        created_at: now.clone(),
        updated_at: now,
    })
}

async fn load_temp_common_config_from_file_with_db(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<GeminiCliCommonConfig, String> {
    let settings = read_settings_value_from_db_async(db)
        .await?
        .unwrap_or_else(|| Value::Object(Map::new()));
    let common_settings = settings_value_without_managed_auth(&settings);
    let now = Local::now().to_rfc3339();
    Ok(GeminiCliCommonConfig {
        config: serde_json::to_string_pretty(&common_settings)
            .map_err(|error| format!("Failed to serialize Gemini CLI common config: {}", error))?,
        root_dir: None,
        updated_at: now,
    })
}

pub fn infer_gemini_cli_provider_category_from_settings(provider_settings: &Value) -> String {
    let env = provider_settings.get("env").and_then(Value::as_object);
    let has_custom_auth_env = env
        .map(|env| {
            OFFICIAL_PROVIDER_REMOVED_ENV_KEYS.iter().any(|key| {
                env.get(*key)
                    .and_then(Value::as_str)
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    let selected_auth_type = provider_settings
        .pointer("/config/security/auth/selectedType")
        .and_then(Value::as_str)
        .unwrap_or_default();

    if has_custom_auth_env || selected_auth_type == "gemini-api-key" {
        "custom".to_string()
    } else {
        "official".to_string()
    }
}

fn emit_sync_requests<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    #[cfg(target_os = "windows")]
    let _ = app.emit("wsl-sync-request-geminicli", ());
}

async fn write_prompt_content_to_file(
    db: Option<&surrealdb::Surreal<surrealdb::engine::local::Db>>,
    prompt_content: Option<&str>,
) -> Result<(), String> {
    let prompt_path = if let Some(db) = db {
        get_gemini_cli_prompt_path_from_db_async(db).await?
    } else {
        get_gemini_cli_prompt_path_from_root(&get_gemini_cli_root_dir_without_db()?)
    };
    write_prompt_content_file(&prompt_path, prompt_content, "Gemini CLI")
}

async fn rewrite_applied_prompt_to_current_file(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<bool, String> {
    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT content FROM gemini_cli_prompt_config WHERE is_applied = true LIMIT 1")
        .await
        .map_err(|error| {
            format!(
                "Failed to query applied Gemini CLI prompt config: {}",
                error
            )
        })?
        .take(0);
    let Some(content) = records_result
        .map_err(|error| format!("Failed to deserialize Gemini CLI prompt config: {}", error))?
        .into_iter()
        .next()
        .and_then(|record| {
            record
                .get("content")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
    else {
        return Ok(false);
    };

    write_prompt_content_to_file(Some(db), Some(&content)).await?;
    Ok(true)
}

async fn get_local_prompt_config(
    db: Option<&surrealdb::Surreal<surrealdb::engine::local::Db>>,
) -> Result<Option<GeminiCliPromptConfig>, String> {
    let prompt_path = if let Some(db) = db {
        get_gemini_cli_prompt_path_from_db_async(db).await?
    } else {
        get_gemini_cli_prompt_path_from_root(&get_gemini_cli_root_dir_without_db()?)
    };
    let Some(prompt_content) = read_prompt_content_file(&prompt_path, "Gemini CLI")? else {
        return Ok(None);
    };
    let now = Local::now().to_rfc3339();
    Ok(Some(GeminiCliPromptConfig {
        id: "__local__".to_string(),
        name: "default".to_string(),
        content: prompt_content,
        is_applied: true,
        sort_index: None,
        created_at: Some(now.clone()),
        updated_at: Some(now),
    }))
}

#[tauri::command]
pub async fn get_gemini_cli_config_path(
    state: tauri::State<'_, DbState>,
) -> Result<String, String> {
    let db = state.db();
    Ok(get_gemini_cli_settings_path_from_db_async(&db)
        .await?
        .to_string_lossy()
        .to_string())
}

#[tauri::command]
pub async fn get_gemini_cli_root_path_info(
    state: tauri::State<'_, DbState>,
) -> Result<ConfigPathInfo, String> {
    let db = state.db();
    get_gemini_cli_root_path_info_from_db_async(&db).await
}

#[tauri::command]
pub async fn reveal_gemini_cli_config_folder(
    state: tauri::State<'_, DbState>,
) -> Result<(), String> {
    let db = state.db();
    let config_dir = get_gemini_cli_root_dir_from_db_async(&db).await?;
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|error| format!("Failed to create Gemini CLI directory: {}", error))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(config_dir)
            .spawn()
            .map_err(|error| format!("Failed to open folder: {}", error))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(config_dir)
            .spawn()
            .map_err(|error| format!("Failed to open folder: {}", error))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(config_dir)
            .spawn()
            .map_err(|error| format!("Failed to open folder: {}", error))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn read_gemini_cli_settings(
    state: tauri::State<'_, DbState>,
) -> Result<GeminiCliSettings, String> {
    let db = state.db();
    Ok(GeminiCliSettings {
        env: Some(read_env_map_from_db_async(&db).await?),
        config: read_settings_value_from_db_async(&db).await?,
    })
}

fn push_gemini_cli_official_model(
    models: &mut Vec<GeminiCliOfficialModel>,
    seen_model_ids: &mut BTreeSet<String>,
    model: GeminiCliOfficialModel,
) {
    let model_id = model.id.trim();
    if model_id.is_empty() {
        return;
    }

    let model_id_key = model_id.to_lowercase();
    if seen_model_ids.contains(&model_id_key) {
        return;
    }

    seen_model_ids.insert(model_id_key);
    models.push(GeminiCliOfficialModel {
        id: model_id.to_string(),
        name: model
            .name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty()),
        owned_by: model
            .owned_by
            .map(|owned_by| owned_by.trim().to_string())
            .filter(|owned_by| !owned_by.is_empty()),
        created: model.created,
    });
}

fn static_gemini_cli_official_models() -> Vec<GeminiCliOfficialModel> {
    let mut models = Vec::new();
    let mut seen_model_ids = BTreeSet::new();

    for model_id in GEMINI_CLI_ALIAS_MODEL_IDS
        .iter()
        .chain(GEMINI_CLI_SOURCE_MODEL_IDS.iter())
    {
        push_gemini_cli_official_model(
            &mut models,
            &mut seen_model_ids,
            GeminiCliOfficialModel {
                id: (*model_id).to_string(),
                name: Some((*model_id).to_string()),
                owned_by: Some("google".to_string()),
                created: None,
            },
        );
    }

    models
}

fn merge_remote_gemini_cli_official_models(
    remote_models: Vec<RemoteGeminiCliModel>,
) -> Vec<GeminiCliOfficialModel> {
    let mut models = Vec::new();
    let mut seen_model_ids = BTreeSet::new();

    for model_id in GEMINI_CLI_ALIAS_MODEL_IDS {
        push_gemini_cli_official_model(
            &mut models,
            &mut seen_model_ids,
            GeminiCliOfficialModel {
                id: model_id.to_string(),
                name: Some(model_id.to_string()),
                owned_by: Some("google".to_string()),
                created: None,
            },
        );
    }

    for remote_model in remote_models {
        push_gemini_cli_official_model(
            &mut models,
            &mut seen_model_ids,
            GeminiCliOfficialModel {
                id: remote_model.id,
                name: remote_model.display_name,
                owned_by: remote_model.owned_by.or_else(|| Some("google".to_string())),
                created: remote_model.created,
            },
        );
    }

    for model_id in GEMINI_CLI_SOURCE_MODEL_IDS {
        push_gemini_cli_official_model(
            &mut models,
            &mut seen_model_ids,
            GeminiCliOfficialModel {
                id: model_id.to_string(),
                name: Some(model_id.to_string()),
                owned_by: Some("google".to_string()),
                created: None,
            },
        );
    }

    models
}

async fn fetch_remote_gemini_cli_model_catalog(
    client: &reqwest::Client,
    url: &str,
) -> Result<Vec<RemoteGeminiCliModel>, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("request failed: {}", error))?;

    if !response.status().is_success() {
        return Err(format!("request failed with status {}", response.status()));
    }

    let catalog = response
        .json::<RemoteGeminiCliModelCatalog>()
        .await
        .map_err(|error| format!("failed to parse model catalog: {}", error))?;

    if catalog.gemini_cli.is_empty() {
        return Err("gemini-cli model catalog is empty".to_string());
    }

    Ok(catalog.gemini_cli)
}

#[tauri::command]
pub async fn fetch_gemini_cli_official_models(
    state: tauri::State<'_, DbState>,
) -> Result<GeminiCliOfficialModelsResponse, String> {
    if let Ok(client) = http_client::client_with_timeout(&state, 30).await {
        for url in GEMINI_CLI_MODEL_CATALOG_URLS {
            match fetch_remote_gemini_cli_model_catalog(&client, url).await {
                Ok(remote_models) => {
                    let models = merge_remote_gemini_cli_official_models(remote_models);
                    let total = models.len();
                    return Ok(GeminiCliOfficialModelsResponse {
                        models,
                        total,
                        source: "remote".to_string(),
                    });
                }
                Err(error) => {
                    log::warn!(
                        "[GeminiCLI] Failed to fetch official model catalog from {}: {}",
                        url,
                        error
                    );
                }
            }
        }
    } else {
        log::warn!("[GeminiCLI] Failed to create HTTP client for official model catalog");
    }

    let models = static_gemini_cli_official_models();
    let total = models.len();
    Ok(GeminiCliOfficialModelsResponse {
        models,
        total,
        source: "bundled".to_string(),
    })
}

#[tauri::command]
pub async fn list_gemini_cli_providers(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<GeminiCliProvider>, String> {
    let db = state.db();
    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM gemini_cli_provider")
        .await
        .map_err(|error| format!("Failed to query Gemini CLI providers: {}", error))?
        .take(0);

    match records_result {
        Ok(records) => {
            if records.is_empty() {
                if let Ok(temp_provider) = load_temp_provider_from_files_with_db(&db).await {
                    return Ok(vec![temp_provider]);
                }
                return Ok(Vec::new());
            }
            let mut result: Vec<GeminiCliProvider> = records
                .into_iter()
                .map(adapter::from_db_value_provider)
                .collect();
            result.sort_by_key(|provider| provider.sort_index.unwrap_or(0));
            Ok(result)
        }
        Err(error) => {
            eprintln!("Failed to deserialize Gemini CLI providers: {}", error);
            if let Ok(temp_provider) = load_temp_provider_from_files_with_db(&db).await {
                return Ok(vec![temp_provider]);
            }
            Ok(Vec::new())
        }
    }
}

#[tauri::command]
pub async fn create_gemini_cli_provider(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    provider: GeminiCliProviderInput,
) -> Result<GeminiCliProvider, String> {
    let db = state.db();
    let normalized_settings_config =
        normalize_provider_settings_for_storage(&provider.settings_config, &provider.category)?;
    let now = Local::now().to_rfc3339();
    let content = GeminiCliProviderContent {
        name: provider.name,
        category: provider.category,
        settings_config: normalized_settings_config,
        source_provider_id: provider.source_provider_id,
        website_url: provider.website_url,
        notes: provider.notes,
        icon: provider.icon,
        icon_color: provider.icon_color,
        sort_index: provider.sort_index,
        is_applied: false,
        is_disabled: provider.is_disabled.unwrap_or(false),
        created_at: now.clone(),
        updated_at: now,
    };

    db.query("CREATE gemini_cli_provider CONTENT $data")
        .bind(("data", adapter::to_db_value_provider(&content)))
        .await
        .map_err(|error| format!("Failed to create Gemini CLI provider: {}", error))?;

    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM gemini_cli_provider ORDER BY created_at DESC LIMIT 1")
        .await
        .map_err(|error| format!("Failed to fetch created Gemini CLI provider: {}", error))?
        .take(0);
    let _ = app.emit("config-changed", "window");

    records_result
        .ok()
        .and_then(|records| records.first().cloned())
        .map(adapter::from_db_value_provider)
        .ok_or_else(|| "Failed to retrieve created Gemini CLI provider".to_string())
}

#[tauri::command]
pub async fn update_gemini_cli_provider(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    provider: GeminiCliProvider,
) -> Result<GeminiCliProvider, String> {
    let db = state.db();
    let normalized_settings_config =
        normalize_provider_settings_for_storage(&provider.settings_config, &provider.category)?;
    let id = provider.id.clone();
    let now = Local::now().to_rfc3339();
    let record_id = db_record_id("gemini_cli_provider", &id);
    let existing_result: Result<Vec<Value>, _> = db
        .query(&format!("SELECT * OMIT id FROM {} LIMIT 1", record_id))
        .await
        .map_err(|error| format!("Failed to query Gemini CLI provider: {}", error))?
        .take(0);

    let existing_record = existing_result
        .map_err(|error| format!("Failed to deserialize Gemini CLI provider: {}", error))?
        .into_iter()
        .next()
        .ok_or_else(|| format!("Gemini CLI provider with ID '{}' not found", id))?;

    let existing_category = existing_record
        .get("category")
        .and_then(Value::as_str)
        .unwrap_or("custom")
        .to_string();
    if existing_category == "official" && provider.category != "official" {
        super::official_accounts::ensure_gemini_cli_provider_has_no_official_accounts(&db, &id)
            .await?;
    }

    let created_at = existing_record
        .get("created_at")
        .and_then(Value::as_str)
        .unwrap_or(&provider.created_at)
        .to_string();
    let is_disabled = existing_record
        .get("is_disabled")
        .or_else(|| existing_record.get("isDisabled"))
        .and_then(Value::as_bool)
        .unwrap_or(provider.is_disabled);

    let content = GeminiCliProviderContent {
        name: provider.name,
        category: provider.category,
        settings_config: normalized_settings_config,
        source_provider_id: provider.source_provider_id,
        website_url: provider.website_url,
        notes: provider.notes,
        icon: provider.icon,
        icon_color: provider.icon_color,
        sort_index: provider.sort_index,
        is_applied: provider.is_applied,
        is_disabled,
        created_at,
        updated_at: now,
    };

    db.query(format!("UPDATE gemini_cli_provider:`{}` CONTENT $data", id))
        .bind(("data", adapter::to_db_value_provider(&content)))
        .await
        .map_err(|error| format!("Failed to update Gemini CLI provider: {}", error))?;

    if content.is_applied {
        if let Err(error) = apply_config_to_file(&db, &id).await {
            eprintln!("Failed to auto-apply Gemini CLI provider: {}", error);
        } else {
            emit_sync_requests(&app);
        }
    }

    let _ = app.emit("config-changed", "window");

    Ok(GeminiCliProvider {
        id,
        name: content.name,
        category: content.category,
        settings_config: content.settings_config,
        source_provider_id: content.source_provider_id,
        website_url: content.website_url,
        notes: content.notes,
        icon: content.icon,
        icon_color: content.icon_color,
        sort_index: content.sort_index,
        is_applied: content.is_applied,
        is_disabled: content.is_disabled,
        created_at: content.created_at,
        updated_at: content.updated_at,
    })
}

#[tauri::command]
pub async fn delete_gemini_cli_provider(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let db = state.db();
    super::official_accounts::ensure_gemini_cli_provider_has_no_official_accounts(&db, &id).await?;
    db.query(format!("DELETE gemini_cli_provider:`{}`", id))
        .await
        .map_err(|error| format!("Failed to delete Gemini CLI provider: {}", error))?;
    let _ = app.emit("config-changed", "window");
    Ok(())
}

#[tauri::command]
pub async fn reorder_gemini_cli_providers(
    state: tauri::State<'_, DbState>,
    ids: Vec<String>,
) -> Result<(), String> {
    let db = state.db();
    let now = Local::now().to_rfc3339();
    for (index, id) in ids.iter().enumerate() {
        let record_id = db_record_id("gemini_cli_provider", id);
        db.query(&format!(
            "UPDATE {} SET sort_index = $index, updated_at = $now",
            record_id
        ))
        .bind(("index", index as i32))
        .bind(("now", now.clone()))
        .await
        .map_err(|error| format!("Failed to reorder Gemini CLI provider: {}", error))?;
    }
    Ok(())
}

pub(crate) async fn query_provider_by_id(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    provider_id: &str,
) -> Result<GeminiCliProvider, String> {
    let record_id = db_record_id("gemini_cli_provider", provider_id);
    let records_result: Result<Vec<Value>, _> = db
        .query(&format!(
            "SELECT *, type::string(id) as id FROM {} LIMIT 1",
            record_id
        ))
        .await
        .map_err(|error| format!("Failed to query Gemini CLI provider: {}", error))?
        .take(0);
    records_result
        .map_err(|error| format!("Failed to deserialize Gemini CLI provider: {}", error))?
        .into_iter()
        .next()
        .map(adapter::from_db_value_provider)
        .ok_or_else(|| "Gemini CLI provider not found".to_string())
}

async fn apply_config_to_file(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    provider_id: &str,
) -> Result<(), String> {
    let provider = query_provider_by_id(db, provider_id).await?;
    if provider.is_disabled {
        return Err(format!(
            "Gemini CLI provider '{}' is disabled and cannot be applied",
            provider_id
        ));
    }
    let mut provider_settings = parse_provider_settings_config(&provider.settings_config)?;
    normalize_provider_settings_for_category(&mut provider_settings, &provider.category);
    let provider_env = extract_env_object(&provider_settings);
    let provider_config = extract_config_object(&provider_settings);

    write_env_to_db_async(db, &provider_env).await?;

    let common_config = load_stored_common_config_value(db)
        .await?
        .unwrap_or_else(|| Value::Object(Map::new()));
    let mut settings = read_settings_value_from_db_async(db)
        .await?
        .unwrap_or_else(|| Value::Object(Map::new()));
    if !settings.is_object() {
        settings = Value::Object(Map::new());
    }
    remove_selected_auth_type(&mut settings);
    merge_json_value(&mut settings, &common_config);
    merge_json_value(&mut settings, &provider_config);
    write_settings_value_to_db_async(db, &settings).await
}

pub async fn apply_config_internal<R: tauri::Runtime>(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    app: &tauri::AppHandle<R>,
    provider_id: &str,
    from_tray: bool,
) -> Result<(), String> {
    apply_config_to_file(db, provider_id).await?;
    rewrite_applied_prompt_to_current_file(db).await?;
    let now = Local::now().to_rfc3339();

    db.query("UPDATE gemini_cli_provider SET is_applied = false, updated_at = $now WHERE is_applied = true")
        .bind(("now", now.clone()))
        .await
        .map_err(|error| format!("Failed to reset Gemini CLI applied status: {}", error))?;

    let record_id = db_record_id("gemini_cli_provider", provider_id);
    db.query(&format!(
        "UPDATE {} SET is_applied = true, updated_at = $now",
        record_id
    ))
    .bind(("now", now))
    .await
    .map_err(|error| format!("Failed to set Gemini CLI applied status: {}", error))?;

    let payload = if from_tray { "tray" } else { "window" };
    let _ = app.emit("config-changed", payload);
    emit_sync_requests(app);
    Ok(())
}

#[tauri::command]
pub async fn select_gemini_cli_provider(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let db = state.db();
    apply_config_internal(&db, &app, &id, false).await
}

#[tauri::command]
pub async fn toggle_gemini_cli_provider_disabled(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    provider_id: String,
    is_disabled: bool,
) -> Result<(), String> {
    let db = state.db();
    let now = Local::now().to_rfc3339();
    db.query(format!(
        "UPDATE gemini_cli_provider:`{}` SET is_disabled = $is_disabled, updated_at = $now",
        provider_id
    ))
    .bind(("is_disabled", is_disabled))
    .bind(("now", now))
    .await
    .map_err(|error| format!("Failed to toggle Gemini CLI provider: {}", error))?;

    let provider = query_provider_by_id(&db, &provider_id).await?;
    if provider.is_applied && !is_disabled {
        apply_config_internal(&db, &app, &provider_id, false).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_gemini_cli_common_config(
    state: tauri::State<'_, DbState>,
) -> Result<Option<GeminiCliCommonConfig>, String> {
    let db = state.db();
    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM gemini_cli_common_config:`common` LIMIT 1")
        .await
        .map_err(|error| format!("Failed to query Gemini CLI common config: {}", error))?
        .take(0);
    match records_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                Ok(Some(adapter::from_db_value_common(record.clone())))
            } else if let Ok(temp_common) = load_temp_common_config_from_file_with_db(&db).await {
                Ok(Some(temp_common))
            } else {
                Ok(None)
            }
        }
        Err(error) => {
            eprintln!(
                "Gemini CLI common config has incompatible format, cleaning up: {}",
                error
            );
            let _ = db.query("DELETE gemini_cli_common_config:`common`").await;
            let _ =
                runtime_location::refresh_runtime_location_cache_for_module_async(&db, "geminicli")
                    .await;
            Ok(None)
        }
    }
}

#[tauri::command]
pub async fn extract_gemini_cli_common_config_from_current_file(
    state: tauri::State<'_, DbState>,
) -> Result<GeminiCliCommonConfig, String> {
    let db = state.db();
    load_temp_common_config_from_file_with_db(&db).await
}

#[tauri::command]
pub async fn save_gemini_cli_common_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: GeminiCliCommonConfigInput,
) -> Result<(), String> {
    let db = state.db();
    if !input.config.trim().is_empty() {
        let parsed: Value = serde_json::from_str(&input.config)
            .map_err(|error| format!("Invalid JSON: {}", error))?;
        if !parsed.is_object() {
            return Err("Gemini CLI common config must be a JSON object".to_string());
        }
    }

    let existing_common = get_gemini_cli_common_config(state.clone()).await?;
    let root_dir = if input.clear_root_dir {
        None
    } else {
        input
            .root_dir
            .as_deref()
            .map(str::trim)
            .filter(|dir| !dir.is_empty())
            .map(str::to_string)
            .or_else(|| existing_common.and_then(|config| config.root_dir))
    };
    db.query("UPSERT gemini_cli_common_config:`common` CONTENT $data")
        .bind((
            "data",
            adapter::to_db_value_common(&input.config, root_dir.as_deref()),
        ))
        .await
        .map_err(|error| format!("Failed to save Gemini CLI common config: {}", error))?;
    runtime_location::refresh_runtime_location_cache_for_module_async(&db, "geminicli").await?;

    let applied_result: Result<Vec<Value>, _> = db
        .query(
            "SELECT *, type::string(id) as id FROM gemini_cli_provider WHERE is_applied = true LIMIT 1",
        )
        .await
        .map_err(|error| format!("Failed to query applied Gemini CLI provider: {}", error))?
        .take(0);
    if let Ok(records) = applied_result {
        if let Some(record) = records.first() {
            let provider = adapter::from_db_value_provider(record.clone());
            if apply_config_to_file(&db, &provider.id).await.is_ok() {
                if let Err(error) = rewrite_applied_prompt_to_current_file(&db).await {
                    eprintln!(
                        "Failed to rewrite Gemini CLI applied prompt after common config save: {}",
                        error
                    );
                }
                emit_sync_requests(&app);
            }
        }
    }

    let _ = app.emit("config-changed", "window");
    Ok(())
}

#[tauri::command]
pub async fn save_gemini_cli_local_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: GeminiCliLocalConfigInput,
) -> Result<(), String> {
    let db = state.db();
    let base_provider = load_temp_provider_from_files_with_db(&db).await?;
    let base_common = load_temp_common_config_from_file_with_db(&db).await.ok();
    let provider_input = input.provider;
    let provider_settings_config = provider_input
        .as_ref()
        .map(|provider| provider.settings_config.clone())
        .unwrap_or_else(|| base_provider.settings_config.clone());
    let provider_category = provider_input
        .as_ref()
        .map(|provider| provider.category.clone())
        .unwrap_or_else(|| base_provider.category.clone());
    let normalized_provider_settings_config =
        normalize_provider_settings_for_storage(&provider_settings_config, &provider_category)?;
    let now = Local::now().to_rfc3339();
    let provider_content = GeminiCliProviderContent {
        name: provider_input
            .as_ref()
            .map(|provider| provider.name.clone())
            .unwrap_or(base_provider.name),
        category: provider_category,
        settings_config: normalized_provider_settings_config,
        source_provider_id: provider_input
            .as_ref()
            .and_then(|provider| provider.source_provider_id.clone()),
        website_url: provider_input
            .as_ref()
            .and_then(|provider| provider.website_url.clone()),
        notes: provider_input
            .as_ref()
            .and_then(|provider| provider.notes.clone())
            .or(base_provider.notes),
        icon: provider_input
            .as_ref()
            .and_then(|provider| provider.icon.clone()),
        icon_color: provider_input
            .as_ref()
            .and_then(|provider| provider.icon_color.clone()),
        sort_index: provider_input
            .as_ref()
            .and_then(|provider| provider.sort_index)
            .or(base_provider.sort_index),
        is_applied: false,
        is_disabled: provider_input
            .as_ref()
            .and_then(|provider| provider.is_disabled)
            .unwrap_or(false),
        created_at: now.clone(),
        updated_at: now,
    };
    db.query("CREATE gemini_cli_provider CONTENT $data")
        .bind(("data", adapter::to_db_value_provider(&provider_content)))
        .await
        .map_err(|error| format!("Failed to save Gemini CLI local provider: {}", error))?;
    let created_result: Result<Vec<Value>, _> = db
        .query(
            "SELECT *, type::string(id) as id FROM gemini_cli_provider ORDER BY created_at DESC LIMIT 1",
        )
        .await
        .map_err(|error| format!("Failed to fetch saved Gemini CLI local provider: {}", error))?
        .take(0);
    let created_provider = created_result
        .map_err(|error| {
            format!(
                "Failed to deserialize saved Gemini CLI local provider: {}",
                error
            )
        })?
        .into_iter()
        .next()
        .map(adapter::from_db_value_provider)
        .ok_or_else(|| "Failed to retrieve saved Gemini CLI local provider".to_string())?;

    let common_config = if let Some(config) = input.common_config {
        if !config.trim().is_empty() {
            let parsed: Value = serde_json::from_str(&config)
                .map_err(|error| format!("Invalid JSON: {}", error))?;
            if !parsed.is_object() {
                return Err("Gemini CLI common config must be a JSON object".to_string());
            }
        }
        config
    } else if let Some(common) = base_common.as_ref() {
        common.config.clone()
    } else {
        "{}".to_string()
    };
    let existing_custom_root = get_gemini_cli_custom_root_dir_async(&db)
        .await
        .map(|path| path.to_string_lossy().to_string());
    let root_dir = if input.clear_root_dir {
        None
    } else {
        input
            .root_dir
            .as_deref()
            .map(str::trim)
            .filter(|dir| !dir.is_empty())
            .map(str::to_string)
            .or(existing_custom_root)
    };
    db.query("UPSERT gemini_cli_common_config:`common` CONTENT $data")
        .bind((
            "data",
            adapter::to_db_value_common(&common_config, root_dir.as_deref()),
        ))
        .await
        .map_err(|error| format!("Failed to save Gemini CLI common config: {}", error))?;
    runtime_location::refresh_runtime_location_cache_for_module_async(&db, "geminicli").await?;
    apply_config_internal(&db, &app, &created_provider.id, false).await?;
    Ok(())
}

#[tauri::command]
pub async fn list_gemini_cli_prompt_configs(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<GeminiCliPromptConfig>, String> {
    let db = state.db();
    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM gemini_cli_prompt_config")
        .await
        .map_err(|error| format!("Failed to query Gemini CLI prompt configs: {}", error))?
        .take(0);
    match records_result {
        Ok(records) => {
            if records.is_empty() {
                if let Some(local_config) = get_local_prompt_config(Some(&db)).await? {
                    return Ok(vec![local_config]);
                }
                return Ok(Vec::new());
            }
            let mut result: Vec<GeminiCliPromptConfig> = records
                .into_iter()
                .map(adapter::from_db_value_prompt)
                .collect();
            result.sort_by(|a, b| match (a.sort_index, b.sort_index) {
                (Some(ai), Some(bi)) => ai.cmp(&bi),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.name.cmp(&b.name),
            });
            Ok(result)
        }
        Err(error) => {
            eprintln!("Failed to deserialize Gemini CLI prompt configs: {}", error);
            if let Some(local_config) = get_local_prompt_config(Some(&db)).await? {
                return Ok(vec![local_config]);
            }
            Ok(Vec::new())
        }
    }
}

#[tauri::command]
pub async fn create_gemini_cli_prompt_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: GeminiCliPromptConfigInput,
) -> Result<GeminiCliPromptConfig, String> {
    let db = state.db();
    let now = Local::now().to_rfc3339();
    let sort_index_result: Result<Vec<Value>, _> = db
        .query("SELECT sort_index FROM gemini_cli_prompt_config ORDER BY sort_index DESC LIMIT 1")
        .await
        .map_err(|error| format!("Failed to query prompt sort index: {}", error))?
        .take(0);
    let next_sort_index = sort_index_result
        .ok()
        .and_then(|records| records.first().cloned())
        .and_then(|record| record.get("sort_index").and_then(Value::as_i64))
        .map(|value| value as i32 + 1)
        .unwrap_or(0);
    let content = GeminiCliPromptConfigContent {
        name: input.name,
        content: input.content,
        is_applied: false,
        sort_index: Some(next_sort_index),
        created_at: now.clone(),
        updated_at: now,
    };
    let prompt_id = db_new_id();
    let record_id = db_record_id("gemini_cli_prompt_config", &prompt_id);
    db.query(&format!("CREATE {} CONTENT $data", record_id))
        .bind(("data", adapter::to_db_value_prompt(&content)))
        .await
        .map_err(|error| format!("Failed to create Gemini CLI prompt config: {}", error))?;
    let records_result: Result<Vec<Value>, _> = db
        .query(&format!(
            "SELECT *, type::string(id) as id FROM {} LIMIT 1",
            record_id
        ))
        .await
        .map_err(|error| format!("Failed to query created prompt config: {}", error))?
        .take(0);
    let _ = app.emit("config-changed", "window");
    records_result
        .ok()
        .and_then(|records| records.first().cloned())
        .map(adapter::from_db_value_prompt)
        .ok_or_else(|| "Failed to retrieve created Gemini CLI prompt config".to_string())
}

#[tauri::command]
pub async fn update_gemini_cli_prompt_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: GeminiCliPromptConfigInput,
) -> Result<GeminiCliPromptConfig, String> {
    let config_id = input
        .id
        .ok_or_else(|| "ID is required for update".to_string())?;
    let db = state.db();
    let record_id = db_record_id("gemini_cli_prompt_config", &config_id);
    let existing_result: Result<Vec<Value>, _> = db
        .query(&format!(
            "SELECT created_at, is_applied, sort_index FROM {} LIMIT 1",
            record_id
        ))
        .await
        .map_err(|error| format!("Failed to query Gemini CLI prompt config: {}", error))?
        .take(0);
    let existing_record = existing_result
        .map_err(|error| format!("Failed to deserialize prompt config: {}", error))?
        .into_iter()
        .next()
        .ok_or_else(|| format!("Prompt config '{}' not found", config_id))?;
    let now = Local::now().to_rfc3339();
    let created_at = existing_record
        .get("created_at")
        .and_then(Value::as_str)
        .unwrap_or(&now)
        .to_string();
    let is_applied = existing_record
        .get("is_applied")
        .or_else(|| existing_record.get("isApplied"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let sort_index = existing_record
        .get("sort_index")
        .or_else(|| existing_record.get("sortIndex"))
        .and_then(Value::as_i64)
        .map(|value| value as i32);
    let content = GeminiCliPromptConfigContent {
        name: input.name,
        content: input.content.clone(),
        is_applied,
        sort_index,
        created_at,
        updated_at: now.clone(),
    };
    db.query(&format!("UPDATE {} CONTENT $data", record_id))
        .bind(("data", adapter::to_db_value_prompt(&content)))
        .await
        .map_err(|error| format!("Failed to update Gemini CLI prompt config: {}", error))?;
    if is_applied {
        write_prompt_content_to_file(Some(&db), Some(input.content.as_str())).await?;
        emit_sync_requests(&app);
    }
    let _ = app.emit("config-changed", "window");
    Ok(GeminiCliPromptConfig {
        id: config_id,
        name: content.name,
        content: content.content,
        is_applied,
        sort_index,
        created_at: Some(content.created_at),
        updated_at: Some(now),
    })
}

#[tauri::command]
pub async fn delete_gemini_cli_prompt_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let db = state.db();
    let record_id = db_record_id("gemini_cli_prompt_config", &id);
    let was_applied = db
        .query(&format!("SELECT is_applied FROM {} LIMIT 1", record_id))
        .await
        .ok()
        .and_then(|mut result| result.take::<Vec<Value>>(0).ok())
        .and_then(|records| records.into_iter().next())
        .and_then(|record| {
            record
                .get("is_applied")
                .or_else(|| record.get("isApplied"))
                .and_then(Value::as_bool)
        })
        .unwrap_or(false);
    db.query(&format!("DELETE {}", record_id))
        .await
        .map_err(|error| format!("Failed to delete Gemini CLI prompt config: {}", error))?;
    if was_applied {
        write_prompt_content_to_file(Some(&db), None).await?;
        emit_sync_requests(&app);
    }
    let _ = app.emit("config-changed", "window");
    Ok(())
}

pub async fn apply_prompt_config_internal<R: tauri::Runtime>(
    state: tauri::State<'_, DbState>,
    app: &tauri::AppHandle<R>,
    config_id: &str,
    from_tray: bool,
) -> Result<(), String> {
    if config_id == "__local__" {
        let db = state.db();
        let local_prompt = get_local_prompt_config(Some(&db))
            .await?
            .ok_or_else(|| "Local default prompt not found".to_string())?;
        write_prompt_content_to_file(Some(&db), Some(local_prompt.content.as_str())).await?;
        let payload = if from_tray { "tray" } else { "window" };
        let _ = app.emit("config-changed", payload);
        emit_sync_requests(app);
        return Ok(());
    }

    let db = state.db();
    let record_id = db_record_id("gemini_cli_prompt_config", config_id);
    let records_result: Result<Vec<Value>, _> = db
        .query(&format!(
            "SELECT *, type::string(id) as id FROM {} LIMIT 1",
            record_id
        ))
        .await
        .map_err(|error| format!("Failed to query Gemini CLI prompt config: {}", error))?
        .take(0);
    let prompt_config = records_result
        .map_err(|error| format!("Failed to deserialize Gemini CLI prompt config: {}", error))?
        .into_iter()
        .next()
        .map(adapter::from_db_value_prompt)
        .ok_or_else(|| format!("Prompt config '{}' not found", config_id))?;
    let now = Local::now().to_rfc3339();
    db.query("UPDATE gemini_cli_prompt_config SET is_applied = false, updated_at = $now WHERE is_applied = true")
        .bind(("now", now.clone()))
        .await
        .map_err(|error| format!("Failed to clear prompt applied flags: {}", error))?;
    db.query(&format!(
        "UPDATE {} SET is_applied = true, updated_at = $now",
        record_id
    ))
    .bind(("now", now))
    .await
    .map_err(|error| format!("Failed to set prompt applied flag: {}", error))?;
    write_prompt_content_to_file(Some(&db), Some(prompt_config.content.as_str())).await?;
    let payload = if from_tray { "tray" } else { "window" };
    let _ = app.emit("config-changed", payload);
    emit_sync_requests(app);
    Ok(())
}

#[tauri::command]
pub async fn apply_gemini_cli_prompt_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    config_id: String,
) -> Result<(), String> {
    apply_prompt_config_internal(state, &app, &config_id, false).await
}

#[tauri::command]
pub async fn reorder_gemini_cli_prompt_configs(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    ids: Vec<String>,
) -> Result<(), String> {
    let db = state.db();
    for (index, id) in ids.iter().enumerate() {
        let record_id = db_record_id("gemini_cli_prompt_config", id);
        db.query(&format!("UPDATE {} SET sort_index = $index", record_id))
            .bind(("index", index as i32))
            .await
            .map_err(|error| format!("Failed to update prompt sort index: {}", error))?;
    }
    let _ = app.emit("config-changed", "window");
    Ok(())
}

#[tauri::command]
pub async fn save_gemini_cli_local_prompt_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: GeminiCliPromptConfigInput,
) -> Result<GeminiCliPromptConfig, String> {
    let prompt_content = if input.content.trim().is_empty() {
        let db = state.db();
        get_local_prompt_config(Some(&db))
            .await?
            .map(|config| config.content)
            .unwrap_or_default()
    } else {
        input.content
    };
    let created = create_gemini_cli_prompt_config(
        state.clone(),
        app.clone(),
        GeminiCliPromptConfigInput {
            id: None,
            name: input.name,
            content: prompt_content,
        },
    )
    .await?;
    apply_prompt_config_internal(state.clone(), &app, &created.id, false).await?;
    let db = state.db();
    let record_id = db_record_id("gemini_cli_prompt_config", &created.id);
    let refreshed_result: Result<Vec<Value>, _> = db
        .query(&format!(
            "SELECT *, type::string(id) as id FROM {} LIMIT 1",
            record_id
        ))
        .await
        .map_err(|error| format!("Failed to query saved local prompt config: {}", error))?
        .take(0);
    Ok(refreshed_result
        .ok()
        .and_then(|records| records.first().cloned())
        .map(adapter::from_db_value_prompt)
        .unwrap_or(created))
}

#[cfg(test)]
mod tests {
    use super::{
        get_gemini_cli_prompt_file_name_from_settings_value,
        infer_gemini_cli_provider_category_from_settings, merge_env_content, merge_json_value,
        normalize_provider_settings_for_storage, parse_env_content, DEFAULT_GEMINI_CLI_PROMPT_FILE,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn env_merge_removes_previous_managed_keys_and_preserves_other_lines() {
        let existing = "# keep\nOTHER=1\nGEMINI_API_KEY=old\nGOOGLE_GENAI_USE_GCA=true\nGOOGLE_GENAI_USE_VERTEXAI=true\nGOOGLE_VERTEX_BASE_URL=https://old.vertex.example\nGOOGLE_CLOUD_PROJECT=old-project\nGEMINI_CLI_CUSTOM_HEADERS=old\n";
        let provider_env = BTreeMap::from([
            ("GEMINI_API_KEY".to_string(), "new".to_string()),
            ("GEMINI_MODEL".to_string(), "gemini-3.1-pro".to_string()),
            (
                "GEMINI_CLI_CUSTOM_HEADERS".to_string(),
                "x-provider:direct".to_string(),
            ),
        ]);
        let merged = merge_env_content(existing, &provider_env);
        assert!(merged.contains("# keep"));
        assert!(merged.contains("OTHER=1"));
        assert!(!merged.contains("GEMINI_API_KEY=old"));
        assert!(!merged.contains("GOOGLE_GENAI_USE_GCA=true"));
        assert!(!merged.contains("GOOGLE_GENAI_USE_VERTEXAI=true"));
        assert!(!merged.contains("GOOGLE_VERTEX_BASE_URL=https://old.vertex.example"));
        assert!(!merged.contains("GOOGLE_CLOUD_PROJECT=old-project"));
        assert!(!merged.contains("GEMINI_CLI_CUSTOM_HEADERS=old"));
        assert!(merged.contains("GEMINI_API_KEY=new"));
        assert!(merged.contains("GEMINI_MODEL=gemini-3.1-pro"));
        assert!(merged.contains("GEMINI_CLI_CUSTOM_HEADERS=x-provider:direct"));
    }

    #[test]
    fn env_parser_handles_basic_quoted_values() {
        let parsed = parse_env_content("GEMINI_API_KEY=\"abc 123\"\nexport GEMINI_MODEL='flash'\n");
        assert_eq!(
            parsed.get("GEMINI_API_KEY").map(String::as_str),
            Some("abc 123")
        );
        assert_eq!(
            parsed.get("GEMINI_MODEL").map(String::as_str),
            Some("flash")
        );
    }

    #[test]
    fn prompt_file_name_follows_settings_context_file_name() {
        let settings = json!({
            "context": {
                "fileName": [" AGENTS.md ", "GEMINI.md"]
            }
        });
        assert_eq!(
            get_gemini_cli_prompt_file_name_from_settings_value(&settings).as_deref(),
            Some("AGENTS.md")
        );

        let invalid_settings = json!({
            "context": {
                "fileName": ["../outside.md", "", DEFAULT_GEMINI_CLI_PROMPT_FILE]
            }
        });
        assert_eq!(
            get_gemini_cli_prompt_file_name_from_settings_value(&invalid_settings).as_deref(),
            Some(DEFAULT_GEMINI_CLI_PROMPT_FILE)
        );
    }

    #[test]
    fn json_merge_deep_merges_objects() {
        let mut base = json!({"security":{"auth":{"old": true}},"theme":"dark"});
        let patch = json!({"security":{"auth":{"selectedType":"oauth-personal"}}});
        merge_json_value(&mut base, &patch);
        assert_eq!(base["security"]["auth"]["old"], true);
        assert_eq!(base["security"]["auth"]["selectedType"], "oauth-personal");
        assert_eq!(base["theme"], "dark");
    }

    #[test]
    fn official_provider_storage_forces_oauth_and_removes_api_key_env() {
        let settings_config = json!({
            "env": {
                "GEMINI_MODEL": "gemini-3.1-pro-preview",
                "GEMINI_API_KEY": "stale-key",
                "GOOGLE_GEMINI_BASE_URL": "https://proxy.example/v1"
            },
            "config": {
                "general": {
                    "previewFeatures": true
                },
                "security": {
                    "auth": {
                        "selectedType": "gemini-api-key"
                    }
                }
            }
        })
        .to_string();

        let normalized =
            normalize_provider_settings_for_storage(&settings_config, "official").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&normalized).unwrap();

        assert_eq!(
            parsed["config"]["security"]["auth"]["selectedType"],
            "oauth-personal"
        );
        assert_eq!(parsed["env"]["GEMINI_MODEL"], "gemini-3.1-pro-preview");
        assert!(parsed["env"].get("GEMINI_API_KEY").is_none());
        assert!(parsed["env"].get("GOOGLE_GEMINI_BASE_URL").is_none());
        assert_eq!(parsed["config"]["general"]["previewFeatures"], true);
    }

    #[test]
    fn custom_provider_storage_forces_api_key_auth_and_preserves_gateway_env() {
        let settings_config = json!({
            "env": {
                "GEMINI_MODEL": "gemini-3.1-pro-preview",
                "GEMINI_API_KEY": "custom-key",
                "GOOGLE_GEMINI_BASE_URL": "https://proxy.example/v1beta"
            },
            "config": {
                "general": {
                    "previewFeatures": true
                },
                "security": {
                    "auth": {
                        "selectedType": "oauth-personal"
                    }
                }
            }
        })
        .to_string();

        let normalized =
            normalize_provider_settings_for_storage(&settings_config, "custom").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&normalized).unwrap();

        assert_eq!(
            parsed["config"]["security"]["auth"]["selectedType"],
            "gemini-api-key"
        );
        assert_eq!(parsed["env"]["GEMINI_MODEL"], "gemini-3.1-pro-preview");
        assert_eq!(parsed["env"]["GEMINI_API_KEY"], "custom-key");
        assert_eq!(
            parsed["env"]["GOOGLE_GEMINI_BASE_URL"],
            "https://proxy.example/v1beta"
        );
        assert_eq!(parsed["config"]["general"]["previewFeatures"], true);
    }

    #[test]
    fn provider_category_inference_treats_model_only_oauth_config_as_official() {
        let official_settings = json!({
            "env": {
                "GEMINI_MODEL": "gemini-3.1-pro-preview"
            },
            "config": {
                "security": {
                    "auth": {
                        "selectedType": "oauth-personal"
                    }
                }
            }
        });
        let custom_settings = json!({
            "env": {
                "GEMINI_MODEL": "gemini-3.1-pro-preview"
            },
            "config": {
                "security": {
                    "auth": {
                        "selectedType": "gemini-api-key"
                    }
                }
            }
        });

        assert_eq!(
            infer_gemini_cli_provider_category_from_settings(&official_settings),
            "official"
        );
        assert_eq!(
            infer_gemini_cli_provider_category_from_settings(&custom_settings),
            "custom"
        );
    }
}
