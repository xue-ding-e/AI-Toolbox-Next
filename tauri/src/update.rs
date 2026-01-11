use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri_plugin_updater::UpdaterExt;

/// Response from GitHub latest.json
#[derive(Debug, Serialize, Deserialize)]
struct LatestRelease {
    version: String,
    notes: Option<String>,
    pub_date: Option<String>,
    platforms: HashMap<String, PlatformInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PlatformInfo {
    signature: Option<String>,
    url: Option<String>,
}

/// Update check result
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub has_update: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
    pub release_notes: String,
    pub signature: Option<String>,
    pub url: Option<String>,
}

/// Check for updates from GitHub releases
#[tauri::command]
pub async fn check_for_updates(app_handle: tauri::AppHandle) -> Result<UpdateCheckResult, String> {
    const GITHUB_REPO: &str = "coulsontl/ai-toolbox";
    let latest_json_url = format!(
        "https://github.com/{}/releases/latest/download/latest.json",
        GITHUB_REPO
    );

    // Get current version from package info
    let current_version = app_handle.package_info().version.to_string();

    // Detect current platform
    let current_platform = detect_current_platform();

    // Fetch latest.json using reqwest (handles redirects properly)
    let client = reqwest::Client::new();
    let response = client
        .get(&latest_json_url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch latest.json: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to fetch latest.json: HTTP {}",
            response.status()
        ));
    }

    let release: LatestRelease = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse latest.json: {}", e))?;

    let latest_version = release.version.trim_start_matches('v').to_string();

    let has_update = compare_versions(&latest_version, &current_version) > 0;

    // Get signature and url for current platform
    let platform_info = release.platforms.get(&current_platform);
    let signature = platform_info.and_then(|p| p.signature.clone()).filter(|s| !s.is_empty());
    let url = platform_info.and_then(|p| p.url.clone()).filter(|s| !s.is_empty());

    Ok(UpdateCheckResult {
        has_update,
        current_version,
        latest_version: latest_version.clone(),
        release_url: format!(
            "https://github.com/{}/releases/tag/v{}",
            GITHUB_REPO, latest_version
        ),
        release_notes: release.notes.unwrap_or_default(),
        signature,
        url,
    })
}

/// Detect current platform string for matching latest.json
#[allow(unreachable_code)]
fn detect_current_platform() -> String {
    #[cfg(target_os = "windows")]
    {
        return "windows-x86_64".to_string();
    }

    #[cfg(target_os = "linux")]
    {
        return "linux-x86_64".to_string();
    }

    #[cfg(target_os = "macos")]
    {
        #[cfg(target_arch = "aarch64")]
        {
            return "darwin-aarch64".to_string();
        }
        #[cfg(target_arch = "x86_64")]
        {
            return "darwin-x86_64".to_string();
        }
    }

    "unknown".to_string()
}

/// Install the update
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<bool, String> {
    // Check for updates using the updater plugin
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => {
            // Download and install
            let mut downloaded = 0;
            let mut last_percentage = 0;

            let result = update.download_and_install(
                |chunk_length, content_length| {
                    downloaded += chunk_length;
                    if let Some(total) = content_length {
                        let percentage = (downloaded as f64 / total as f64 * 100.0) as u8;
                        if percentage != last_percentage {
                            last_percentage = percentage;
                            println!("Downloaded {}%", percentage);
                        }
                    }
                },
                || {},
            ).await;

            match result {
                Ok(_) => {
                    println!("Update installed successfully");
                    Ok(true)
                }
                Err(e) => Err(format!("Failed to install update: {}", e)),
            }
        }
        Ok(None) => Err("No update available".to_string()),
        Err(e) => Err(format!("Failed to check for updates: {}", e)),
    }
}

/// Compare two version strings (e.g., "1.2.3" vs "1.2.4")
/// Returns: 1 if v1 > v2, -1 if v1 < v2, 0 if equal
fn compare_versions(v1: &str, v2: &str) -> i32 {
    let parts1: Vec<i32> = v1.split('.').filter_map(|s| s.parse().ok()).collect();
    let parts2: Vec<i32> = v2.split('.').filter_map(|s| s.parse().ok()).collect();

    let max_len = parts1.len().max(parts2.len());

    for i in 0..max_len {
        let num1 = parts1.get(i).copied().unwrap_or(0);
        let num2 = parts2.get(i).copied().unwrap_or(0);

        if num1 > num2 {
            return 1;
        }
        if num1 < num2 {
            return -1;
        }
    }

    0
}
