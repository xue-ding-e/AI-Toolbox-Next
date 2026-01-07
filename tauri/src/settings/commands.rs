use crate::db::DbState;
use super::types::{AppSettings, WebDAVConfig, S3Config};

/// Get settings from database
#[tauri::command]
pub async fn get_settings(state: tauri::State<'_, DbState>) -> Result<AppSettings, String> {
    let db = state.0.lock().await;

    let result: Option<AppSettings> = db
        .select(("settings", "app"))
        .await
        .map_err(|e| format!("Failed to get settings: {}", e))?;

    Ok(result.unwrap_or_else(|| AppSettings {
        language: "zh-CN".to_string(),
        current_module: "coding".to_string(),
        current_sub_tab: "opencode".to_string(),
        backup_type: "local".to_string(),
        local_backup_path: String::new(),
        webdav: WebDAVConfig::default(),
        s3: S3Config::default(),
        last_backup_time: None,
    }))
}

/// Save settings to database
#[tauri::command]
pub async fn save_settings(
    state: tauri::State<'_, DbState>,
    settings: AppSettings,
) -> Result<(), String> {
    let db = state.0.lock().await;

    let _: Option<AppSettings> = db
        .upsert(("settings", "app"))
        .content(settings)
        .await
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    Ok(())
}
