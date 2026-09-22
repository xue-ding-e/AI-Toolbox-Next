//! System clipboard access for the WebView (issue #369).
//!
//! Monaco's context-menu copy/cut/paste cannot rely on the browser's native
//! clipboard events: the context menu has no native paste event, so Monaco
//! falls back to the async Clipboard API (`navigator.clipboard.readText`),
//! which fails silently inside WebKitGTK (WSLg clipboard bridge) and is
//! unreliable in WebView2. These commands talk to the OS clipboard directly
//! via arboard; the frontend clipboard service prefers them and falls back to
//! the Web API.

use arboard::Clipboard;
use log::warn;

/// Clipboard failures are silent in the WebView (the frontend swallows them and
/// falls back to the Web API), so they are logged here: when a user reports that
/// paste or copy does nothing, the app log has to say which side failed
/// (issue #384).
fn log_failure(message: String) -> String {
    warn!("{message}");
    message
}

fn copy_text_sync(text: &str) -> Result<(), String> {
    let mut clipboard = Clipboard::new()
        .map_err(|error| log_failure(format!("Failed to access system clipboard: {error}")))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|error| log_failure(format!("Failed to copy text to clipboard: {error}")))
}

fn read_text_sync() -> Result<String, String> {
    let mut clipboard = Clipboard::new()
        .map_err(|error| log_failure(format!("Failed to access system clipboard: {error}")))?;
    clipboard
        .get_text()
        .map_err(|error| log_failure(format!("Failed to read clipboard text: {error}")))
}

#[tauri::command]
pub async fn copy_text_to_clipboard(text: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || copy_text_sync(&text))
        .await
        .map_err(|error| format!("Clipboard task failed: {error}"))?
}

#[tauri::command]
pub async fn read_clipboard_text() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(read_text_sync)
        .await
        .map_err(|error| format!("Clipboard task failed: {error}"))?
}
