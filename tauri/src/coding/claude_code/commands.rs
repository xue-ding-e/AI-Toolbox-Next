use chrono::Local;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use super::adapter;
use super::plugin_cli;
use super::plugin_state;
use super::plugin_types::{
    ClaudeMarketplaceAddInput, ClaudeMarketplaceAutoUpdateInput, ClaudeMarketplaceRemoveInput,
    ClaudeMarketplaceUpdateInput, ClaudePluginActionInput,
};
use super::settings_merge;
use super::settings_merge::KNOWN_ENV_FIELDS;
use super::types::*;
use crate::coding::all_api_hub;
use crate::coding::db_id::{db_new_id, db_record_id};
use crate::coding::open_code::shell_env;
use crate::coding::prompt_file::{read_prompt_content_file, write_prompt_content_file};
use crate::coding::runtime_location;
use crate::coding::skills::commands::resync_all_skills_if_tool_path_changed;
use crate::db::helpers::{db_count, db_delete, db_get, db_list, db_max_i64, db_put};
use crate::db::schema::{DbTable, JsonFieldPath, OrderDirection, OrderField, OrderSpec};
use crate::db::sqlite_state::{global_sqlite_state, SqliteDbState};
use crate::db::DbState;
use tauri::Emitter;

fn get_home_dir() -> Result<PathBuf, String> {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| "Failed to get home directory".to_string())
}

pub fn get_claude_default_root_dir() -> Result<PathBuf, String> {
    Ok(get_home_dir()?.join(".claude"))
}

pub(crate) fn get_claude_root_dir_without_db() -> Result<PathBuf, String> {
    if let Ok(env_path) = std::env::var("CLAUDE_CONFIG_DIR") {
        if !env_path.trim().is_empty() {
            return Ok(PathBuf::from(env_path));
        }
    }

    if let Some(shell_path) = get_claude_root_dir_from_shell() {
        return Ok(shell_path);
    }

    get_claude_default_root_dir()
}

fn get_claude_root_dir_from_shell() -> Option<PathBuf> {
    shell_env::get_env_from_shell_config("CLAUDE_CONFIG_DIR")
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
}

async fn get_claude_custom_root_dir_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Option<PathBuf> {
    if let Some(sqlite_state) = global_sqlite_state() {
        if let Ok(Some(config)) = get_claude_common_from_sqlite(sqlite_state) {
            return config
                .root_dir
                .filter(|dir| !dir.trim().is_empty())
                .map(PathBuf::from);
        }
    }

    let mut result = db
        .query("SELECT * OMIT id FROM claude_common_config:`common` LIMIT 1")
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

pub fn get_claude_root_dir_from_db(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(runtime_location::get_claude_runtime_location_sync(db)?.host_path)
}

async fn get_claude_root_dir_from_db_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    Ok(runtime_location::get_claude_runtime_location_async(db)
        .await?
        .host_path)
}

pub fn get_claude_root_path_info_from_db(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<ConfigPathInfo, String> {
    let location = runtime_location::get_claude_runtime_location_sync(db)?;
    Ok(ConfigPathInfo {
        path: location.host_path.to_string_lossy().to_string(),
        source: location.source,
    })
}

async fn get_claude_root_path_info_from_db_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<ConfigPathInfo, String> {
    let location = runtime_location::get_claude_runtime_location_async(db).await?;
    Ok(ConfigPathInfo {
        path: location.host_path.to_string_lossy().to_string(),
        source: location.source,
    })
}

fn get_claude_prompt_file_path_from_root(root_dir: &Path) -> PathBuf {
    root_dir.join("CLAUDE.md")
}

fn get_claude_prompt_file_path() -> Result<std::path::PathBuf, String> {
    let root_dir = get_claude_root_dir_without_db()?;
    Ok(get_claude_prompt_file_path_from_root(&root_dir))
}

async fn get_claude_prompt_file_path_from_db_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<std::path::PathBuf, String> {
    let root_dir = get_claude_root_dir_from_db_async(db).await?;
    Ok(get_claude_prompt_file_path_from_root(&root_dir))
}

pub(crate) fn get_claude_settings_path_from_root(root_dir: &Path) -> PathBuf {
    root_dir.join("settings.json")
}

async fn get_claude_settings_path_from_db_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    let root_dir = get_claude_root_dir_from_db_async(db).await?;
    Ok(get_claude_settings_path_from_root(&root_dir))
}

async fn get_claude_plugin_config_path_from_db_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<PathBuf, String> {
    runtime_location::get_claude_plugin_config_path_async(db).await
}

async fn read_current_claude_settings_value_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<Option<Value>, String> {
    let settings_path = get_claude_settings_path_from_db_async(db).await?;
    if !settings_path.exists() {
        return Ok(None);
    }

    let raw_content = fs::read_to_string(&settings_path)
        .map_err(|error| format!("Failed to read settings file: {}", error))?;
    let parsed_value = serde_json::from_str::<Value>(&raw_content)
        .map_err(|error| format!("Failed to parse settings file: {}", error))?;
    Ok(Some(parsed_value))
}

async fn write_claude_settings_value_async(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    settings_value: &Value,
) -> Result<(), String> {
    let settings_path = get_claude_settings_path_from_db_async(db).await?;
    if let Some(parent_dir) = settings_path.parent() {
        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir)
                .map_err(|error| format!("Failed to create Claude config directory: {}", error))?;
        }
    }

    let serialized = serde_json::to_string_pretty(settings_value)
        .map_err(|error| format!("Failed to serialize settings: {}", error))?;
    fs::write(&settings_path, format!("{serialized}\n"))
        .map_err(|error| format!("Failed to write settings file: {}", error))
}

async fn load_temp_provider_from_file_with_db(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<ClaudeCodeProvider, String> {
    let settings_value = read_current_claude_settings_value_async(db)
        .await?
        .ok_or_else(|| "No settings file found".to_string())?;
    let stored_common_config = load_stored_claude_common_config_value(db).await?;
    let provider_settings = settings_merge::extract_provider_settings_for_storage(
        &settings_value,
        stored_common_config.as_ref(),
        &KNOWN_ENV_FIELDS,
    )?;

    let env_object = provider_settings
        .as_object()
        .and_then(|object| object.get("env"))
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default();
    if env_object.is_empty() {
        return Err("No provider env section in settings".to_string());
    }

    let inferred_category = infer_claude_provider_category_from_settings(&provider_settings);

    let now = Local::now().to_rfc3339();
    Ok(ClaudeCodeProvider {
        id: "__local__".to_string(),
        name: "default".to_string(),
        category: inferred_category,
        settings_config: serde_json::to_string(&provider_settings)
            .map_err(|error| format!("Failed to serialize provider settings: {}", error))?,
        extra_settings_config: "{}".to_string(),
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

fn infer_claude_provider_category_from_settings(provider_settings: &Value) -> String {
    let provider_env = provider_settings
        .as_object()
        .and_then(|object| object.get("env"))
        .and_then(|value| value.as_object());

    let has_base_url = provider_env
        .and_then(|env| env.get("ANTHROPIC_BASE_URL"))
        .and_then(|value| value.as_str())
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);

    let has_managed_auth = provider_env
        .map(|env| {
            ["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_API_KEY"]
                .iter()
                .any(|field_key| {
                    env.get(*field_key)
                        .and_then(|value| value.as_str())
                        .map(|value| !value.trim().is_empty())
                        .unwrap_or(false)
                })
        })
        .unwrap_or(false);

    if !has_base_url && !has_managed_auth {
        "official".to_string()
    } else {
        "custom".to_string()
    }
}

async fn load_temp_common_config_from_file_with_db(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<ClaudeCommonConfig, String> {
    let settings_value = read_current_claude_settings_value_async(db)
        .await?
        .ok_or_else(|| "No settings file found".to_string())?;

    let (_, common_settings) = settings_merge::split_settings_into_provider_and_common(
        &settings_value,
        &KNOWN_ENV_FIELDS,
    )?;
    let now = Local::now().to_rfc3339();
    Ok(ClaudeCommonConfig {
        config: serde_json::to_string(&common_settings)
            .map_err(|error| format!("Failed to serialize common config: {}", error))?,
        root_dir: get_claude_custom_root_dir_async(db)
            .await
            .map(|path| path.to_string_lossy().to_string()),
        updated_at: now,
    })
}

async fn load_stored_claude_common_config_value(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<Option<Value>, String> {
    if let Some(sqlite_state) = global_sqlite_state() {
        return get_claude_common_from_sqlite(sqlite_state)?
            .map(|config| {
                serde_json::from_str::<Value>(&config.config)
                    .map_err(|e| format!("Failed to parse common config: {}", e))
            })
            .transpose();
    }

    let common_config_result: Result<Vec<Value>, _> = db
        .query("SELECT * OMIT id FROM claude_common_config:`common` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query common config: {}", e))?
        .take(0);

    match common_config_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                let config = adapter::from_db_value_common(record.clone());
                let parsed = serde_json::from_str::<Value>(&config.config)
                    .map_err(|e| format!("Failed to parse common config: {}", e))?;
                Ok(Some(parsed))
            } else {
                Ok(None)
            }
        }
        Err(_) => Ok(None),
    }
}

fn parse_optional_common_config_value(
    raw_common_config: Option<&str>,
) -> Result<Option<Value>, String> {
    match raw_common_config {
        Some(raw_config) => {
            let parsed = serde_json::from_str::<Value>(raw_config)
                .map_err(|e| format!("Failed to parse common config: {}", e))?;
            Ok(Some(parsed))
        }
        None => Ok(None),
    }
}

fn normalize_extra_settings_config_for_storage(
    category: &str,
    raw_extra_settings_config: Option<&str>,
) -> Result<String, String> {
    if category == "official" {
        return Ok("{}".to_string());
    }

    let raw_config = raw_extra_settings_config.unwrap_or("{}");
    let parsed = settings_merge::parse_json_object(raw_config).map(Value::Object)?;
    serde_json::to_string(&parsed)
        .map_err(|e| format!("Failed to serialize extra settings config: {}", e))
}

fn parse_extra_settings_config_value(provider: &ClaudeCodeProvider) -> Result<Value, String> {
    if provider.category == "official" {
        return Ok(Value::Object(serde_json::Map::new()));
    }

    Ok(Value::Object(settings_merge::parse_json_object(
        &provider.extra_settings_config,
    )?))
}

async fn load_applied_provider_extra_settings_value(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<Option<Value>, String> {
    if let Some(sqlite_state) = global_sqlite_state() {
        let applied = list_claude_providers_from_sqlite(sqlite_state)?
            .into_iter()
            .find(|provider| provider.is_applied);
        return applied
            .as_ref()
            .map(parse_extra_settings_config_value)
            .transpose();
    }

    let applied_result: Result<Vec<Value>, _> = db
        .query(
            "SELECT *, type::string(id) as id FROM claude_provider WHERE is_applied = true LIMIT 1",
        )
        .await
        .map_err(|e| format!("Failed to query applied provider: {}", e))?
        .take(0);

    match applied_result {
        Ok(records) => records
            .first()
            .map(|record| {
                let provider = adapter::from_db_value_provider(record.clone());
                parse_extra_settings_config_value(&provider)
            })
            .transpose(),
        Err(_) => Ok(None),
    }
}

async fn normalize_provider_settings_for_storage(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    raw_settings_config: &str,
    common_config_override: Option<&Value>,
) -> Result<String, String> {
    let parsed_settings = serde_json::from_str::<Value>(raw_settings_config)
        .map_err(|e| format!("Failed to parse provider config: {}", e))?;

    let effective_common_config = match common_config_override {
        Some(value) => Some(value.clone()),
        None => load_stored_claude_common_config_value(db).await?,
    };

    let normalized_settings = settings_merge::extract_provider_settings_for_storage(
        &parsed_settings,
        effective_common_config.as_ref(),
        &KNOWN_ENV_FIELDS,
    )?;

    serde_json::to_string(&normalized_settings)
        .map_err(|e| format!("Failed to serialize normalized provider config: {}", e))
}

async fn get_local_prompt_config(
    db: Option<&surrealdb::Surreal<surrealdb::engine::local::Db>>,
) -> Result<Option<ClaudePromptConfig>, String> {
    let prompt_path = if let Some(db) = db {
        get_claude_prompt_file_path_from_db_async(db).await?
    } else {
        get_claude_prompt_file_path()?
    };
    let Some(prompt_content) = read_prompt_content_file(&prompt_path, "Claude Code")? else {
        return Ok(None);
    };

    let now = Local::now().to_rfc3339();
    Ok(Some(ClaudePromptConfig {
        id: "__local__".to_string(),
        name: "default".to_string(),
        content: prompt_content,
        is_applied: true,
        sort_index: None,
        created_at: Some(now.clone()),
        updated_at: Some(now),
    }))
}

async fn write_prompt_content_to_file(
    db: Option<&surrealdb::Surreal<surrealdb::engine::local::Db>>,
    prompt_content: Option<&str>,
) -> Result<(), String> {
    let prompt_path = if let Some(db) = db {
        get_claude_prompt_file_path_from_db_async(db).await?
    } else {
        get_claude_prompt_file_path()?
    };
    write_prompt_content_file(&prompt_path, prompt_content, "Claude Code")
}

fn emit_prompt_sync_requests<R: tauri::Runtime>(_app: &tauri::AppHandle<R>) {
    #[cfg(target_os = "windows")]
    let _ = _app.emit("wsl-sync-request-claude", ());
}

fn claude_provider_order() -> Result<OrderSpec, String> {
    Ok(OrderSpec::single(OrderField::json_integer(
        "sort_index",
        OrderDirection::Asc,
    )?))
}

fn claude_prompt_order() -> Result<OrderSpec, String> {
    Ok(OrderSpec::new(vec![
        OrderField::json_integer("sort_index", OrderDirection::Asc)?,
        OrderField::json_text("name", OrderDirection::Asc)?,
    ]))
}

fn list_claude_providers_from_sqlite(
    sqlite_state: &SqliteDbState,
) -> Result<Vec<ClaudeCodeProvider>, String> {
    let order = claude_provider_order()?;
    sqlite_state.with_conn(|conn| {
        Ok(db_list(conn, DbTable::ClaudeProvider, Some(&order))?
            .into_iter()
            .map(adapter::from_db_value_provider)
            .collect())
    })
}

fn get_claude_provider_from_sqlite(
    sqlite_state: &SqliteDbState,
    provider_id: &str,
) -> Result<Option<ClaudeCodeProvider>, String> {
    sqlite_state.with_conn(|conn| {
        Ok(
            db_get(conn, DbTable::ClaudeProvider, provider_id)?
                .map(adapter::from_db_value_provider),
        )
    })
}

fn put_claude_provider_to_sqlite(
    sqlite_state: &SqliteDbState,
    provider_id: &str,
    content: &ClaudeCodeProviderContent,
) -> Result<(), String> {
    sqlite_state.with_conn(|conn| {
        db_put(
            conn,
            DbTable::ClaudeProvider,
            provider_id,
            &adapter::to_db_value_provider(content),
        )
    })
}

fn delete_claude_provider_from_sqlite(
    sqlite_state: &SqliteDbState,
    provider_id: &str,
) -> Result<(), String> {
    sqlite_state.with_conn(|conn| db_delete(conn, DbTable::ClaudeProvider, provider_id).map(|_| ()))
}

fn list_claude_prompts_from_sqlite(
    sqlite_state: &SqliteDbState,
) -> Result<Vec<ClaudePromptConfig>, String> {
    let order = claude_prompt_order()?;
    sqlite_state.with_conn(|conn| {
        Ok(db_list(conn, DbTable::ClaudePromptConfig, Some(&order))?
            .into_iter()
            .map(adapter::from_db_value_prompt)
            .collect())
    })
}

fn get_claude_prompt_from_sqlite(
    sqlite_state: &SqliteDbState,
    config_id: &str,
) -> Result<Option<ClaudePromptConfig>, String> {
    sqlite_state.with_conn(|conn| {
        Ok(
            db_get(conn, DbTable::ClaudePromptConfig, config_id)?
                .map(adapter::from_db_value_prompt),
        )
    })
}

fn put_claude_prompt_to_sqlite(
    sqlite_state: &SqliteDbState,
    config_id: &str,
    content: &ClaudePromptConfigContent,
) -> Result<(), String> {
    sqlite_state.with_conn(|conn| {
        db_put(
            conn,
            DbTable::ClaudePromptConfig,
            config_id,
            &adapter::to_db_value_prompt(content),
        )
    })
}

fn delete_claude_prompt_from_sqlite(
    sqlite_state: &SqliteDbState,
    config_id: &str,
) -> Result<(), String> {
    sqlite_state
        .with_conn(|conn| db_delete(conn, DbTable::ClaudePromptConfig, config_id).map(|_| ()))
}

fn get_claude_common_from_sqlite(
    sqlite_state: &SqliteDbState,
) -> Result<Option<ClaudeCommonConfig>, String> {
    sqlite_state.with_conn(|conn| {
        Ok(db_get(conn, DbTable::ClaudeCommonConfig, "common")?.map(adapter::from_db_value_common))
    })
}

fn put_claude_common_to_sqlite(
    sqlite_state: &SqliteDbState,
    config: &str,
    root_dir: Option<&str>,
) -> Result<(), String> {
    sqlite_state.with_conn(|conn| {
        db_put(
            conn,
            DbTable::ClaudeCommonConfig,
            "common",
            &adapter::to_db_value_common(config, root_dir),
        )
    })
}

async fn create_claude_provider_in_surreal(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    provider_id: &str,
    content: &ClaudeCodeProviderContent,
) -> Result<(), String> {
    let record_id = db_record_id("claude_provider", provider_id);
    db.query(&format!("CREATE {} CONTENT $data", record_id))
        .bind(("data", adapter::to_db_value_provider(content)))
        .await
        .map_err(|e| format!("Failed to create provider: {}", e))?;
    Ok(())
}

async fn update_claude_provider_in_surreal(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    provider_id: &str,
    content: &ClaudeCodeProviderContent,
) -> Result<(), String> {
    let record_id = db_record_id("claude_provider", provider_id);
    db.query(&format!("UPDATE {} CONTENT $data", record_id))
        .bind(("data", adapter::to_db_value_provider(content)))
        .await
        .map_err(|e| format!("Failed to update provider: {}", e))?;
    Ok(())
}

async fn put_claude_prompt_in_surreal(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    config_id: &str,
    content: &ClaudePromptConfigContent,
    create: bool,
) -> Result<(), String> {
    let record_id = db_record_id("claude_prompt_config", config_id);
    let verb = if create { "CREATE" } else { "UPDATE" };
    db.query(&format!("{} {} CONTENT $data", verb, record_id))
        .bind(("data", adapter::to_db_value_prompt(content)))
        .await
        .map_err(|e| format!("Failed to save prompt config: {}", e))?;
    Ok(())
}

async fn put_claude_common_in_surreal(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    config: &str,
    root_dir: Option<&str>,
) -> Result<(), String> {
    db.query("UPSERT claude_common_config:`common` CONTENT $data")
        .bind(("data", adapter::to_db_value_common(config, root_dir)))
        .await
        .map_err(|e| format!("Failed to save common config: {}", e))?;
    Ok(())
}

// ============================================================================
// Claude Code Provider Commands
// ============================================================================

/// List all Claude Code providers ordered by sort_index
#[tauri::command]
pub async fn list_claude_providers(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<ClaudeCodeProvider>, String> {
    let db = state.db();
    if let Some(sqlite_state) = global_sqlite_state() {
        let records = list_claude_providers_from_sqlite(sqlite_state)?;
        if records.is_empty() {
            if let Ok(temp_provider) = load_temp_provider_from_file_with_db(&db).await {
                return Ok(vec![temp_provider]);
            }
        }
        return Ok(records);
    }

    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM claude_provider")
        .await
        .map_err(|e| format!("Failed to query providers: {}", e))?
        .take(0);

    match records_result {
        Ok(records) => {
            if records.is_empty() {
                // Database is empty, try to load from local file as temporary provider
                if let Ok(temp_provider) = load_temp_provider_from_file_with_db(&db).await {
                    return Ok(vec![temp_provider]);
                }
                Ok(Vec::new())
            } else {
                let mut result: Vec<ClaudeCodeProvider> = records
                    .into_iter()
                    .map(adapter::from_db_value_provider)
                    .collect();
                result.sort_by_key(|p| p.sort_index.unwrap_or(0));
                Ok(result)
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to deserialize providers: {}", e);
            // Try to load from local file as fallback
            if let Ok(temp_provider) = load_temp_provider_from_file_with_db(&db).await {
                return Ok(vec![temp_provider]);
            }
            Ok(Vec::new())
        }
    }
}

/// Load a temporary provider from settings.json without writing to database
/// This is used when the database is empty and we want to show the local config
/// Create a new Claude Code provider
#[tauri::command]
pub async fn create_claude_provider(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    provider: ClaudeCodeProviderInput,
) -> Result<ClaudeCodeProvider, String> {
    let db = state.db();
    let normalized_settings_config =
        normalize_provider_settings_for_storage(&db, &provider.settings_config, None).await?;
    let extra_settings_config = normalize_extra_settings_config_for_storage(
        &provider.category,
        provider.extra_settings_config.as_deref(),
    )?;

    let now = Local::now().to_rfc3339();
    let content = ClaudeCodeProviderContent {
        name: provider.name,
        category: provider.category,
        settings_config: normalized_settings_config,
        extra_settings_config,
        source_provider_id: provider.source_provider_id,
        website_url: provider.website_url,
        notes: provider.notes,
        icon: provider.icon,
        icon_color: provider.icon_color,
        sort_index: provider.sort_index,
        is_applied: false,
        is_disabled: false,
        created_at: now.clone(),
        updated_at: now,
    };

    let json_data = adapter::to_db_value_provider(&content);

    let provider_id = db_new_id();
    if let Some(sqlite_state) = global_sqlite_state() {
        put_claude_provider_to_sqlite(sqlite_state, &provider_id, &content)?;
    }

    create_claude_provider_in_surreal(&db, &provider_id, &content).await?;

    // Notify to refresh tray menu
    let _ = app.emit("config-changed", "window");

    Ok(adapter::from_db_value_provider({
        let mut value = json_data;
        if let Some(object) = value.as_object_mut() {
            object.insert("id".to_string(), Value::String(provider_id));
        }
        value
    }))
}

/// Update an existing Claude Code provider
#[tauri::command]
pub async fn update_claude_provider(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    provider: ClaudeCodeProvider,
) -> Result<ClaudeCodeProvider, String> {
    let db = state.db();
    let normalized_settings_config =
        normalize_provider_settings_for_storage(&db, &provider.settings_config, None).await?;
    let extra_settings_config = normalize_extra_settings_config_for_storage(
        &provider.category,
        Some(&provider.extra_settings_config),
    )?;

    // Use the id from frontend (pure string id without table prefix)
    let id = provider.id.clone();
    let now = Local::now().to_rfc3339();

    // Get existing record to preserve created_at
    let record_id = db_record_id("claude_provider", &id);
    let existing_provider = if let Some(sqlite_state) = global_sqlite_state() {
        get_claude_provider_from_sqlite(sqlite_state, &id)?
    } else {
        let existing_result: Result<Vec<Value>, _> = db
            .query(&format!(
                "SELECT *, type::string(id) as id FROM {} LIMIT 1",
                record_id
            ))
            .await
            .map_err(|e| format!("Failed to query existing provider: {}", e))?
            .take(0);

        existing_result
            .map_err(|e| format!("Failed to deserialize existing provider: {}", e))?
            .first()
            .cloned()
            .map(adapter::from_db_value_provider)
    };
    let existing_provider = existing_provider
        .ok_or_else(|| format!("Claude Code provider with ID '{}' not found", id))?;

    // Get created_at and is_disabled from existing record
    let previous_extra_settings_config_value =
        parse_extra_settings_config_value(&existing_provider).map(Some)?;

    let (created_at, existing_is_disabled) = if !provider.created_at.trim().is_empty() {
        (provider.created_at.clone(), existing_provider.is_disabled)
    } else {
        (
            if existing_provider.created_at.trim().is_empty() {
                now.clone()
            } else {
                existing_provider.created_at.clone()
            },
            existing_provider.is_disabled,
        )
    };

    let content = ClaudeCodeProviderContent {
        name: provider.name,
        category: provider.category,
        settings_config: normalized_settings_config,
        extra_settings_config,
        source_provider_id: provider.source_provider_id,
        website_url: provider.website_url,
        notes: provider.notes,
        icon: provider.icon,
        icon_color: provider.icon_color,
        sort_index: provider.sort_index,
        is_applied: provider.is_applied,
        is_disabled: existing_is_disabled,
        created_at,
        updated_at: now,
    };

    if let Some(sqlite_state) = global_sqlite_state() {
        put_claude_provider_to_sqlite(sqlite_state, &id, &content)?;
    }
    update_claude_provider_in_surreal(&db, &id, &content).await?;

    // 如果该配置当前是应用状态，立即重新写入到配置文件
    if content.is_applied {
        if let Err(e) =
            apply_config_to_file_with_context(&db, &id, None, previous_extra_settings_config_value)
                .await
        {
            eprintln!("Failed to auto-apply updated config: {}", e);
            // 不中断更新流程，只记录错误
        }
    }

    // Notify frontend and tray to refresh
    let _ = app.emit("config-changed", "window");

    Ok(ClaudeCodeProvider {
        id,
        name: content.name,
        category: content.category,
        settings_config: content.settings_config,
        extra_settings_config: content.extra_settings_config,
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

/// Delete a Claude Code provider
#[tauri::command]
pub async fn delete_claude_provider(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let db = state.db();
    if let Some(sqlite_state) = global_sqlite_state() {
        delete_claude_provider_from_sqlite(sqlite_state, &id)?;
    }

    db.query(format!("DELETE claude_provider:`{}`", id))
        .await
        .map_err(|e| format!("Failed to delete claude provider: {}", e))?;

    // Notify to refresh tray menu
    let _ = app.emit("config-changed", "window");

    Ok(())
}

/// Reorder Claude Code providers
#[tauri::command]
pub async fn reorder_claude_providers(
    state: tauri::State<'_, DbState>,
    ids: Vec<String>,
) -> Result<(), String> {
    let db = state.db();
    let now = Local::now().to_rfc3339();

    if let Some(sqlite_state) = global_sqlite_state() {
        for (index, id) in ids.iter().enumerate() {
            if let Some(mut provider) = get_claude_provider_from_sqlite(sqlite_state, id)? {
                provider.sort_index = Some(index as i32);
                provider.updated_at = now.clone();
                let content = ClaudeCodeProviderContent {
                    name: provider.name,
                    category: provider.category,
                    settings_config: provider.settings_config,
                    extra_settings_config: provider.extra_settings_config,
                    source_provider_id: provider.source_provider_id,
                    website_url: provider.website_url,
                    notes: provider.notes,
                    icon: provider.icon,
                    icon_color: provider.icon_color,
                    sort_index: provider.sort_index,
                    is_applied: provider.is_applied,
                    is_disabled: provider.is_disabled,
                    created_at: provider.created_at,
                    updated_at: provider.updated_at,
                };
                put_claude_provider_to_sqlite(sqlite_state, id, &content)?;
            }
        }
    }

    for (index, id) in ids.iter().enumerate() {
        let record_id = db_record_id("claude_provider", id);
        db.query(&format!(
            "UPDATE {} SET sort_index = $index, updated_at = $now",
            record_id
        ))
        .bind(("index", index as i32))
        .bind(("now", now.clone()))
        .await
        .map_err(|e| format!("Failed to update provider {}: {}", id, e))?;
    }

    Ok(())
}

/// Select a Claude Code provider (mark as applied in database, but not write to file)
/// This sets the provider as "current" using is_applied field
#[tauri::command]
pub async fn select_claude_provider(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let db = state.db();

    let now = Local::now().to_rfc3339();

    if let Some(sqlite_state) = global_sqlite_state() {
        for mut provider in list_claude_providers_from_sqlite(sqlite_state)? {
            let should_be_applied = provider.id == id;
            if provider.is_applied == should_be_applied {
                continue;
            }
            provider.is_applied = should_be_applied;
            provider.updated_at = now.clone();
            let provider_id = provider.id.clone();
            let content = ClaudeCodeProviderContent {
                name: provider.name,
                category: provider.category,
                settings_config: provider.settings_config,
                extra_settings_config: provider.extra_settings_config,
                source_provider_id: provider.source_provider_id,
                website_url: provider.website_url,
                notes: provider.notes,
                icon: provider.icon,
                icon_color: provider.icon_color,
                sort_index: provider.sort_index,
                is_applied: provider.is_applied,
                is_disabled: provider.is_disabled,
                created_at: provider.created_at,
                updated_at: provider.updated_at,
            };
            put_claude_provider_to_sqlite(sqlite_state, &provider_id, &content)?;
        }
    }

    // Mark all providers as not applied (only update the currently applied one)
    db.query(
        "UPDATE claude_provider SET is_applied = false, updated_at = $now WHERE is_applied = true",
    )
    .bind(("now", now.clone()))
    .await
    .map_err(|e| format!("Failed to reset applied status: {}", e))?;

    // Mark target provider as applied
    let record_id = db_record_id("claude_provider", &id);
    db.query(&format!(
        "UPDATE {} SET is_applied = true, updated_at = $now",
        record_id
    ))
    .bind(("now", now))
    .await
    .map_err(|e| format!("Failed to set applied status: {}", e))?;

    // Notify frontend to refresh
    let _ = app.emit("config-changed", "window");

    Ok(())
}

// ============================================================================
// Claude Config File Commands
// ============================================================================

/// Get Claude config file path (~/.claude/settings.json)
#[tauri::command]
pub async fn get_claude_config_path(state: tauri::State<'_, DbState>) -> Result<String, String> {
    let db = state.db();
    let config_path = get_claude_settings_path_from_db_async(&db).await?;
    Ok(config_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn get_claude_root_path_info(
    state: tauri::State<'_, DbState>,
) -> Result<ConfigPathInfo, String> {
    let db = state.db();
    get_claude_root_path_info_from_db_async(&db).await
}

/// Reveal Claude config folder in file explorer
#[tauri::command]
pub async fn reveal_claude_config_folder(state: tauri::State<'_, DbState>) -> Result<(), String> {
    let db = state.db();
    let config_dir = get_claude_root_dir_from_db_async(&db).await?;

    // Ensure directory exists
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create .claude directory: {}", e))?;
    }

    // Open in file explorer (platform-specific)
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(config_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(config_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(config_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    Ok(())
}

/// Read Claude settings.json file
#[tauri::command]
pub async fn read_claude_settings(
    state: tauri::State<'_, DbState>,
) -> Result<ClaudeSettings, String> {
    let db = state.db();
    let config_path = get_claude_settings_path_from_db_async(&db).await?;

    if !config_path.exists() {
        // Return empty settings if file doesn't exist
        return Ok(ClaudeSettings {
            env: None,
            other: serde_json::Map::new(),
        });
    }

    let content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read settings file: {}", e))?;

    let settings: ClaudeSettings = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse settings file: {}", e))?;

    Ok(settings)
}

/// 内部函数：将指定 provider 的配置应用到 settings.json（不改变数据库中的 is_applied 状态）
async fn apply_config_to_file(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    provider_id: &str,
) -> Result<(), String> {
    apply_config_to_file_with_previous_common_config(db, provider_id, None).await
}

async fn apply_config_to_file_with_previous_common_config(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    provider_id: &str,
    previous_common_config: Option<Value>,
) -> Result<(), String> {
    apply_config_to_file_with_context(db, provider_id, previous_common_config, None).await
}

async fn apply_config_to_file_with_context(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    provider_id: &str,
    previous_common_config: Option<Value>,
    previous_extra_settings_config: Option<Value>,
) -> Result<(), String> {
    // Get the provider
    let record_id = db_record_id("claude_provider", provider_id);
    let provider = if let Some(sqlite_state) = global_sqlite_state() {
        get_claude_provider_from_sqlite(sqlite_state, provider_id)?
            .ok_or_else(|| "Provider not found".to_string())?
    } else {
        let provider_result: Result<Vec<Value>, _> = db
            .query(&format!(
                "SELECT *, type::string(id) as id FROM {} LIMIT 1",
                record_id
            ))
            .await
            .map_err(|e| format!("Failed to query provider: {}", e))?
            .take(0);

        match provider_result {
            Ok(records) => {
                if let Some(record) = records.first() {
                    adapter::from_db_value_provider(record.clone())
                } else {
                    return Err("Provider not found".to_string());
                }
            }
            Err(e) => {
                return Err(format!("Failed to deserialize provider: {}", e));
            }
        }
    };

    // Check if provider is disabled
    if provider.is_disabled {
        return Err(format!(
            "Provider '{}' is disabled and cannot be applied",
            provider_id
        ));
    }

    // Parse provider settings_config
    let provider_config: serde_json::Value = serde_json::from_str(&provider.settings_config)
        .map_err(|e| format!("Failed to parse provider config: {}", e))?;
    let extra_settings_config = parse_extra_settings_config_value(&provider)?;
    let previous_extra_settings_config = match previous_extra_settings_config {
        Some(value) => Some(value),
        None => load_applied_provider_extra_settings_value(db).await?,
    };

    // Get common config
    let common_config: serde_json::Value = if let Some(sqlite_state) = global_sqlite_state() {
        if let Some(config) = get_claude_common_from_sqlite(sqlite_state)? {
            serde_json::from_str(&config.config)
                .map_err(|e| format!("Failed to parse common config: {}", e))?
        } else {
            serde_json::json!({})
        }
    } else {
        let common_config_result: Result<Vec<Value>, _> = db
            .query("SELECT * OMIT id FROM claude_common_config:`common` LIMIT 1")
            .await
            .map_err(|e| format!("Failed to query common config: {}", e))?
            .take(0);

        match common_config_result {
            Ok(records) => {
                if let Some(record) = records.first() {
                    let config = adapter::from_db_value_common(record.clone());
                    serde_json::from_str(&config.config)
                        .map_err(|e| format!("Failed to parse common config: {}", e))?
                } else {
                    serde_json::json!({})
                }
            }
            Err(_) => serde_json::json!({}),
        }
    };

    let current_settings = read_current_claude_settings_value_async(db).await?;
    let merged_settings = settings_merge::merge_claude_settings_for_provider(
        current_settings.as_ref(),
        previous_common_config.as_ref(),
        &common_config,
        previous_extra_settings_config.as_ref(),
        Some(&extra_settings_config),
        &provider_config,
        &KNOWN_ENV_FIELDS,
    )?;
    write_claude_settings_value_async(db, &merged_settings).await
}

/// Public version of apply_config_to_file for tray module
pub async fn apply_config_to_file_public(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    provider_id: &str,
) -> Result<(), String> {
    apply_config_to_file_with_previous_common_config(db, provider_id, None).await
}
/// Toggle is_disabled status for a provider
#[tauri::command]
pub async fn toggle_claude_code_provider_disabled(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    provider_id: String,
    is_disabled: bool,
) -> Result<(), String> {
    let db = state.db();

    // Update is_disabled field in database
    let now = Local::now().to_rfc3339();
    if let Some(sqlite_state) = global_sqlite_state() {
        if let Some(mut provider) = get_claude_provider_from_sqlite(sqlite_state, &provider_id)? {
            provider.is_disabled = is_disabled;
            provider.updated_at = now.clone();
            let content = ClaudeCodeProviderContent {
                name: provider.name,
                category: provider.category,
                settings_config: provider.settings_config,
                extra_settings_config: provider.extra_settings_config,
                source_provider_id: provider.source_provider_id,
                website_url: provider.website_url,
                notes: provider.notes,
                icon: provider.icon,
                icon_color: provider.icon_color,
                sort_index: provider.sort_index,
                is_applied: provider.is_applied,
                is_disabled: provider.is_disabled,
                created_at: provider.created_at,
                updated_at: provider.updated_at,
            };
            put_claude_provider_to_sqlite(sqlite_state, &provider_id, &content)?;
        }
    }
    db.query(format!(
        "UPDATE claude_provider:`{}` SET is_disabled = $is_disabled, updated_at = $now",
        provider_id
    ))
    .bind(("is_disabled", is_disabled))
    .bind(("now", now))
    .await
    .map_err(|e| format!("Failed to toggle provider disabled status: {}", e))?;

    // If this provider is applied and now disabled, re-apply config to update files
    let toggle_record_id = db_record_id("claude_provider", &provider_id);
    let is_applied = if let Some(sqlite_state) = global_sqlite_state() {
        get_claude_provider_from_sqlite(sqlite_state, &provider_id)?
            .map(|provider| provider.is_applied)
            .unwrap_or(false)
    } else {
        let provider: Option<Value> = db
            .query(&format!(
                "SELECT *, type::string(id) as id FROM {}",
                toggle_record_id
            ))
            .await
            .map_err(|e| format!("Failed to query provider: {}", e))?
            .take(0)
            .map_err(|e| format!("Failed to parse provider: {}", e))?;

        provider
            .and_then(|provider_value| {
                provider_value
                    .get("is_applied")
                    .or_else(|| provider_value.get("isApplied"))
                    .and_then(|v| v.as_bool())
            })
            .unwrap_or(false)
    };

    if is_applied {
        // Re-apply config to update files (will check is_disabled internally)
        apply_config_internal(&db, &app, &provider_id, false).await?;
    }

    Ok(())
}

/// Apply Claude Code provider configuration to settings.json
#[tauri::command]
pub async fn apply_claude_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    provider_id: String,
) -> Result<(), String> {
    let db = state.db();
    apply_config_internal(&db, &app, &provider_id, false).await
}

/// Internal function to apply config: writes to file and updates database
/// This is the single source of truth for applying a Claude Code provider config
pub async fn apply_config_internal<R: tauri::Runtime>(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    app: &tauri::AppHandle<R>,
    provider_id: &str,
    from_tray: bool,
) -> Result<(), String> {
    // 应用配置到文件
    apply_config_to_file(db, provider_id).await?;

    // Update provider's is_applied status
    let now = Local::now().to_rfc3339();

    if let Some(sqlite_state) = global_sqlite_state() {
        for mut provider in list_claude_providers_from_sqlite(sqlite_state)? {
            let should_be_applied = provider.id == provider_id;
            if provider.is_applied == should_be_applied {
                continue;
            }
            let current_id = provider.id.clone();
            provider.is_applied = should_be_applied;
            provider.updated_at = now.clone();
            let content = ClaudeCodeProviderContent {
                name: provider.name,
                category: provider.category,
                settings_config: provider.settings_config,
                extra_settings_config: provider.extra_settings_config,
                source_provider_id: provider.source_provider_id,
                website_url: provider.website_url,
                notes: provider.notes,
                icon: provider.icon,
                icon_color: provider.icon_color,
                sort_index: provider.sort_index,
                is_applied: provider.is_applied,
                is_disabled: provider.is_disabled,
                created_at: provider.created_at,
                updated_at: provider.updated_at,
            };
            put_claude_provider_to_sqlite(sqlite_state, &current_id, &content)?;
        }
    }

    // Mark all providers as not applied (only update the currently applied one)
    db.query(
        "UPDATE claude_provider SET is_applied = false, updated_at = $now WHERE is_applied = true",
    )
    .bind(("now", now.clone()))
    .await
    .map_err(|e| format!("Failed to reset applied status: {}", e))?;

    // Mark target provider as applied
    let apply_record_id = db_record_id("claude_provider", provider_id);
    db.query(&format!(
        "UPDATE {} SET is_applied = true, updated_at = $now",
        apply_record_id
    ))
    .bind(("now", now))
    .await
    .map_err(|e| format!("Failed to set applied status: {}", e))?;

    // Notify based on source
    let payload = if from_tray { "tray" } else { "window" };
    let _ = app.emit("config-changed", payload);

    // Trigger WSL sync via event (Windows only)
    #[cfg(target_os = "windows")]
    let _ = app.emit("wsl-sync-request-claude", ());

    Ok(())
}

// ============================================================================
// Claude Prompt Config Commands
// ============================================================================

#[tauri::command]
pub async fn list_claude_prompt_configs(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<ClaudePromptConfig>, String> {
    let db = state.db();
    if let Some(sqlite_state) = global_sqlite_state() {
        let records = list_claude_prompts_from_sqlite(sqlite_state)?;
        if records.is_empty() {
            if let Some(local_config) = get_local_prompt_config(Some(&db)).await? {
                return Ok(vec![local_config]);
            }
        }
        return Ok(records);
    }

    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM claude_prompt_config")
        .await
        .map_err(|e| format!("Failed to query prompt configs: {}", e))?
        .take(0);

    match records_result {
        Ok(records) => {
            if records.is_empty() {
                if let Some(local_config) = get_local_prompt_config(Some(&db)).await? {
                    return Ok(vec![local_config]);
                }
                return Ok(Vec::new());
            }

            let mut result: Vec<ClaudePromptConfig> = records
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
        Err(e) => {
            eprintln!("Failed to deserialize Claude prompt configs: {}", e);
            if let Some(local_config) = get_local_prompt_config(Some(&db)).await? {
                return Ok(vec![local_config]);
            }
            Ok(Vec::new())
        }
    }
}

#[tauri::command]
pub async fn create_claude_prompt_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: ClaudePromptConfigInput,
) -> Result<ClaudePromptConfig, String> {
    let db = state.db();
    let now = Local::now().to_rfc3339();

    let next_sort_index = if let Some(sqlite_state) = global_sqlite_state() {
        sqlite_state.with_conn(|conn| {
            db_max_i64(
                conn,
                DbTable::ClaudePromptConfig,
                &JsonFieldPath::new("sort_index")?,
            )
        })?
    } else {
        let sort_index_result: Result<Vec<Value>, _> = db
            .query("SELECT sort_index FROM claude_prompt_config ORDER BY sort_index DESC LIMIT 1")
            .await
            .map_err(|e| format!("Failed to query prompt sort index: {}", e))?
            .take(0);

        sort_index_result
            .ok()
            .and_then(|records| records.first().cloned())
            .and_then(|record| record.get("sort_index").and_then(|value| value.as_i64()))
    }
    .map(|value| value as i32 + 1)
    .unwrap_or(0);

    let content = ClaudePromptConfigContent {
        name: input.name,
        content: input.content,
        is_applied: false,
        sort_index: Some(next_sort_index),
        created_at: now.clone(),
        updated_at: now,
    };

    let json_data = adapter::to_db_value_prompt(&content);
    let prompt_id = db_new_id();

    if let Some(sqlite_state) = global_sqlite_state() {
        put_claude_prompt_to_sqlite(sqlite_state, &prompt_id, &content)?;
    }
    put_claude_prompt_in_surreal(&db, &prompt_id, &content, true).await?;

    let created_config = adapter::from_db_value_prompt({
        let mut value = json_data;
        if let Some(object) = value.as_object_mut() {
            object.insert("id".to_string(), Value::String(prompt_id));
        }
        value
    });

    let _ = app.emit("config-changed", "window");

    Ok(created_config)
}

#[tauri::command]
pub async fn update_claude_prompt_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: ClaudePromptConfigInput,
) -> Result<ClaudePromptConfig, String> {
    let config_id = input
        .id
        .ok_or_else(|| "ID is required for update".to_string())?;
    let db = state.db();
    let record_id = db_record_id("claude_prompt_config", &config_id);

    let existing_prompt = if let Some(sqlite_state) = global_sqlite_state() {
        get_claude_prompt_from_sqlite(sqlite_state, &config_id)?
    } else {
        let existing_result: Result<Vec<Value>, _> = db
            .query(&format!(
                "SELECT *, type::string(id) as id FROM {} LIMIT 1",
                record_id
            ))
            .await
            .map_err(|e| format!("Failed to query prompt config: {}", e))?
            .take(0);

        existing_result
            .map_err(|e| format!("Failed to deserialize prompt config: {}", e))?
            .first()
            .cloned()
            .map(adapter::from_db_value_prompt)
    }
    .ok_or_else(|| format!("Prompt config '{}' not found", config_id))?;

    let created_at = existing_prompt
        .created_at
        .clone()
        .unwrap_or_else(|| Local::now().to_rfc3339());
    let is_applied = existing_prompt.is_applied;
    let sort_index = existing_prompt.sort_index;

    let now = Local::now().to_rfc3339();
    let content = ClaudePromptConfigContent {
        name: input.name,
        content: input.content.clone(),
        is_applied,
        sort_index,
        created_at,
        updated_at: now.clone(),
    };
    if let Some(sqlite_state) = global_sqlite_state() {
        put_claude_prompt_to_sqlite(sqlite_state, &config_id, &content)?;
    }
    put_claude_prompt_in_surreal(&db, &config_id, &content, false).await?;

    if is_applied {
        write_prompt_content_to_file(Some(&db), Some(input.content.as_str())).await?;
        emit_prompt_sync_requests(&app);
    }

    let _ = app.emit("config-changed", "window");

    Ok(ClaudePromptConfig {
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
pub async fn delete_claude_prompt_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let db = state.db();
    let record_id = db_record_id("claude_prompt_config", &id);
    if let Some(sqlite_state) = global_sqlite_state() {
        delete_claude_prompt_from_sqlite(sqlite_state, &id)?;
    }

    db.query(&format!("DELETE {}", record_id))
        .await
        .map_err(|e| format!("Failed to delete prompt config: {}", e))?;

    drop(db);
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
        emit_prompt_sync_requests(app);

        return Ok(());
    }

    let db = state.db();
    let record_id = db_record_id("claude_prompt_config", config_id);
    let prompt_config = if let Some(sqlite_state) = global_sqlite_state() {
        get_claude_prompt_from_sqlite(sqlite_state, config_id)?
            .ok_or_else(|| format!("Prompt config '{}' not found", config_id))?
    } else {
        let records_result: Result<Vec<Value>, _> = db
            .query(&format!(
                "SELECT *, type::string(id) as id FROM {} LIMIT 1",
                record_id
            ))
            .await
            .map_err(|e| format!("Failed to query prompt config: {}", e))?
            .take(0);

        match records_result {
            Ok(records) => {
                if let Some(record) = records.first() {
                    adapter::from_db_value_prompt(record.clone())
                } else {
                    return Err(format!("Prompt config '{}' not found", config_id));
                }
            }
            Err(e) => return Err(format!("Failed to deserialize prompt config: {}", e)),
        }
    };

    let now = Local::now().to_rfc3339();

    if let Some(sqlite_state) = global_sqlite_state() {
        for mut prompt in list_claude_prompts_from_sqlite(sqlite_state)? {
            let should_be_applied = prompt.id == config_id;
            if prompt.is_applied == should_be_applied {
                continue;
            }
            let prompt_id = prompt.id.clone();
            prompt.is_applied = should_be_applied;
            let content = ClaudePromptConfigContent {
                name: prompt.name,
                content: prompt.content,
                is_applied: prompt.is_applied,
                sort_index: prompt.sort_index,
                created_at: prompt.created_at.unwrap_or_else(|| now.clone()),
                updated_at: now.clone(),
            };
            put_claude_prompt_to_sqlite(sqlite_state, &prompt_id, &content)?;
        }
    }

    db.query("UPDATE claude_prompt_config SET is_applied = false, updated_at = $now WHERE is_applied = true")
        .bind(("now", now.clone()))
        .await
        .map_err(|e| format!("Failed to clear prompt applied flags: {}", e))?;

    db.query(&format!(
        "UPDATE {} SET is_applied = true, updated_at = $now",
        record_id
    ))
    .bind(("now", now))
    .await
    .map_err(|e| format!("Failed to set prompt applied flag: {}", e))?;

    drop(db);

    let db = state.db();
    write_prompt_content_to_file(Some(&db), Some(prompt_config.content.as_str())).await?;

    let payload = if from_tray { "tray" } else { "window" };
    let _ = app.emit("config-changed", payload);
    emit_prompt_sync_requests(app);

    Ok(())
}

#[tauri::command]
pub async fn apply_claude_prompt_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    config_id: String,
) -> Result<(), String> {
    apply_prompt_config_internal(state, &app, &config_id, false).await
}

#[tauri::command]
pub async fn reorder_claude_prompt_configs(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    ids: Vec<String>,
) -> Result<(), String> {
    let db = state.db();

    if let Some(sqlite_state) = global_sqlite_state() {
        let now = Local::now().to_rfc3339();
        for (index, id) in ids.iter().enumerate() {
            if let Some(prompt) = get_claude_prompt_from_sqlite(sqlite_state, id)? {
                let content = ClaudePromptConfigContent {
                    name: prompt.name,
                    content: prompt.content,
                    is_applied: prompt.is_applied,
                    sort_index: Some(index as i32),
                    created_at: prompt.created_at.unwrap_or_else(|| now.clone()),
                    updated_at: prompt.updated_at.unwrap_or_else(|| now.clone()),
                };
                put_claude_prompt_to_sqlite(sqlite_state, id, &content)?;
            }
        }
    }

    for (index, id) in ids.iter().enumerate() {
        let record_id = db_record_id("claude_prompt_config", id);
        db.query(&format!("UPDATE {} SET sort_index = $index", record_id))
            .bind(("index", index as i32))
            .await
            .map_err(|e| format!("Failed to update prompt sort index: {}", e))?;
    }

    drop(db);
    let _ = app.emit("config-changed", "window");

    Ok(())
}

#[tauri::command]
pub async fn save_claude_local_prompt_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: ClaudePromptConfigInput,
) -> Result<ClaudePromptConfig, String> {
    let prompt_content = if input.content.trim().is_empty() {
        let db = state.db();
        get_local_prompt_config(Some(&db))
            .await?
            .map(|config| config.content)
            .unwrap_or_default()
    } else {
        input.content
    };

    let created = create_claude_prompt_config(
        state.clone(),
        app.clone(),
        ClaudePromptConfigInput {
            id: None,
            name: input.name,
            content: prompt_content,
        },
    )
    .await?;

    apply_prompt_config_internal(state.clone(), &app, &created.id, false).await?;

    let db = state.db();
    let record_id = db_record_id("claude_prompt_config", &created.id);
    let refreshed_result: Result<Vec<Value>, _> = db
        .query(&format!(
            "SELECT *, type::string(id) as id FROM {} LIMIT 1",
            record_id
        ))
        .await
        .map_err(|e| format!("Failed to query saved local prompt config: {}", e))?
        .take(0);

    match refreshed_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                Ok(adapter::from_db_value_prompt(record.clone()))
            } else {
                Ok(created)
            }
        }
        Err(_) => Ok(created),
    }
}

// ============================================================================
// Claude Common Config Commands
// ============================================================================

/// Get Claude common config
#[tauri::command]
pub async fn get_claude_common_config(
    state: tauri::State<'_, DbState>,
) -> Result<Option<ClaudeCommonConfig>, String> {
    let db = state.db();
    if let Some(sqlite_state) = global_sqlite_state() {
        if let Some(config) = get_claude_common_from_sqlite(sqlite_state)? {
            return Ok(Some(config));
        }
        if let Ok(temp_common) = load_temp_common_config_from_file_with_db(&db).await {
            return Ok(Some(temp_common));
        }
        return Ok(None);
    }

    let records_result: Result<Vec<Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM claude_common_config:`common` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query common config: {}", e))?
        .take(0);

    match records_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                Ok(Some(adapter::from_db_value_common(record.clone())))
            } else {
                // Database is empty, try to load from local file
                if let Ok(temp_common) = load_temp_common_config_from_file_with_db(&db).await {
                    Ok(Some(temp_common))
                } else {
                    Ok(None)
                }
            }
        }
        Err(e) => {
            // Try to load from local file as fallback
            if let Ok(temp_common) = load_temp_common_config_from_file_with_db(&db).await {
                Ok(Some(temp_common))
            } else {
                // 反序列化失败，删除旧数据以修复版本冲突
                eprintln!(
                    "⚠️ Claude common config has incompatible format, cleaning up: {}",
                    e
                );
                let _ = db.query("DELETE claude_common_config:`common`").await;
                let _ = runtime_location::refresh_runtime_location_cache_for_module_async(
                    &db, "claude",
                )
                .await;
                Ok(None)
            }
        }
    }
}

#[tauri::command]
pub async fn extract_claude_common_config_from_current_file(
    state: tauri::State<'_, DbState>,
) -> Result<ClaudeCommonConfig, String> {
    let db = state.db();
    load_temp_common_config_from_file_with_db(&db).await
}

/// Load a temporary common config from settings.json without writing to database
/// This extracts non-env fields and unknown env fields from settings.json
/// Save Claude common config
#[tauri::command]
pub async fn save_claude_common_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: ClaudeCommonConfigInput,
) -> Result<(), String> {
    let db = state.db();
    let previous_skills_path =
        runtime_location::get_tool_skills_path_async(&db, "claude_code").await;

    // Validate JSON
    let _: serde_json::Value =
        serde_json::from_str(&input.config).map_err(|e| format!("Invalid JSON: {}", e))?;

    let existing_common = get_claude_common_config(state.clone()).await?;
    let previous_common_config_value = existing_common
        .as_ref()
        .map(|config| settings_merge::parse_json_object(&config.config).map(Value::Object))
        .transpose()?;
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
    if let Some(sqlite_state) = global_sqlite_state() {
        put_claude_common_to_sqlite(sqlite_state, &input.config, root_dir.as_deref())?;
    }
    put_claude_common_in_surreal(&db, &input.config, root_dir.as_deref()).await?;
    runtime_location::refresh_runtime_location_cache_for_module_async(&db, "claude").await?;

    // 查找当前应用的 provider，如果存在则重新应用到文件
    let applied_provider = if let Some(sqlite_state) = global_sqlite_state() {
        list_claude_providers_from_sqlite(sqlite_state)?
            .into_iter()
            .find(|provider| provider.is_applied)
    } else {
        let applied_result: Result<Vec<Value>, _> = db
            .query(
                "SELECT *, type::string(id) as id FROM claude_provider WHERE is_applied = true LIMIT 1",
            )
            .await
            .map_err(|e| format!("Failed to query applied provider: {}", e))?
            .take(0);

        applied_result
            .ok()
            .and_then(|records| records.first().cloned())
            .map(adapter::from_db_value_provider)
    };

    if let Some(applied_provider) = applied_provider {
        // 重新应用配置到文件（不改变数据库中的 is_applied 状态）
        if let Err(e) = apply_config_to_file_with_previous_common_config(
            &db,
            &applied_provider.id,
            previous_common_config_value.clone(),
        )
        .await
        {
            eprintln!(
                "Failed to auto-apply config after common config update: {}",
                e
            );
            // 不中断保存流程，只记录错误
        } else {
            #[cfg(target_os = "windows")]
            let _ = app.emit("wsl-sync-request-claude", ());
        }
    }

    resync_all_skills_if_tool_path_changed(
        app.clone(),
        state.inner(),
        "claude_code",
        previous_skills_path,
    )
    .await;

    // Notify frontend to refresh
    let _ = app.emit("config-changed", "window");

    Ok(())
}

/// Save local config (provider and/or common) into database
/// Input can include provider and/or commonConfig; missing parts will be loaded from settings.json
#[tauri::command]
pub async fn save_claude_local_config(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: ClaudeLocalConfigInput,
) -> Result<(), String> {
    let db = state.db();
    let previous_skills_path =
        runtime_location::get_tool_skills_path_async(&db, "claude_code").await;

    // Load base provider/common from local settings
    let base_provider = load_temp_provider_from_file_with_db(&db).await?;
    let base_common = load_temp_common_config_from_file_with_db(&db).await.ok();

    let provider_input = input.provider;
    let provider_name = provider_input
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or(base_provider.name);
    let provider_category = provider_input
        .as_ref()
        .map(|p| p.category.clone())
        .unwrap_or(base_provider.category);
    let provider_settings_config = provider_input
        .as_ref()
        .map(|p| p.settings_config.clone())
        .unwrap_or(base_provider.settings_config);
    let provider_extra_settings_config = provider_input
        .as_ref()
        .and_then(|p| p.extra_settings_config.clone())
        .unwrap_or(base_provider.extra_settings_config);
    let provider_source_id = provider_input
        .as_ref()
        .and_then(|p| p.source_provider_id.clone());
    let provider_notes = provider_input
        .as_ref()
        .and_then(|p| p.notes.clone())
        .or(base_provider.notes);
    let provider_sort_index = provider_input
        .as_ref()
        .and_then(|p| p.sort_index)
        .or(base_provider.sort_index);

    let common_config = if let Some(config) = input.common_config {
        // Validate JSON
        let _: serde_json::Value =
            serde_json::from_str(&config).map_err(|e| format!("Invalid JSON: {}", e))?;
        config
    } else if let Some(common) = base_common.as_ref() {
        common.config.clone()
    } else {
        "{}".to_string()
    };
    let previous_common_config_value = base_common
        .as_ref()
        .map(|config| settings_merge::parse_json_object(&config.config).map(Value::Object))
        .transpose()?;
    let next_common_config_value = parse_optional_common_config_value(Some(&common_config))?;

    let now = Local::now().to_rfc3339();
    let normalized_provider_settings_config = normalize_provider_settings_for_storage(
        &db,
        &provider_settings_config,
        next_common_config_value.as_ref(),
    )
    .await?;
    let normalized_extra_settings_config = normalize_extra_settings_config_for_storage(
        &provider_category,
        Some(&provider_extra_settings_config),
    )?;
    let provider_content = ClaudeCodeProviderContent {
        name: provider_name,
        category: provider_category,
        settings_config: normalized_provider_settings_config,
        extra_settings_config: normalized_extra_settings_config,
        source_provider_id: provider_source_id,
        website_url: None,
        notes: provider_notes,
        icon: None,
        icon_color: None,
        sort_index: provider_sort_index,
        is_applied: true,
        is_disabled: false,
        created_at: now.clone(),
        updated_at: now,
    };

    let provider_id = db_new_id();
    if let Some(sqlite_state) = global_sqlite_state() {
        put_claude_provider_to_sqlite(sqlite_state, &provider_id, &provider_content)?;
    }
    create_claude_provider_in_surreal(&db, &provider_id, &provider_content).await?;

    let root_dir = if input.clear_root_dir {
        None
    } else {
        let trimmed_root_dir = input
            .root_dir
            .as_deref()
            .map(str::trim)
            .filter(|dir| !dir.is_empty())
            .map(str::to_string);
        if trimmed_root_dir.is_some() {
            trimmed_root_dir
        } else {
            get_claude_custom_root_dir_async(&db)
                .await
                .map(|path| path.to_string_lossy().to_string())
        }
    };
    if let Some(sqlite_state) = global_sqlite_state() {
        put_claude_common_to_sqlite(sqlite_state, &common_config, root_dir.as_deref())?;
    }
    put_claude_common_in_surreal(&db, &common_config, root_dir.as_deref()).await?;
    runtime_location::refresh_runtime_location_cache_for_module_async(&db, "claude").await?;

    // Re-apply config to file using the newly created provider
    if let Err(e) = apply_config_to_file_with_previous_common_config(
        &db,
        &provider_id,
        previous_common_config_value.clone(),
    )
    .await
    {
        eprintln!("Failed to apply config after local save: {}", e);
    } else {
        #[cfg(target_os = "windows")]
        let _ = app.emit("wsl-sync-request-claude", ());
    }

    resync_all_skills_if_tool_path_changed(
        app.clone(),
        state.inner(),
        "claude_code",
        previous_skills_path,
    )
    .await;

    let _ = app.emit("config-changed", "window");
    Ok(())
}

pub async fn sync_sqlite_claude_from_surreal_if_missing(
    sqlite_state: &SqliteDbState,
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<(), String> {
    let mut missing_tables = Vec::new();
    for table in [
        DbTable::ClaudeProvider,
        DbTable::ClaudeCommonConfig,
        DbTable::ClaudePromptConfig,
    ] {
        let sqlite_count = sqlite_state.with_conn(|conn| db_count(conn, table))?;
        if sqlite_count == 0 {
            missing_tables.push(table);
        }
    }

    if missing_tables.is_empty() {
        return Ok(());
    }

    crate::db::surreal_import::import_tables_from_surreal(sqlite_state, db, &missing_tables)
        .await?;
    Ok(())
}

// ============================================================================
// Claude Plugin Integration Commands
// ============================================================================

/// Check if plugin config has primaryApiKey = "any"
fn is_plugin_config_enabled(content: &str) -> bool {
    match serde_json::from_str::<serde_json::Value>(content) {
        Ok(value) => value
            .get("primaryApiKey")
            .and_then(|v| v.as_str())
            .map(|val| val == "any")
            .unwrap_or(false),
        Err(_) => false,
    }
}

fn emit_claude_plugin_config_changed<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let _ = app.emit("config-changed", "window");
    let _ = app.emit("mcp-changed", "window");
    let _ = app.emit("skills-changed", "window");

    #[cfg(target_os = "windows")]
    let _ = app.emit("wsl-sync-request-claude", ());
}

/// Get Claude plugin integration status
#[tauri::command]
pub async fn get_claude_plugin_status(
    state: tauri::State<'_, DbState>,
) -> Result<ClaudePluginStatus, String> {
    let db = state.db();
    let config_path = get_claude_plugin_config_path_from_db_async(&db).await?;
    let has_config_file = config_path.exists();

    if !has_config_file {
        return Ok(ClaudePluginStatus {
            enabled: false,
            has_config_file: false,
        });
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;

    let enabled = is_plugin_config_enabled(&content);

    Ok(ClaudePluginStatus {
        enabled,
        has_config_file: true,
    })
}

/// Apply Claude plugin configuration
#[tauri::command]
pub async fn apply_claude_plugin_config(
    state: tauri::State<'_, DbState>,
    enabled: bool,
) -> Result<bool, String> {
    let db = state.db();
    let config_path = get_claude_plugin_config_path_from_db_async(&db).await?;

    // Ensure directory exists
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create .claude directory: {}", e))?;
        }
    }

    // Read existing config or create empty
    let mut obj: serde_json::Map<String, serde_json::Value> = if config_path.exists() {
        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;

        match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(serde_json::Value::Object(map)) => map,
            _ => serde_json::Map::new(),
        }
    } else {
        serde_json::Map::new()
    };

    if enabled {
        // Set primaryApiKey = "any"
        let current = obj
            .get("primaryApiKey")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if current != "any" {
            obj.insert(
                "primaryApiKey".to_string(),
                serde_json::Value::String("any".to_string()),
            );
        }
    } else {
        // Remove primaryApiKey field
        obj.remove("primaryApiKey");
    }

    // Write back to file
    let serialized = serde_json::to_string_pretty(&serde_json::Value::Object(obj))
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    fs::write(&config_path, format!("{serialized}\n"))
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    Ok(true)
}

// ============================================================================
// Claude Plugins Marketplace Commands
// ============================================================================

#[tauri::command]
pub async fn get_claude_plugin_runtime_status(
    state: tauri::State<'_, DbState>,
) -> Result<super::plugin_types::ClaudePluginRuntimeStatus, String> {
    let db = state.db();
    plugin_state::get_claude_plugin_runtime_status(&db).await
}

#[tauri::command]
pub async fn list_claude_installed_plugins(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<super::plugin_types::ClaudeInstalledPlugin>, String> {
    let db = state.db();
    plugin_state::list_claude_installed_plugins(&db).await
}

#[tauri::command]
pub async fn list_claude_known_marketplaces(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<super::plugin_types::ClaudeKnownMarketplace>, String> {
    let db = state.db();
    plugin_state::list_claude_known_marketplaces(&db).await
}

#[tauri::command]
pub async fn list_claude_marketplace_plugins(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<super::plugin_types::ClaudeMarketplacePlugin>, String> {
    let db = state.db();
    plugin_state::list_claude_marketplace_plugins(&db).await
}

#[tauri::command]
pub async fn add_claude_marketplace(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: ClaudeMarketplaceAddInput,
) -> Result<(), String> {
    let db = state.db();
    plugin_state::run_claude_marketplace_command_preserving_auto_update(
        &db,
        move |runtime_location| async move {
            plugin_cli::run_claude_plugin_command(
                &runtime_location,
                &["plugin", "marketplace", "add", &input.source],
            )
            .await
        },
    )
    .await?;
    emit_claude_plugin_config_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn update_claude_marketplace(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: ClaudeMarketplaceUpdateInput,
) -> Result<(), String> {
    let db = state.db();
    plugin_state::run_claude_marketplace_command_preserving_auto_update(
        &db,
        move |runtime_location| async move {
            let mut args = vec!["plugin", "marketplace", "update"];
            if let Some(marketplace_name) = input.marketplace_name.as_deref() {
                args.push(marketplace_name);
            }
            plugin_cli::run_claude_plugin_command(&runtime_location, &args).await
        },
    )
    .await?;
    emit_claude_plugin_config_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn set_claude_marketplace_auto_update(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: ClaudeMarketplaceAutoUpdateInput,
) -> Result<(), String> {
    let db = state.db();
    plugin_state::set_claude_marketplace_auto_update_enabled(
        &db,
        &input.marketplace_name,
        input.auto_update_enabled,
    )
    .await?;
    emit_claude_plugin_config_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn remove_claude_marketplace(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: ClaudeMarketplaceRemoveInput,
) -> Result<(), String> {
    let db = state.db();
    plugin_state::run_claude_marketplace_command_preserving_auto_update(
        &db,
        move |runtime_location| async move {
            plugin_cli::run_claude_plugin_command(
                &runtime_location,
                &["plugin", "marketplace", "remove", &input.marketplace_name],
            )
            .await
        },
    )
    .await?;
    emit_claude_plugin_config_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn install_claude_plugin_user_scope(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: ClaudePluginActionInput,
) -> Result<(), String> {
    let db = state.db();
    let runtime_location = runtime_location::get_claude_runtime_location_async(&db).await?;
    plugin_cli::run_claude_plugin_command(
        &runtime_location,
        &["plugin", "install", &input.plugin_id, "--scope", "user"],
    )
    .await?;
    emit_claude_plugin_config_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn enable_claude_plugin_user_scope(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: ClaudePluginActionInput,
) -> Result<(), String> {
    let db = state.db();
    let runtime_location = runtime_location::get_claude_runtime_location_async(&db).await?;
    plugin_cli::run_claude_plugin_command(
        &runtime_location,
        &["plugin", "enable", &input.plugin_id, "--scope", "user"],
    )
    .await?;
    emit_claude_plugin_config_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn disable_claude_plugin_user_scope(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: ClaudePluginActionInput,
) -> Result<(), String> {
    let db = state.db();
    let runtime_location = runtime_location::get_claude_runtime_location_async(&db).await?;
    plugin_cli::run_claude_plugin_command(
        &runtime_location,
        &["plugin", "disable", &input.plugin_id, "--scope", "user"],
    )
    .await?;
    emit_claude_plugin_config_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn update_claude_plugin_user_scope(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: ClaudePluginActionInput,
) -> Result<(), String> {
    let db = state.db();
    let runtime_location = runtime_location::get_claude_runtime_location_async(&db).await?;
    plugin_cli::run_claude_plugin_command(
        &runtime_location,
        &["plugin", "update", &input.plugin_id, "--scope", "user"],
    )
    .await?;
    emit_claude_plugin_config_changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn uninstall_claude_plugin_user_scope(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
    input: ClaudePluginActionInput,
) -> Result<(), String> {
    let db = state.db();
    let runtime_location = runtime_location::get_claude_runtime_location_async(&db).await?;
    plugin_cli::run_claude_plugin_command(
        &runtime_location,
        &["plugin", "uninstall", &input.plugin_id, "--scope", "user"],
    )
    .await?;
    emit_claude_plugin_config_changed(&app);
    Ok(())
}

// ============================================================================
// Claude Code Initialization Commands
// ============================================================================

/// Known fields in provider settings config (env section)

/// Initialize Claude provider from settings.json if database is empty
/// This function reads the settings.json file and imports its configuration
/// as a default provider if no providers exist in the database.
pub async fn init_claude_provider_from_settings(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<(), String> {
    // Check if any providers exist by querying for one record
    let check_result: Result<Vec<Value>, _> = db
        .query("SELECT * OMIT id FROM claude_provider LIMIT 1")
        .await
        .map_err(|e| format!("Failed to check providers: {}", e))?
        .take(0);

    let has_providers = match check_result {
        Ok(records) => !records.is_empty(),
        Err(_) => false,
    };

    if has_providers {
        // Already have providers, skip initialization
        return Ok(());
    }

    // Read settings.json
    let settings_value = read_current_claude_settings_value_async(db).await?;
    let Some(settings_value) = settings_value else {
        // No settings file, nothing to import
        return Ok(());
    };

    // Check if settings has env section with ANTHROPIC fields
    let settings_obj = match settings_value.as_object() {
        Some(obj) => obj,
        None => return Ok(()), // Not a valid object, skip
    };

    let env_obj = match settings_obj.get("env").and_then(|v| v.as_object()) {
        Some(env) => env,
        None => return Ok(()), // No env section, skip
    };

    // Check if there are any ANTHROPIC fields
    let has_anthropic_config = env_obj.keys().any(|k| k.starts_with("ANTHROPIC_"));
    if !has_anthropic_config {
        return Ok(()); // No ANTHROPIC config, skip
    }

    let (provider_settings, common_config) =
        settings_merge::split_settings_into_provider_and_common(
            &settings_value,
            &KNOWN_ENV_FIELDS,
        )?;

    // Save common config if not empty
    if common_config
        .as_object()
        .map(|config| !config.is_empty())
        .unwrap_or(false)
    {
        let common_json = serde_json::to_string(&common_config)
            .map_err(|e| format!("Failed to serialize common config: {}", e))?;

        let common_db_data = adapter::to_db_value_common(&common_json, None);

        // Use UPSERT to create if not exists, update if exists
        db.query("UPSERT claude_common_config:`common` CONTENT $data")
            .bind(("data", common_db_data))
            .await
            .map_err(|e| format!("Failed to save common config: {}", e))?;
    }

    // Create default provider
    let now = Local::now().to_rfc3339();
    let provider_name = "默认配置";

    let content = ClaudeCodeProviderContent {
        name: provider_name.to_string(),
        category: infer_claude_provider_category_from_settings(&provider_settings),
        settings_config: serde_json::to_string(&provider_settings)
            .map_err(|e| format!("Failed to serialize provider settings: {}", e))?,
        extra_settings_config: "{}".to_string(),
        source_provider_id: None,
        website_url: None,
        notes: Some("从 settings.json 自动导入".to_string()),
        icon: None,
        icon_color: None,
        sort_index: Some(0),
        is_applied: true,
        is_disabled: false,
        created_at: now.clone(),
        updated_at: now,
    };

    let json_data = adapter::to_db_value_provider(&content);

    // Create new provider with auto-generated random ID
    db.query("CREATE claude_provider CONTENT $data")
        .bind(("data", json_data))
        .await
        .map_err(|e| format!("Failed to create default provider: {}", e))?;

    println!("✅ Imported Claude Code settings from settings.json as default provider");

    Ok(())
}

// ============================================================================
// Claude Code Onboarding Commands
// ============================================================================

async fn get_claude_mcp_config_path(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<std::path::PathBuf, String> {
    runtime_location::get_claude_mcp_config_path_async(db).await
}

/// Get Claude onboarding status
/// Returns true if hasCompletedOnboarding is set to true in ~/.claude.json
#[tauri::command]
pub async fn get_claude_onboarding_status(
    state: tauri::State<'_, DbState>,
) -> Result<bool, String> {
    let db = state.db();
    let config_path = get_claude_mcp_config_path(&db).await?;

    if !config_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;

    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config file: {}", e))?;

    let status = value
        .get("hasCompletedOnboarding")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Ok(status)
}

/// Skip Claude Code initial setup confirmation
/// Writes hasCompletedOnboarding=true to ~/.claude.json
#[tauri::command]
pub async fn apply_claude_onboarding_skip(
    state: tauri::State<'_, DbState>,
) -> Result<bool, String> {
    let db = state.db();
    let config_path = get_claude_mcp_config_path(&db).await?;

    // Ensure directory exists
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
        }
    }

    // Read existing config or create empty object
    let mut obj: serde_json::Map<String, serde_json::Value> = if config_path.exists() {
        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;

        match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(serde_json::Value::Object(map)) => map,
            _ => serde_json::Map::new(),
        }
    } else {
        serde_json::Map::new()
    };

    // Check if already set
    let already = obj
        .get("hasCompletedOnboarding")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if already {
        return Ok(false);
    }

    // Set hasCompletedOnboarding = true
    obj.insert(
        "hasCompletedOnboarding".to_string(),
        serde_json::Value::Bool(true),
    );

    // Write back to file
    let serialized = serde_json::to_string_pretty(&serde_json::Value::Object(obj))
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    fs::write(&config_path, format!("{serialized}\n"))
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    Ok(true)
}

/// Restore Claude Code initial setup confirmation
/// Removes hasCompletedOnboarding field from ~/.claude.json
#[tauri::command]
pub async fn clear_claude_onboarding_skip(
    state: tauri::State<'_, DbState>,
) -> Result<bool, String> {
    let db = state.db();
    let config_path = get_claude_mcp_config_path(&db).await?;

    if !config_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;

    let mut obj: serde_json::Map<String, serde_json::Value> =
        match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(serde_json::Value::Object(map)) => map,
            _ => return Ok(false),
        };

    // Check if field exists
    let existed = obj.remove("hasCompletedOnboarding").is_some();

    if !existed {
        return Ok(false);
    }

    // Write back to file
    let serialized = serde_json::to_string_pretty(&serde_json::Value::Object(obj))
        .map_err(|e| format!("Failed to serialize config: {}", e))?;

    fs::write(&config_path, format!("{serialized}\n"))
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    Ok(true)
}

#[tauri::command]
pub async fn list_claude_all_api_hub_providers(
    state: tauri::State<'_, DbState>,
) -> Result<ClaudeAllApiHubProvidersResult, String> {
    let _ = state;
    let discovery = all_api_hub::list_provider_candidates()?;

    let providers = discovery
        .providers
        .iter()
        .map(|candidate| ClaudeAllApiHubProvider {
            provider_id: candidate.provider_id.clone(),
            name: candidate.name.clone(),
            npm: Some(candidate.npm.clone()),
            base_url: candidate.base_url.clone(),
            requires_browser_open: candidate
                .auth_type
                .as_deref()
                .map(|value| value.trim().eq_ignore_ascii_case("cookie"))
                .unwrap_or(false),
            is_disabled: candidate.is_disabled,
            has_api_key: candidate
                .api_key
                .as_ref()
                .map(|v| !v.is_empty())
                .unwrap_or(false),
            api_key_preview: candidate
                .api_key
                .as_ref()
                .map(|value| all_api_hub::mask_api_key_preview(value)),
            balance_usd: candidate.balance_usd,
            balance_cny: candidate.balance_cny,
            site_name: candidate.site_name.clone(),
            site_type: candidate.site_type.clone(),
            account_label: candidate.account_label.clone(),
            source_profile_name: candidate.source_profile_name.clone(),
            source_extension_id: candidate.source_extension_id.clone(),
            provider_config: serde_json::to_value(all_api_hub::candidate_to_opencode_provider(
                candidate,
            ))
            .unwrap_or_else(|_| serde_json::json!({})),
        })
        .collect();

    Ok(ClaudeAllApiHubProvidersResult {
        found: discovery.found,
        profiles: discovery.profiles,
        providers,
        message: discovery.message,
    })
}

#[tauri::command]
pub async fn resolve_claude_all_api_hub_providers(
    state: tauri::State<'_, DbState>,
    request: ResolveClaudeAllApiHubProvidersRequest,
) -> Result<Vec<ClaudeAllApiHubProvider>, String> {
    let providers =
        all_api_hub::resolve_provider_candidates_with_keys(&state, &request.provider_ids).await?;

    Ok(providers
        .iter()
        .map(|candidate| ClaudeAllApiHubProvider {
            provider_id: candidate.provider_id.clone(),
            name: candidate.name.clone(),
            npm: Some(candidate.npm.clone()),
            base_url: candidate.base_url.clone(),
            requires_browser_open: candidate
                .auth_type
                .as_deref()
                .map(|value| value.trim().eq_ignore_ascii_case("cookie"))
                .unwrap_or(false),
            is_disabled: candidate.is_disabled,
            has_api_key: candidate
                .api_key
                .as_ref()
                .map(|v| !v.is_empty())
                .unwrap_or(false),
            api_key_preview: candidate
                .api_key
                .as_ref()
                .map(|value| all_api_hub::mask_api_key_preview(value)),
            balance_usd: candidate.balance_usd,
            balance_cny: candidate.balance_cny,
            site_name: candidate.site_name.clone(),
            site_type: candidate.site_type.clone(),
            account_label: candidate.account_label.clone(),
            source_profile_name: candidate.source_profile_name.clone(),
            source_extension_id: candidate.source_extension_id.clone(),
            provider_config: serde_json::to_value(all_api_hub::candidate_to_opencode_provider(
                candidate,
            ))
            .unwrap_or_else(|_| serde_json::json!({})),
        })
        .collect())
}
