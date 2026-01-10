//! System Tray Module
//!
//! Provides system tray icon and menu with flat structure:
//! - Open Main Window
//! - ─── Oh My OpenCode ───
//! - Config options (with checkmarks for applied config)
//! - ─── Claude Code ───
//! - Provider options (with checkmarks for applied provider)
//! - Quit

use crate::db::DbState;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};

/// Create system tray icon and menu
pub fn create_tray<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    let quit_item = PredefinedMenuItem::quit(app, Some("退出"))?;
    let show_item = MenuItem::with_id(app, "show", "打开主界面", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .on_menu_event(move |app, event| {
            let event_id = event.id().as_ref().to_string();

            if event_id == "show" {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();

                    // macOS: Show dock icon when window is shown
                    #[cfg(target_os = "macos")]
                    {
                        let _ = app.show();
                    }
                }
            } else if event_id.starts_with("omo_config_") {
                let config_id = event_id.strip_prefix("omo_config_").unwrap().to_string();
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = apply_omo_config(&app_handle, &config_id).await {
                        eprintln!("Failed to apply Oh My OpenCode config: {}", e);
                    }
                });
            } else if event_id.starts_with("claude_provider_") {
                let provider_id = event_id
                    .strip_prefix("claude_provider_")
                    .unwrap()
                    .to_string();
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = apply_claude_provider(&app_handle, &provider_id).await {
                        eprintln!("Failed to apply Claude provider: {}", e);
                    }
                });
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();

                    // macOS: Show dock icon when window is shown
                    #[cfg(target_os = "macos")]
                    {
                        let _ = app.show();
                    }
                }
            }
        })
        .build(app)?;

    // Store tray in app state for later updates
    app.manage(_tray);

    // Initial menu refresh
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = refresh_tray_menus(&app_clone).await;
    });

    Ok(())
}

/// Refresh tray menus with flat structure
pub async fn refresh_tray_menus<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    // Get database state
    let db_state = app.state::<DbState>();
    let db = db_state.0.lock().await;

    // Query Oh My OpenCode configs
    let omo_records_result: Result<Vec<serde_json::Value>, _> = db
        .query("SELECT * OMIT id FROM oh_my_opencode_config")
        .await
        .map_err(|e| format!("Failed to query configs: {}", e))?
        .take(0);

    let mut omo_configs = Vec::new();
    if let Ok(records) = omo_records_result {
        for record in records {
            if let (Some(name), Some(config_id), Some(is_applied)) = (
                record.get("name").and_then(|v| v.as_str()),
                record
                    .get("config_id")
                    .or_else(|| record.get("configId"))
                    .and_then(|v| v.as_str()),
                record
                    .get("is_applied")
                    .or_else(|| record.get("isApplied"))
                    .and_then(|v| v.as_bool()),
            ) {
                omo_configs.push((config_id.to_string(), name.to_string(), is_applied));
            }
        }
    }
    omo_configs.sort_by(|a, b| a.1.cmp(&b.1));

    // Query Claude Code providers
    let claude_records_result: Result<Vec<serde_json::Value>, _> = db
        .query("SELECT * OMIT id FROM claude_provider")
        .await
        .map_err(|e| format!("Failed to query providers: {}", e))?
        .take(0);

    let mut claude_providers = Vec::new();
    if let Ok(records) = claude_records_result {
        for record in records {
            if let (Some(name), Some(provider_id), Some(is_applied), sort_index) = (
                record.get("name").and_then(|v| v.as_str()),
                record
                    .get("provider_id")
                    .or_else(|| record.get("providerId"))
                    .and_then(|v| v.as_str()),
                record
                    .get("is_applied")
                    .or_else(|| record.get("isApplied"))
                    .and_then(|v| v.as_bool()),
                record
                    .get("sort_index")
                    .or_else(|| record.get("sortIndex"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
            ) {
                claude_providers.push((
                    provider_id.to_string(),
                    name.to_string(),
                    is_applied,
                    sort_index,
                ));
            }
        }
    }
    claude_providers.sort_by_key(|p| p.3);

    drop(db);

    // Build flat menu
    let quit_item = PredefinedMenuItem::quit(app, Some("退出")).map_err(|e| e.to_string())?;
    let show_item = MenuItem::with_id(app, "show", "打开主界面", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let separator1 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;

    // Oh My OpenCode section header
    let omo_header = MenuItem::with_id(
        app,
        "omo_header",
        "──── Oh My OpenCode ────",
        false,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;

    // Build Oh My OpenCode items
    let mut omo_items: Vec<Box<dyn tauri::menu::IsMenuItem<R>>> = Vec::new();
    if omo_configs.is_empty() {
        let empty_item: Box<dyn tauri::menu::IsMenuItem<R>> = Box::new(
            MenuItem::with_id(app, "omo_empty", "  暂无配置", false, None::<&str>)
                .map_err(|e| e.to_string())?,
        );
        omo_items.push(empty_item);
    } else {
        for (config_id, name, is_applied) in omo_configs {
            let item_id = format!("omo_config_{}", config_id);
            let item: Box<dyn tauri::menu::IsMenuItem<R>> = Box::new(
                CheckMenuItem::with_id(app, &item_id, &name, true, is_applied, None::<&str>)
                    .map_err(|e| e.to_string())?,
            );
            omo_items.push(item);
        }
    }

    let separator2 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;

    // Claude Code section header
    let claude_header = MenuItem::with_id(
        app,
        "claude_header",
        "──── Claude Code ────",
        false,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;

    // Build Claude Code items
    let mut claude_items: Vec<Box<dyn tauri::menu::IsMenuItem<R>>> = Vec::new();
    if claude_providers.is_empty() {
        let empty_item: Box<dyn tauri::menu::IsMenuItem<R>> = Box::new(
            MenuItem::with_id(app, "claude_empty", "  暂无配置", false, None::<&str>)
                .map_err(|e| e.to_string())?,
        );
        claude_items.push(empty_item);
    } else {
        for (provider_id, name, is_applied, _) in claude_providers {
            let item_id = format!("claude_provider_{}", provider_id);
            let item: Box<dyn tauri::menu::IsMenuItem<R>> = Box::new(
                CheckMenuItem::with_id(app, &item_id, &name, true, is_applied, None::<&str>)
                    .map_err(|e| e.to_string())?,
            );
            claude_items.push(item);
        }
    }

    // Combine all items into a flat menu
    let mut all_items: Vec<&dyn tauri::menu::IsMenuItem<R>> = Vec::new();
    all_items.push(&show_item);
    all_items.push(&separator1);
    all_items.push(&omo_header);
    for item in &omo_items {
        all_items.push(item.as_ref());
    }
    all_items.push(&separator2);
    all_items.push(&claude_header);
    for item in &claude_items {
        all_items.push(item.as_ref());
    }
    all_items.push(&quit_item);

    let menu = Menu::with_items(app, &all_items).map_err(|e| e.to_string())?;

    // Update tray menu
    let tray = app.state::<tauri::tray::TrayIcon>();
    tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;

    Ok(())
}

/// Apply Oh My OpenCode config
async fn apply_omo_config<R: Runtime>(app: &AppHandle<R>, config_id: &str) -> Result<(), String> {
    let db_state = app.state::<DbState>();
    let db = db_state.0.lock().await;

    // Use the single source of truth from oh_my_opencode module
    crate::coding::oh_my_opencode::commands::apply_config_internal(&db, config_id).await?;

    drop(db);

    // Refresh tray menus
    refresh_tray_menus(app).await?;

    // Notify main window to refresh
    app.emit("config-changed", "oh-my-opencode")
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Apply Claude Code provider
async fn apply_claude_provider<R: Runtime>(
    app: &AppHandle<R>,
    provider_id: &str,
) -> Result<(), String> {
    let db_state = app.state::<DbState>();
    let db = db_state.0.lock().await;

    // Use the single source of truth from claude_code module
    crate::coding::claude_code::commands::apply_config_internal(&db, provider_id).await?;

    drop(db);

    // Refresh tray menus
    refresh_tray_menus(app).await?;

    // Notify main window to refresh
    app.emit("config-changed", "claude-code")
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Apply minimize-to-tray policy (macOS only - hide dock icon)
#[cfg(target_os = "macos")]
pub fn apply_tray_policy<R: Runtime>(app: &AppHandle<R>, minimize_to_tray: bool) {
    if minimize_to_tray {
        let _ = app.hide();
    } else {
        let _ = app.show();
    }
}

#[cfg(not(target_os = "macos"))]
pub fn apply_tray_policy<R: Runtime>(_app: &AppHandle<R>, _minimize_to_tray: bool) {
    // No-op on Windows/Linux
}
