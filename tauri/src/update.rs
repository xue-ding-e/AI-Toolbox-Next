use serde::{Deserialize, Serialize};

/// Response from GitHub latest.json
#[derive(Debug, Serialize, Deserialize)]
struct LatestRelease {
    version: String,
    notes: Option<String>,
    pub_date: Option<String>,
}

/// Update check result
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub has_update: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
    pub release_notes: String,
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

    Ok(UpdateCheckResult {
        has_update,
        current_version,
        latest_version: latest_version.clone(),
        release_url: format!(
            "https://github.com/{}/releases/tag/v{}",
            GITHUB_REPO, latest_version
        ),
        release_notes: release.notes.unwrap_or_default(),
    })
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
