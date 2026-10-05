//! ZCode backend commands.
//!
//! ZCode has no single "active provider": every enabled provider shows up in
//! the model picker at once. "Apply" therefore means writing the provider rule,
//! enabling it, and pointing `defaultModelSelection` at it.

use std::path::PathBuf;

use serde_json::Value;
use tauri::Emitter;

use crate::db::schema::DbTable;
use crate::db::SqliteDbState;

use super::constants::*;
use super::projection;
use super::types::{ConfigPathInfo, ZcodeCommonConfigInput, ZcodeProviderInput};

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
