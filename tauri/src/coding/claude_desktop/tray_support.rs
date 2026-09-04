//! Claude Desktop Tray Support Module.
//!
//! Provides the standardized API for tray menu integration. Unlike Claude Code,
//! this module does not integrate with the gateway yet — `apply_claude_desktop_provider`
//! applies the direct path via the commands internal apply.

use tauri::{AppHandle, Manager, Runtime};

use super::constants::COMMON_CONFIG_ID;

/// Item for provider selection in the tray menu.
#[derive(Debug, Clone)]
pub struct TrayProviderItem {
    pub id: String,
    pub display_name: String,
    pub is_selected: bool,
    pub is_disabled: bool,
    pub sort_index: i64,
}

/// Data for a provider submenu.
#[derive(Debug, Clone)]
pub struct TrayProviderData {
    pub title: String,
    pub current_display: String,
    pub items: Vec<TrayProviderItem>,
}

fn find_provider_display_name(items: &[TrayProviderItem]) -> String {
    items
        .iter()
        .find(|item| item.is_selected)
        .map(|item| item.display_name.clone())
        .unwrap_or_default()
}

/// Get tray provider data for Claude Desktop.
pub async fn get_claude_desktop_tray_data<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<TrayProviderData, String> {
    let providers = super::commands::list_claude_desktop_providers(app.state()).await?;

    let mut items: Vec<TrayProviderItem> = providers
        .into_iter()
        .filter(|provider| provider.id != COMMON_CONFIG_ID)
        .map(|provider| TrayProviderItem {
            id: provider.id,
            display_name: provider.name,
            is_selected: provider.is_applied,
            is_disabled: provider.is_disabled,
            sort_index: provider.sort_index.unwrap_or(0) as i64,
        })
        .collect();

    items.sort_by_key(|item| item.sort_index);
    let current_display = find_provider_display_name(&items);
    Ok(TrayProviderData {
        title: "──── Claude Desktop ────".to_string(),
        current_display,
        items,
    })
}

/// Apply provider selection from the tray menu (direct path).
pub async fn apply_claude_desktop_provider<R: Runtime>(
    app: &AppHandle<R>,
    provider_id: &str,
) -> Result<(), String> {
    let state = app.state::<crate::db::SqliteDbState>();
    super::commands::apply_config_internal_with_sync(state.inner(), app, provider_id, true, true)
        .await
}

/// Check if Claude Desktop should be shown in the tray menu.
/// Returns true — always visible as a core feature.
pub async fn is_enabled_for_tray<R: Runtime>(_app: &AppHandle<R>) -> bool {
    true
}
