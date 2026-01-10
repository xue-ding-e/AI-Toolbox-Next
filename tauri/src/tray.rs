//! System Tray Module
//!
//! Provides system tray icon and menu with flat structure:
//! - Open Main Window
//! - ─── OpenCode 模型 ────
//! - 主模型 / 小模型 (with submenus for model selection)
//! - ─── Oh My OpenCode ───
//! - Config options (with checkmarks for applied config)
//! - ─── Claude Code ───
//! - Provider options (with checkmarks for applied provider)
//! - Quit

use crate::coding::open_code::tray_support as opencode_tray;
use crate::coding::oh_my_opencode::tray_support as omo_tray;
use crate::coding::claude_code::tray_support as claude_tray;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};

/// 命令：刷新托盘菜单
#[tauri::command]
pub async fn refresh_tray_menu<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    refresh_tray_menus(&app).await
}

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
                    if let Err(e) = omo_tray::apply_oh_my_opencode_config(&app_handle, &config_id).await {
                        eprintln!("Failed to apply Oh My OpenCode config: {}", e);
                    }
                    // Refresh tray menu to update checkmarks
                    let _ = refresh_tray_menus(&app_handle).await;
                });
            } else if event_id.starts_with("claude_provider_") {
                let provider_id = event_id
                    .strip_prefix("claude_provider_")
                    .unwrap()
                    .to_string();
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = claude_tray::apply_claude_code_provider(&app_handle, &provider_id).await {
                        eprintln!("Failed to apply Claude provider: {}", e);
                    }
                    // Refresh tray menu to update checkmarks
                    let _ = refresh_tray_menus(&app_handle).await;
                });
            } else if event_id.starts_with("opencode_model_") {
                // Parse: opencode_model_main|small_provider/model_id
                let remaining = event_id.strip_prefix("opencode_model_").unwrap();
                let (model_type, item_id) = remaining.split_once('_').unwrap();
                let model_type = model_type.to_string();
                let item_id = item_id.to_string();
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = opencode_tray::apply_opencode_model(&app_handle, &model_type, &item_id).await {
                        eprintln!("Failed to apply OpenCode model: {}", e);
                    }
                    // Refresh tray menu to update checkmarks
                    let _ = refresh_tray_menus(&app_handle).await;
                });
            }
        })
        .on_tray_icon_event(move |tray, event| {
            let app = tray.app_handle().clone();
            let app_for_refresh = app.clone();

            // 立即刷新菜单以确保显示最新的选中状态
            // 同步刷新确保在菜单显示前完成
            tauri::async_runtime::block_on(async {
                let _ = refresh_tray_menus(&app_for_refresh);
            });

            // 只处理左键点击打开主窗口
            if let TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                ..
            } = event
            {
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
    // Get data from modules
    let (main_model_data, small_model_data) = opencode_tray::get_opencode_tray_model_data(app).await?;
    let omo_data = omo_tray::get_oh_my_opencode_tray_data(app).await?;
    let claude_data = claude_tray::get_claude_code_tray_data(app).await?;

    // Build flat menu
    let quit_item = PredefinedMenuItem::quit(app, Some("退出")).map_err(|e| e.to_string())?;
    let show_item = MenuItem::with_id(app, "show", "打开主界面", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let separator1 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;

    // OpenCode Model section header
    let opencode_model_header = MenuItem::with_id(
        app,
        "opencode_model_header",
        "──── OpenCode 模型 ────",
        false,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;

    // Build OpenCode model submenus
    let main_model_submenu = build_model_submenu(app, &main_model_data, "main").await?;
    let small_model_submenu = build_model_submenu(app, &small_model_data, "small").await?;

    let separator2 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;

    // Oh My OpenCode section header
    let omo_header = MenuItem::with_id(
        app,
        "omo_header",
        &omo_data.title,
        false,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;

    // Build Oh My OpenCode items
    let mut omo_items: Vec<Box<dyn tauri::menu::IsMenuItem<R>>> = Vec::new();
    if omo_data.items.is_empty() {
        let empty_item: Box<dyn tauri::menu::IsMenuItem<R>> = Box::new(
            MenuItem::with_id(app, "omo_empty", "  暂无配置", false, None::<&str>)
                .map_err(|e| e.to_string())?,
        );
        omo_items.push(empty_item);
    } else {
        for item in omo_data.items {
            let item_id = format!("omo_config_{}", item.id);
            let menu_item: Box<dyn tauri::menu::IsMenuItem<R>> = Box::new(
                CheckMenuItem::with_id(app, &item_id, &item.display_name, true, item.is_selected, None::<&str>)
                    .map_err(|e| e.to_string())?,
            );
            omo_items.push(menu_item);
        }
    }

    let _separator2 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;

    // Claude Code section header
    let _claude_header = MenuItem::with_id(
        app,
        "claude_header",
        &claude_data.title,
        false,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;

    // Build Claude Code items
    let mut claude_items: Vec<Box<dyn tauri::menu::IsMenuItem<R>>> = Vec::new();
    if claude_data.items.is_empty() {
        let empty_item: Box<dyn tauri::menu::IsMenuItem<R>> = Box::new(
            MenuItem::with_id(app, "claude_empty", "  暂无配置", false, None::<&str>)
                .map_err(|e| e.to_string())?,
        );
        claude_items.push(empty_item);
    } else {
        for item in claude_data.items {
            let item_id = format!("claude_provider_{}", item.id);
            let menu_item: Box<dyn tauri::menu::IsMenuItem<R>> = Box::new(
                CheckMenuItem::with_id(app, &item_id, &item.display_name, true, item.is_selected, None::<&str>)
                    .map_err(|e| e.to_string())?,
            );
            claude_items.push(menu_item);
        }
    }

    // Combine all items into a flat menu
    let mut all_items: Vec<&dyn tauri::menu::IsMenuItem<R>> = Vec::new();
    all_items.push(&show_item);
    all_items.push(&separator1);
    all_items.push(&opencode_model_header);
    all_items.push(&main_model_submenu);
    all_items.push(&small_model_submenu);
    all_items.push(&separator2);
    all_items.push(&omo_header);
    for item in &omo_items {
        all_items.push(item.as_ref());
    }
    all_items.push(&_separator2);
    all_items.push(&_claude_header);
    for item in &claude_items {
        all_items.push(item.as_ref());
    }
    all_items.push(&separator1);
    all_items.push(&quit_item);

    let menu = Menu::with_items(app, &all_items).map_err(|e| e.to_string())?;

    // Update tray menu
    let tray = app.state::<tauri::tray::TrayIcon>();
    tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;

    Ok(())
}

/// Build a model selection submenu from tray data
async fn build_model_submenu<R: Runtime>(
    app: &AppHandle<R>,
    data: &opencode_tray::TrayModelData,
    model_type: &str, // "main" or "small"
) -> Result<Submenu<R>, String> {
    // Build title with current selection in parentheses
    let title = if data.current_display.is_empty() {
        data.title.clone()
    } else {
        format!("{} ({})", data.title, data.current_display)
    };
    let submenu_id = format!("{}_submenu", data.title);
    let submenu = Submenu::with_id(app, &submenu_id, &title, true)
        .map_err(|e| e.to_string())?;

    if data.items.is_empty() {
        let empty_item = MenuItem::with_id(app, &format!("{}_empty", data.title), "  暂无模型", false, None::<&str>)
            .map_err(|e| e.to_string())?;
        submenu.append(&empty_item).map_err(|e| e.to_string())?;
    } else {
        for item in &data.items {
            let item_id = format!("opencode_model_{}_{}", model_type, item.id);
            let menu_item = CheckMenuItem::with_id(app, &item_id, &item.display_name, true, item.is_selected, None::<&str>)
                .map_err(|e| e.to_string())?;
            submenu.append(&menu_item).map_err(|e| e.to_string())?;
        }
    }

    Ok(submenu)
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
