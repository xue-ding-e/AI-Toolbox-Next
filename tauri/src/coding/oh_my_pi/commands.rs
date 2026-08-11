use chrono::Local;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Emitter;

use super::types::*;
use crate::coding::open_code::shell_env;
use crate::db::helpers::{db_get, db_put};
use crate::db::schema::DbTable;
use crate::db::SqliteDbState;

const OMP_SETTINGS_RECORD_ID: &str = "oh_my_pi";
const OMP_ENV_KEY: &str = "PI_CODING_AGENT_DIR";
const OMP_MODELS_FILE: &str = "models.yml";
const OMP_MCP_FILE: &str = "mcp.json";

fn home_dir() -> Result<PathBuf, String> {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| "Failed to get home directory".to_string())
}

pub fn get_omp_default_root_dir() -> Result<PathBuf, String> {
    Ok(home_dir()?.join(".omp").join("agent"))
}

fn settings_from_value(value: Value) -> OmpSettingsConfig {
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

fn custom_root(db: &SqliteDbState) -> Result<Option<PathBuf>, String> {
    Ok(db
        .with_conn(|conn| db_get(conn, DbTable::PiSettingsConfig, OMP_SETTINGS_RECORD_ID))?
        .map(settings_from_value)
        .and_then(|settings| settings.root_dir)
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from))
}

pub fn get_omp_root_path_info_from_db(db: &SqliteDbState) -> Result<OmpPathInfo, String> {
    if let Some(path) = custom_root(db)? {
        return Ok(OmpPathInfo {
            path: path.to_string_lossy().to_string(),
            source: "custom".to_string(),
        });
    }
    if let Ok(path) = std::env::var(OMP_ENV_KEY) {
        if !path.trim().is_empty() {
            return Ok(OmpPathInfo {
                path,
                source: "env".to_string(),
            });
        }
    }
    if let Some(path) =
        shell_env::get_env_from_shell_config(OMP_ENV_KEY).filter(|path| !path.trim().is_empty())
    {
        return Ok(OmpPathInfo {
            path,
            source: "shell".to_string(),
        });
    }
    Ok(OmpPathInfo {
        path: get_omp_default_root_dir()?.to_string_lossy().to_string(),
        source: "default".to_string(),
    })
}

pub fn get_omp_mcp_path_from_db(db: &SqliteDbState) -> Result<PathBuf, String> {
    Ok(PathBuf::from(get_omp_root_path_info_from_db(db)?.path).join(OMP_MCP_FILE))
}

fn read_models(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(json!({ "providers": {} }));
    }
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read OMP models.yml: {error}"))?;
    if content.trim().is_empty() {
        return Ok(json!({ "providers": {} }));
    }
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content)
        .map_err(|error| format!("Failed to parse OMP models.yml: {error}"))?;
    let value = serde_json::to_value(yaml)
        .map_err(|error| format!("Failed to convert OMP models.yml: {error}"))?;
    if !value.is_object() {
        return Err("OMP models.yml root must be an object".to_string());
    }
    Ok(value)
}

fn write_models(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create OMP config directory: {error}"))?;
    }
    let yaml = serde_yaml::to_string(value)
        .map_err(|error| format!("Failed to serialize OMP models.yml: {error}"))?;
    fs::write(path, yaml).map_err(|error| format!("Failed to write OMP models.yml: {error}"))
}

fn ensure_providers(models: &mut Value) -> Result<&mut Map<String, Value>, String> {
    let root = models
        .as_object_mut()
        .ok_or_else(|| "OMP models.yml root must be an object".to_string())?;
    if !root.get("providers").is_some_and(Value::is_object) {
        root.insert("providers".to_string(), Value::Object(Map::new()));
    }
    root.get_mut("providers")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "OMP models.yml providers must be an object".to_string())
}

async fn read_runtime(db: &SqliteDbState) -> Result<OmpRuntimeConfig, String> {
    let root_path_info = get_omp_root_path_info_from_db(db)?;
    let root = PathBuf::from(&root_path_info.path);
    let models_path = root.join(OMP_MODELS_FILE);
    let models = read_models(&models_path)?;
    let providers = models
        .get("providers")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| Value::Object(Map::new()));
    Ok(OmpRuntimeConfig {
        root_path_info,
        models_path: models_path.to_string_lossy().to_string(),
        mcp_path: root.join(OMP_MCP_FILE).to_string_lossy().to_string(),
        models,
        providers,
    })
}

#[tauri::command]
pub async fn get_omp_settings_config(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<Option<OmpSettingsConfig>, String> {
    Ok(state
        .db()
        .with_conn(|conn| db_get(conn, DbTable::PiSettingsConfig, OMP_SETTINGS_RECORD_ID))?
        .map(settings_from_value))
}

#[tauri::command]
pub async fn save_omp_settings_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: OmpSettingsConfigInput,
) -> Result<(), String> {
    let existing = get_omp_settings_config(state.clone()).await?;
    let root_dir = if input.clear_root_dir {
        None
    } else {
        input
            .root_dir
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_string)
            .or_else(|| existing.and_then(|settings| settings.root_dir))
    };
    let mut value = json!({ "updated_at": Local::now().to_rfc3339() });
    if let Some(root_dir) = root_dir {
        value["root_dir"] = json!(root_dir);
    }
    state.db().with_conn(|conn| {
        db_put(
            conn,
            DbTable::PiSettingsConfig,
            OMP_SETTINGS_RECORD_ID,
            &value,
        )
    })?;
    let _ = app.emit("config-changed", "window");
    Ok(())
}

#[tauri::command]
pub async fn read_omp_runtime_config(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<OmpRuntimeConfig, String> {
    read_runtime(state.db()).await
}

#[tauri::command]
pub async fn save_omp_provider(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: OmpProviderInput,
) -> Result<OmpRuntimeConfig, String> {
    let provider_key = input.provider_key.trim();
    if provider_key.is_empty() {
        return Err("OMP provider key is required".to_string());
    }
    if !input.provider.is_object() {
        return Err("OMP provider config must be an object".to_string());
    }
    let path =
        PathBuf::from(get_omp_root_path_info_from_db(state.db())?.path).join(OMP_MODELS_FILE);
    let mut models = read_models(&path)?;
    ensure_providers(&mut models)?.insert(provider_key.to_string(), input.provider);
    write_models(&path, &models)?;
    let _ = app.emit("config-changed", "window");
    read_runtime(state.db()).await
}

#[tauri::command]
pub async fn delete_omp_provider(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    provider_key: String,
) -> Result<OmpRuntimeConfig, String> {
    let provider_key = provider_key.trim();
    if provider_key.is_empty() {
        return Err("OMP provider key is required".to_string());
    }
    let path =
        PathBuf::from(get_omp_root_path_info_from_db(state.db())?.path).join(OMP_MODELS_FILE);
    let mut models = read_models(&path)?;
    ensure_providers(&mut models)?.remove(provider_key);
    write_models(&path, &models)?;
    let _ = app.emit("config-changed", "window");
    read_runtime(state.db()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_round_trip_preserves_unknown_provider_fields() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join(OMP_MODELS_FILE);
        let value = json!({
            "providers": {
                "gateway": {
                    "baseUrl": "https://example.com/v1",
                    "api": "openai-responses",
                    "customField": { "enabled": true },
                    "models": [{ "id": "gpt-test", "unknown": 42 }]
                }
            }
        });
        write_models(&path, &value).expect("write models");
        assert_eq!(read_models(&path).expect("read models"), value);
    }

    #[test]
    fn provider_upsert_preserves_siblings_and_unknown_root_fields() {
        let mut models = json!({
            "providers": {
                "first": { "api": "openai-responses" },
                "second": { "api": "anthropic-messages" }
            },
            "futureRootField": true
        });

        ensure_providers(&mut models)
            .expect("providers")
            .insert("first".to_string(), json!({ "api": "openai-completions" }));

        assert_eq!(models["providers"]["second"]["api"], "anthropic-messages");
        assert_eq!(models["futureRootField"], true);
    }

    #[tokio::test]
    async fn mcp_path_resolvers_use_the_independent_custom_root() {
        let db = SqliteDbState::in_memory_for_test().expect("test database");
        let root = PathBuf::from(r"\\wsl.localhost\Ubuntu\home\tester\.omp\agent");
        db.with_conn(|conn| {
            db_put(
                conn,
                DbTable::PiSettingsConfig,
                OMP_SETTINGS_RECORD_ID,
                &json!({ "root_dir": root.to_string_lossy() }),
            )
        })
        .expect("save OMP root");

        let expected = root.join(OMP_MCP_FILE);
        assert_eq!(
            crate::coding::runtime_location::get_tool_mcp_config_path_sync(&db, "oh_my_pi"),
            Some(expected.clone())
        );
        assert_eq!(
            crate::coding::runtime_location::get_tool_mcp_config_path_async(&db, "oh_my_pi").await,
            Some(expected)
        );
    }
}
