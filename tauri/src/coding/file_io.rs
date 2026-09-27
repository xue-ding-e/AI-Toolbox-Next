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
        fs::read_to_string(&path).map(Some).map_err(|error| {
            format!(
                "Failed to read {} ({}): {error}",
                label_owned,
                path.display()
            )
        })
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

/// Run a blocking filesystem operation on the blocking pool with a wall-clock
/// timeout, so a stalled WSL/UNC root fails instead of leaving the UI spinning.
///
/// The timeout only bounds the await: the blocking thread may stay busy briefly
/// afterwards, so callers must not retry in a loop against unreachable paths.
/// Use a longer `timeout` for operations that legitimately walk a directory tree.
pub async fn run_blocking_fs_operation<T, F>(
    timeout: Duration,
    operation_label: &str,
    display_path: &str,
    operation: F,
) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    let label_owned = operation_label.to_string();
    let display_path_owned = display_path.to_string();

    match tokio::time::timeout(
        timeout,
        tauri::async_runtime::spawn_blocking(operation),
    )
    .await
    {
        Ok(Ok(result)) => result,
        Ok(Err(join_error)) => Err(format!(
            "Failed to {label_owned} ({display_path_owned}): {join_error}"
        )),
        Err(_) => Err(format!(
            "Timed out after {}s while trying to {label_owned} ({display_path_owned}). If this is a WSL or network path, check that the distro/share is running and accessible.",
            timeout.as_secs(),
        )),
    }
}

/// Run a best-effort filesystem probe on the blocking pool with a wall-clock
/// timeout. Returns `None` on timeout or join failure. The timeout only bounds
/// the await; the blocking thread may stay busy briefly afterwards, so callers
/// must not retry in a loop against unreachable paths.
pub async fn blocking_probe_with_timeout<F, T>(probe: F) -> Option<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    match tokio::time::timeout(
        DEFAULT_CONFIG_FILE_IO_TIMEOUT,
        tauri::async_runtime::spawn_blocking(probe),
    )
    .await
    {
        Ok(Ok(value)) => Some(value),
        _ => None,
    }
}
