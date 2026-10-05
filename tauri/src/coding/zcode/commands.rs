//! ZCode backend commands.
//!
//! ZCode has no single "active provider": every enabled provider shows up in
//! the model picker at once. "Apply" therefore means writing the provider rule,
//! enabling it, and pointing `defaultModelSelection` at it.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use chrono::Local;
use serde_json::{json, Value};
use tauri::Emitter;
use tempfile::NamedTempFile;
use tokio::sync::Mutex as AsyncMutex;

use crate::db::helpers::{
    db_delete, db_get, db_list, db_max_i64, db_patch_fields, db_put, db_update_applied_status,
};
use crate::db::schema::{DbTable, JsonFieldPath, OrderDirection, OrderField, OrderSpec};
use crate::db::SqliteDbState;

use super::constants::*;
use super::projection;
use super::types::{
    ConfigPathInfo, ZcodeCommonConfigInput, ZcodePromptConfig, ZcodePromptConfigInput,
    ZcodeProviderInput,
};

/// Serializes every read-modify-write pass over `provider_config.json`.
///
/// Each pass builds the next document from its own read snapshot; two
/// concurrent writes would otherwise project from the same stale snapshot and
/// the later one would drop the earlier one's provider. Entry points hold this
/// across the whole "read disk -> write DB -> project" window; `_locked`
/// bodies must never acquire it (it is not reentrant).
static CONFIG_WRITE_LOCK: LazyLock<AsyncMutex<()>> = LazyLock::new(|| AsyncMutex::new(()));

fn provider_order() -> Result<OrderSpec, String> {
    Ok(OrderSpec::new(vec![
        OrderField::json_integer("sort_index", OrderDirection::Asc)?,
        OrderField::created_at(OrderDirection::Asc),
    ]))
}

fn prompt_order() -> Result<OrderSpec, String> {
    provider_order()
}

fn write_text_atomic(path: &Path, content: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|error| format!("Failed to create temp file for {}: {error}", path.display()))?;
    temporary
        .write_all(content.as_bytes())
        .map_err(|error| format!("Failed to write temp file for {}: {error}", path.display()))?;
    temporary
        .persist(path)
        .map_err(|error| format!("Failed to persist {}: {error}", path.display()))?;
    Ok(())
}

fn zcode_root_dir_from_db(state: &SqliteDbState) -> Result<Option<String>, String> {
    let record = state.with_conn(|conn| {
        crate::db::helpers::db_get(conn, DbTable::ZcodeCommonConfig, "common")
    })?;
    Ok(record
        .map(super::adapter::from_db_value_common)
        .and_then(|record| record.root_dir)
        .filter(|value| !value.trim().is_empty()))
}

/// Resolves the effective ZCode data root.
///
/// Precedence: in-app custom root, then the `ZCODE_DATA_BASE_DIR` process
/// override, then the user's home directory. The desktop app persists its own
/// `dataBaseDir` inside `setting.json`, which is read separately when the
/// process override is absent.
pub fn resolve_zcode_root_dir(state: &SqliteDbState) -> Result<PathBuf, String> {
    if let Some(root) = zcode_root_dir_from_db(state)? {
        return Ok(PathBuf::from(root));
    }
    if let Ok(value) = std::env::var(ZCODE_DATA_BASE_DIR_ENV) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed).join(ZCODE_DEFAULT_ROOT_DIR_NAME));
        }
    }
    let home = dirs::home_dir().ok_or_else(|| "Cannot resolve home directory".to_string())?;
    Ok(home.join(ZCODE_DEFAULT_ROOT_DIR_NAME))
}

pub fn zcode_provider_config_path(state: &SqliteDbState) -> Result<PathBuf, String> {
    Ok(resolve_zcode_root_dir(state)?.join(ZCODE_PROVIDER_CONFIG_RELATIVE_PATH))
}

pub fn zcode_prompt_path(state: &SqliteDbState) -> Result<PathBuf, String> {
    Ok(resolve_zcode_root_dir(state)?.join(ZCODE_PROMPT_FILE_NAME))
}

#[tauri::command]
pub async fn get_zcode_config_file_path(state: tauri::State<'_, SqliteDbState>) -> Result<String, String> {
    Ok(zcode_provider_config_path(&state)?.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn get_zcode_root_path_info(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<ConfigPathInfo, String> {
    let custom = zcode_root_dir_from_db(&state)?;
    let source = if custom.is_some() {
        "custom"
    } else if std::env::var(ZCODE_DATA_BASE_DIR_ENV)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
    {
        "env"
    } else {
        "default"
    };
    Ok(ConfigPathInfo {
        path: resolve_zcode_root_dir(&state)?.to_string_lossy().to_string(),
        source: source.to_string(),
    })
}

#[tauri::command]
pub async fn reveal_zcode_config_folder(state: tauri::State<'_, SqliteDbState>) -> Result<(), String> {
    let root = resolve_zcode_root_dir(&state)?;
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    tauri_plugin_opener::reveal_item_in_dir(&root).map_err(|error| error.to_string())
}

/// Reads the live provider registry so the page can show what ZCode sees.
#[tauri::command]
pub async fn read_zcode_settings(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<Value, String> {
    let path = zcode_provider_config_path(&state)?;
    Ok(projection::read_provider_config_base(&path))
}

/// Whether the user has migrated to the new-generation registry.
///
/// When this is false ZCode still reads the legacy `config.json` provider map,
/// so applying a provider would silently have no effect.
#[tauri::command]
pub async fn get_zcode_generation_status(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<bool, String> {
    let path = zcode_provider_config_path(&state)?;
    Ok(projection::provider_config_exists(&path))
}

#[tauri::command]
pub async fn get_zcode_common_config(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<Value, String> {
    let record = state.with_conn(|conn| {
        crate::db::helpers::db_get(conn, DbTable::ZcodeCommonConfig, "common")
    })?;
    match record {
        Some(value) => {
            let parsed = super::adapter::from_db_value_common(value);
            Ok(serde_json::to_value(parsed).unwrap_or_else(|_| serde_json::json!({})))
        }
        None => Ok(serde_json::json!({ "config": "{}", "rootDir": Value::Null, "updatedAt": "" })),
    }
}

#[tauri::command]
pub async fn save_zcode_common_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: ZcodeCommonConfigInput,
) -> Result<(), String> {
    let clear = input.clear_root_dir.unwrap_or(false);
    let root_dir = if clear {
        None
    } else {
        input.root_dir.as_deref().map(str::trim).filter(|v| !v.is_empty())
    };
    let payload = super::adapter::to_db_value_common(&input.config, root_dir);
    state.with_conn(|conn| {
        crate::db::helpers::db_put(conn, DbTable::ZcodeCommonConfig, "common", &payload)
    })?;
    crate::coding::runtime_location::refresh_runtime_location_cache_for_module_async(&state, "zcode")
        .await
        .ok();
    let _ = app.emit("config-changed", "window");
    Ok(())
}

pub fn list_zcode_providers_for_db(db: &SqliteDbState) -> Result<Vec<super::types::ZcodeProvider>, String> {
    let order = provider_order()?;
    db.with_conn(|conn| db_list(conn, DbTable::ZcodeProvider, Some(&order))).map(|values| {
        values
            .into_iter()
            .map(super::adapter::from_db_value_provider)
            .collect()
    })
}

#[tauri::command]
pub async fn list_zcode_providers(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<Vec<super::types::ZcodeProvider>, String> {
    list_zcode_providers_for_db(state.inner())
}

fn next_sort_index(db: &SqliteDbState) -> Result<i64, String> {
    db.with_conn(|conn| {
        Ok(db_max_i64(conn, DbTable::ZcodeProvider, &JsonFieldPath::new("sort_index")?)?
            .map(|value| value + 1)
            .unwrap_or(0))
    })
}

/// Creates or updates a provider row. The registry projection happens in
/// [`save_zcode_provider`], which holds the write lock.
#[tauri::command]
pub async fn create_zcode_provider(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    provider: ZcodeProviderInput,
) -> Result<super::types::ZcodeProvider, String> {
    let _guard = CONFIG_WRITE_LOCK.lock().await;
    let db = state.inner();
    let id = provider.id.clone().unwrap_or_else(crate::coding::db_id::db_new_id);
    let now = Local::now().to_rfc3339();
    let sort_index = next_sort_index(db)?;
    let content = super::types::ZcodeProviderContent {
        name: provider.name,
        category: provider.category,
        settings_config: provider.settings_config,
        source_provider_id: provider.source_provider_id,
        website_url: provider.website_url,
        notes: provider.notes,
        icon: provider.icon,
        icon_color: provider.icon_color,
        sort_index,
        meta: provider.meta,
        is_disabled: provider.is_disabled.unwrap_or(false),
    };
    let mut payload = super::adapter::to_db_value_provider(&content);
    payload["created_at"] = json!(now);
    payload["updated_at"] = json!(now);
    payload["is_applied"] = json!(false);
    db.with_conn(|conn| db_put(conn, DbTable::ZcodeProvider, &id, &payload))?;

    let saved = db
        .with_conn(|conn| db_get(conn, DbTable::ZcodeProvider, &id))?
        .map(super::adapter::from_db_value_provider)
        .ok_or_else(|| "Failed to read back the created ZCode provider".to_string())?;
    let _ = app.emit("config-changed", "window");
    Ok(saved)
}

#[tauri::command]
pub async fn update_zcode_provider(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    provider: super::types::ZcodeProvider,
) -> Result<super::types::ZcodeProvider, String> {
    let _guard = CONFIG_WRITE_LOCK.lock().await;
    let db = state.inner();
    let existing = db
        .with_conn(|conn| db_get(conn, DbTable::ZcodeProvider, &provider.id))?
        .ok_or_else(|| format!("ZCode provider '{}' not found", provider.id))?;
    let previous = super::adapter::from_db_value_provider(existing);

    let content = super::types::ZcodeProviderContent {
        name: provider.name,
        category: provider.category,
        settings_config: provider.settings_config,
        source_provider_id: provider.source_provider_id,
        website_url: provider.website_url,
        notes: provider.notes,
        icon: provider.icon,
        icon_color: provider.icon_color,
        sort_index: provider.sort_index,
        meta: provider.meta,
        is_disabled: provider.is_disabled,
    };
    let mut payload = super::adapter::to_db_value_provider(&content);
    payload["created_at"] = json!(previous.created_at);
    payload["updated_at"] = json!(Local::now().to_rfc3339());
    payload["is_applied"] = json!(provider.is_applied);
    db.with_conn(|conn| db_put(conn, DbTable::ZcodeProvider, &provider.id, &payload))?;

    let saved = db
        .with_conn(|conn| db_get(conn, DbTable::ZcodeProvider, &provider.id))?
        .map(super::adapter::from_db_value_provider)
        .ok_or_else(|| "Failed to read back the updated ZCode provider".to_string())?;
    let _ = app.emit("config-changed", "window");
    Ok(saved)
}

#[tauri::command]
pub async fn delete_zcode_provider(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    let _guard = CONFIG_WRITE_LOCK.lock().await;
    let db = state.inner();
    let record = db
        .with_conn(|conn| db_get(conn, DbTable::ZcodeProvider, &id))?
        .ok_or_else(|| format!("ZCode provider '{id}' not found"))?;
    let provider = super::adapter::from_db_value_provider(record);
    let settings: Option<super::types::ZcodeSettingsConfig> =
        serde_json::from_str(&provider.settings_config).ok();

    // Remove the registry entry first: leaving a DB row without its projection
    // is recoverable, the reverse is not.
    if let Some(settings) = settings {
        if !projection::is_reserved_provider_id(&settings.provider_id) {
            let path = zcode_provider_config_path(db)?;
            let mut base = projection::read_provider_config_base(&path);
            projection::remove_managed_provider(&mut base, &settings.provider_id);
            projection::atomic_write_json(&path, &base)?;
        }
    }
    db.with_conn(|conn| db_delete(conn, DbTable::ZcodeProvider, &id).map(|_| ()))?;
    let _ = app.emit("config-changed", "window");
    let _ = app.emit("wsl-sync-request-zcode", ());
    Ok(())
}

#[tauri::command]
pub async fn reorder_zcode_providers(
    state: tauri::State<'_, SqliteDbState>,
    ids: Vec<String>,
) -> Result<(), String> {
    let db = state.inner();
    for (index, id) in ids.iter().enumerate() {
        db.with_conn(|conn| {
            db_patch_fields(
                conn,
                DbTable::ZcodeProvider,
                id,
                &[("sort_index", json!(index as i64))],
            )
            .map(|_| ())
        })?;
    }
    Ok(())
}

#[tauri::command]
pub async fn toggle_zcode_provider_disabled(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    provider_id: String,
    is_disabled: bool,
) -> Result<(), String> {
    let db = state.inner();
    db.with_conn(|conn| {
        db_patch_fields(
            conn,
            DbTable::ZcodeProvider,
            &provider_id,
            &[("is_disabled", json!(is_disabled))],
        )
        .map(|_| ())
    })?;
    let _ = app.emit("config-changed", "window");
    Ok(())
}

/// Creates or updates a provider and reprojects the registry.
#[tauri::command]
pub async fn save_zcode_provider(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    provider: ZcodeProviderInput,
) -> Result<String, String> {
    let path = zcode_provider_config_path(&state)?;
    if !projection::provider_config_exists(&path) {
        return Err(
            "ZCode has not migrated to the new provider registry yet. Start the ZCode desktop app once, then retry."
                .to_string(),
        );
    }

    let mut settings: super::types::ZcodeSettingsConfig = serde_json::from_str(&provider.settings_config)
        .map_err(|error| format!("Invalid ZCode provider settings: {error}"))?;
    if projection::is_reserved_provider_id(&settings.provider_id) {
        return Err(format!(
            "Provider id '{}' is reserved by ZCode and cannot be managed here.",
            settings.provider_id
        ));
    }
    if !projection::is_managed_provider_id(&settings.provider_id) {
        return Err(format!(
            "Provider id '{}' must start with '{}'. The registry file is shared with ZCode, so only ids in that namespace are rewritten.",
            settings.provider_id,
            super::constants::ZCODE_MANAGED_PROVIDER_ID_PREFIX
        ));
    }
    projection::apply_personal_model_ids(&mut settings);

    let mut base = projection::read_provider_config_base(&path);
    projection::remove_managed_provider(&mut base, &settings.provider_id);
    projection::upsert_provider_rule(&mut base, projection::build_provider_rule(&settings));
    projection::push_model_rules(&mut base, &settings.provider_id, &settings.models);
    projection::atomic_write_json(&path, &base)?;

    let _ = app.emit("config-changed", "window");
    let _ = app.emit("wsl-sync-request-zcode", ());
    Ok(settings.provider_id)
}

/// Removes a provider from the registry and from storage.
#[tauri::command]
pub async fn delete_zcode_provider_from_file(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    provider_id: String,
) -> Result<(), String> {
    if projection::is_reserved_provider_id(&provider_id) {
        return Err(format!("Provider id '{provider_id}' is reserved by ZCode."));
    }
    let path = zcode_provider_config_path(&state)?;
    let mut base = projection::read_provider_config_base(&path);
    projection::remove_managed_provider(&mut base, &provider_id);
    projection::atomic_write_json(&path, &base)?;
    let _ = app.emit("config-changed", "window");
    let _ = app.emit("wsl-sync-request-zcode", ());
    Ok(())
}

/// Points `defaultModelSelection` at the given provider.
#[tauri::command]
pub async fn select_zcode_provider(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    provider_id: String,
    model_id: String,
) -> Result<(), String> {
    if projection::is_reserved_provider_id(&provider_id) {
        return Err(format!("Provider id '{provider_id}' is reserved by ZCode."));
    }
    let path = zcode_provider_config_path(&state)?;
    let mut base = projection::read_provider_config_base(&path);
    if !projection::list_provider_ids(&base).contains(&provider_id) {
        return Err(format!("Provider '{provider_id}' is not present in the ZCode registry."));
    }
    projection::set_default_model_selection(&mut base, &provider_id, &model_id);
    projection::atomic_write_json(&path, &base)?;
    let _ = app.emit("config-changed", "window");
    let _ = app.emit("wsl-sync-request-zcode", ());
    Ok(())
}

/// Lists provider templates from the installed ZCode built-in catalog.
///
/// The catalog path embeds the app version, so it is discovered at call time
/// rather than hardcoded; a bundled fallback keeps the UI usable when ZCode is
/// not installed.
#[tauri::command]
pub async fn list_zcode_provider_templates(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<Vec<super::types::ZcodeProviderTemplate>, String> {
    let root = resolve_zcode_root_dir(&state)?;
    Ok(super::templates::read_provider_templates(&root))
}

// ---------------------------------------------------------------------------
// Global prompt (`~/.zcode/AGENTS.md`)
// ---------------------------------------------------------------------------

async fn get_zcode_prompt_record(
    db: &SqliteDbState,
    id: &str,
) -> Result<Option<ZcodePromptConfig>, String> {
    db.with_conn(|conn| db_get(conn, DbTable::ZcodePromptConfig, id))
        .map(|value| value.map(super::adapter::from_db_value_prompt))
}

#[tauri::command]
pub async fn list_zcode_prompt_configs(
    state: tauri::State<'_, SqliteDbState>,
) -> Result<Vec<ZcodePromptConfig>, String> {
    let order = prompt_order()?;
    state
        .with_conn(|conn| db_list(conn, DbTable::ZcodePromptConfig, Some(&order)))
        .map(|values| {
            values
                .into_iter()
                .map(super::adapter::from_db_value_prompt)
                .collect()
        })
}

#[tauri::command]
pub async fn create_zcode_prompt_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: ZcodePromptConfigInput,
) -> Result<ZcodePromptConfig, String> {
    let db = state.inner();
    let id = input.id.clone().unwrap_or_else(crate::coding::db_id::db_new_id);
    let now = Local::now().to_rfc3339();
    let sort_index = db.with_conn(|conn| {
        Ok(db_max_i64(conn, DbTable::ZcodePromptConfig, &JsonFieldPath::new("sort_index")?)?
            .map(|value| value + 1)
            .unwrap_or(0))
    })?;
    let payload = json!({
        "name": input.name,
        "content": input.content,
        "is_applied": false,
        "sort_index": sort_index,
        "created_at": now,
        "updated_at": now,
    });
    db.with_conn(|conn| db_put(conn, DbTable::ZcodePromptConfig, &id, &payload))?;
    let _ = app.emit("config-changed", "window");
    get_zcode_prompt_record(db, &id)
        .await?
        .ok_or_else(|| "Failed to read back the created ZCode prompt".to_string())
}

#[tauri::command]
pub async fn update_zcode_prompt_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: ZcodePromptConfigInput,
) -> Result<ZcodePromptConfig, String> {
    let db = state.inner();
    let id = input
        .id
        .clone()
        .ok_or_else(|| "ZCode prompt id is required for update".to_string())?;
    let existing = get_zcode_prompt_record(db, &id)
        .await?
        .ok_or_else(|| format!("ZCode prompt '{id}' not found"))?;
    db.with_conn(|conn| {
        db_patch_fields(
            conn,
            DbTable::ZcodePromptConfig,
            &id,
            &[
                ("name", json!(input.name)),
                ("content", json!(input.content)),
                ("updated_at", json!(Local::now().to_rfc3339())),
            ],
        )
        .map(|_| ())
    })?;
    // An applied prompt must keep the live file in step with its record.
    if existing.is_applied {
        write_text_atomic(&zcode_prompt_path(db)?, &input.content)?;
        let _ = app.emit("wsl-sync-request-zcode", ());
    }
    let _ = app.emit("config-changed", "window");
    get_zcode_prompt_record(db, &id)
        .await?
        .ok_or_else(|| format!("ZCode prompt '{id}' disappeared after update"))
}

/// Deletes the stored record only; the live `AGENTS.md` is left untouched so a
/// user's current rules are never silently dropped.
#[tauri::command]
pub async fn delete_zcode_prompt_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), String> {
    state.with_conn(|conn| db_delete(conn, DbTable::ZcodePromptConfig, &id).map(|_| ()))?;
    let _ = app.emit("config-changed", "window");
    Ok(())
}

#[tauri::command]
pub async fn apply_zcode_prompt_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    config_id: String,
) -> Result<(), String> {
    let db = state.inner();
    let prompt = get_zcode_prompt_record(db, &config_id)
        .await?
        .ok_or_else(|| format!("ZCode prompt '{config_id}' not found"))?;
    write_text_atomic(&zcode_prompt_path(db)?, &prompt.content)?;
    let now = Local::now().to_rfc3339();
    db.with_conn_mut(|conn| {
        db_update_applied_status(conn, DbTable::ZcodePromptConfig, Some(&config_id), &now)
    })?;
    let _ = app.emit("config-changed", "window");
    let _ = app.emit("wsl-sync-request-zcode", ());
    Ok(())
}

/// Clears every applied flag and empties the live file while keeping records.
#[tauri::command]
pub async fn disable_zcode_prompt_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    config_id: String,
) -> Result<(), String> {
    let db = state.inner();
    get_zcode_prompt_record(db, &config_id)
        .await?
        .ok_or_else(|| format!("ZCode prompt '{config_id}' not found"))?;
    let now = Local::now().to_rfc3339();
    db.with_conn_mut(|conn| {
        db_update_applied_status(conn, DbTable::ZcodePromptConfig, None, &now)
    })?;
    write_text_atomic(&zcode_prompt_path(db)?, "")?;
    let _ = app.emit("config-changed", "window");
    let _ = app.emit("wsl-sync-request-zcode", ());
    Ok(())
}

#[tauri::command]
pub async fn reorder_zcode_prompt_configs(
    state: tauri::State<'_, SqliteDbState>,
    ids: Vec<String>,
) -> Result<(), String> {
    for (index, id) in ids.iter().enumerate() {
        state.with_conn(|conn| {
            db_patch_fields(
                conn,
                DbTable::ZcodePromptConfig,
                id,
                &[("sort_index", json!(index as i64))],
            )
            .map(|_| ())
        })?;
    }
    Ok(())
}

/// Adopts the current live `AGENTS.md` as a stored prompt record.
#[tauri::command]
pub async fn save_zcode_local_prompt_config(
    state: tauri::State<'_, SqliteDbState>,
    app: tauri::AppHandle,
    input: ZcodePromptConfigInput,
) -> Result<ZcodePromptConfig, String> {
    create_zcode_prompt_config(state, app, input).await
}
