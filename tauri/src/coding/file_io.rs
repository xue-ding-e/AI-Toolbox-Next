//! Timed file I/O helpers for coding runtime paths.
//!
//! WSL UNC / network roots can make `Path::exists` and `fs::read_to_string` block for a long time.
//! Extract / common-config paths must not hang the async runtime or leave the UI spinning forever.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// Default timeout for reading a single runtime config file on extract paths.
pub const DEFAULT_CONFIG_FILE_IO_TIMEOUT: Duration = Duration::from_secs(10);

/// Read a text file with `spawn_blocking` and a wall-clock timeout.
/// Returns `Ok(None)` when the path does not exist.
pub async fn read_optional_text_file_with_timeout(
    path: PathBuf,
    label: &str,
) -> Result<Option<String>, String> {
    let display_path = path.to_string_lossy().to_string();
    let label_owned = label.to_string();
    let read_task = tauri::async_runtime::spawn_blocking(move || {
        if !path.exists() {
            return Ok(None);
        }
        fs::read_to_string(&path)
            .map(Some)
            .map_err(|error| format!("Failed to read {} ({}): {error}", label_owned, path.display()))
    });

    match tokio::time::timeout(DEFAULT_CONFIG_FILE_IO_TIMEOUT, read_task).await {
        Ok(Ok(result)) => result,
        Ok(Err(join_error)) => Err(format!(
            "Failed to read {label} ({display_path}): {join_error}"
        )),
        Err(_) => Err(format!(
            "Timed out after {}s while reading {label} ({display_path}). If this is a WSL or network path, check that the distro/share is running and accessible.",
            DEFAULT_CONFIG_FILE_IO_TIMEOUT.as_secs(),
        )),
    }
}

/// Read a text file with timeout; missing file becomes an empty string.
pub async fn read_text_file_with_timeout(path: PathBuf, label: &str) -> Result<String, String> {
    Ok(read_optional_text_file_with_timeout(path, label)
        .await?
        .unwrap_or_default())
}
