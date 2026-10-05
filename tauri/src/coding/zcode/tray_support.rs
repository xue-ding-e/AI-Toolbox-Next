//! Tray menu integration for ZCode.
//!
//! The tray only ever toggles which provider is written into
//! `defaultModelSelection`; it never rewrites the registry itself.

use tauri::{AppHandle, Manager, Runtime};

use super::commands;
use super::projection;
use crate::db::SqliteDbState;

/// Tray sections are always offered for ZCode; visibility is decided by the
/// caller through `visible_tabs`.
pub async fn is_enabled_for_tray<R: Runtime>(_app: &AppHandle<R>) -> bool {
    true
}

/// Reads the live registry and reports which provider is currently selected.
pub async fn get_zcode_tray_data<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(String, Vec<(String, bool)>), String> {
    let state = app.state::<SqliteDbState>();
    let path = commands::zcode_provider_config_path(&state)?;
    let base = projection::read_provider_config_base(&path);
    let selected = projection::read_default_provider_id(&base);
    let items = projection::list_provider_ids(&base)
        .into_iter()
        .map(|id| {
            let is_selected = selected.as_deref() == Some(id.as_str());
            (id, is_selected)
        })
        .collect();
    Ok(("ZCode".to_string(), items))
}

/// Applies a provider chosen from the tray.
///
/// The model id comes from the provider's stored settings so the tray does not
/// need to load the whole provider record.
pub async fn apply_zcode_selection<R: Runtime>(
    app: &AppHandle<R>,
    provider_id: &str,
    model_id: &str,
) -> Result<(), String> {
    let state = app.state::<SqliteDbState>();
    let path = commands::zcode_provider_config_path(&state)?;
    let mut base = projection::read_provider_config_base(&path);
    if !projection::list_provider_ids(&base).contains(&provider_id.to_string()) {
        return Err(format!("Provider '{provider_id}' is not present in the ZCode registry."));
    }
    projection::set_default_model_selection(&mut base, provider_id, model_id);
    projection::atomic_write_json(&path, &base)
}
