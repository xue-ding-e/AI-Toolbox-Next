use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tauri::Manager;

const CENTRAL_DIR_NAME: &str = "skills";

/// Resolve the central repo path from settings or default to app_data_dir/skills
pub async fn resolve_central_repo_path(app: &tauri::AppHandle, state: &crate::DbState) -> Result<PathBuf> {
    // Try to get from settings first
    let settings_result: std::result::Result<Option<PathBuf>, String> = async {
        let db = state.0.lock().await;
        let mut result = db
            .query("SELECT * FROM skill_settings:`skills` LIMIT 1")
            .await
            .map_err(|e| e.to_string())?;

        let records: Vec<serde_json::Value> = result.take(0).map_err(|e| e.to_string())?;

        if let Some(record) = records.first() {
            if let Some(path) = record.get("central_repo_path").and_then(|v| v.as_str()) {
                if !path.is_empty() {
                    return Ok(Some(PathBuf::from(path)));
                }
            }
        }
        Ok(None)
    }.await;

    if let Ok(Some(path)) = settings_result {
        return Ok(path);
    }

    // Default to app data directory / skills
    let app_data_dir = app.path().app_data_dir()
        .context("failed to resolve app data directory")?;
    Ok(app_data_dir.join(CENTRAL_DIR_NAME))
}

/// Ensure the central repo directory exists
pub fn ensure_central_repo(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).with_context(|| format!("create {:?}", path))?;
    Ok(())
}

/// Expand ~ and ~/ in paths
pub fn expand_home_path(input: &str) -> Result<PathBuf> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        anyhow::bail!("storage path is empty");
    }
    if trimmed == "~" {
        let home = dirs::home_dir().context("failed to resolve home directory")?;
        return Ok(home);
    }
    if let Some(stripped) = trimmed.strip_prefix("~/") {
        let home = dirs::home_dir().context("failed to resolve home directory")?;
        return Ok(home.join(stripped));
    }
    Ok(PathBuf::from(trimmed))
}
