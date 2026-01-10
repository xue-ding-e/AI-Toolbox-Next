//! System Tray Module
//!
//! Provides system tray icon and menu with:
//! - Open Main Window
//! - Quit

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    Manager, AppHandle, Runtime,
};

/// Create system tray icon and menu
pub fn create_tray<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    let quit_item = PredefinedMenuItem::quit(app, Some("退出"))?;
    let show_item = MenuItem::with_id(app, "show", "打开主界面", true, None::<&str>)?;
    
    let menu = Menu::with_items(
        app,
        &[
            &show_item,
            &quit_item,
        ],
    )?;

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
