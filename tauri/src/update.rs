use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::db::SqliteDbState;
use crate::http_client;

/// Next releases are manual downloads, never unsigned updater payloads.
const NEXT_RELEASES_URL: &str = "https://github.com/xue-ding-e/AI-Toolbox-Next/releases";
const NEXT_RELEASES_API: &str = "https://api.github.com/repos/xue-ding-e/AI-Toolbox-Next/releases";

#[derive(Debug, Deserialize)]
struct ManualRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    draft: bool,
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
    /// Retained response fields for the shared UI. Next has no package-manager
    /// distribution and always returns false rather than suggesting upstream packages.
    pub scoop_install: bool,
    pub deb_install: bool,
}

/// Query the Next release list (including prereleases). No installer payload
/// is exposed: distribution is manual until an independently authorized Next
/// signing channel exists. A missing release is an error, not "up to date".
#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle) -> Result<UpdateCheckResult, String> {
    let current_version = app.package_info().version.to_string();
    let client = match app.try_state::<SqliteDbState>() {
        Some(state) => http_client::client(&state).await?,
        None => http_client::create_client_with_env_proxy(30)?,
    };
    let releases: Vec<ManualRelease> = client
        .get(NEXT_RELEASES_API)
        .header("User-Agent", "AI-Toolbox-Next")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch Next releases: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Failed to fetch Next releases: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse Next releases: {e}"))?;
    let release = releases
        .into_iter()
        .find(|release| !release.draft)
        .ok_or_else(|| format!("No Next release is published yet: {NEXT_RELEASES_URL}"))?;
    manual_release_result(
        release,
        current_version,
        is_scoop_install(),
        is_deb_install_async().await,
    )
}

fn manual_release_result(
    release: ManualRelease,
    current_version: String,
    _scoop_install: bool,
    _deb_install: bool,
) -> Result<UpdateCheckResult, String> {
    // Never let remote metadata redirect the manual-update button upstream.
    if !release
        .html_url
        .starts_with(&format!("{NEXT_RELEASES_URL}/tag/"))
    {
        return Err("Release URL is outside the Next repository".to_string());
    }
    let latest_version = release.tag_name.trim_start_matches('v').to_string();
    let latest = semver::Version::parse(&latest_version)
        .map_err(|e| format!("Invalid Next release version: {e}"))?;
    let current = semver::Version::parse(&current_version)
        .map_err(|e| format!("Invalid current version: {e}"))?;
    Ok(UpdateCheckResult {
        has_update: latest > current,
        current_version,
        latest_version,
        release_url: release.html_url,
        release_notes: release.body.unwrap_or_default(),
        signature: None,
        url: None,
        // No Next package-manager distribution exists. In particular, never
        // tell a Next user to install the upstream Scoop package.
        scoop_install: false,
        deb_install: false,
    })
}

/// Whether the running executable is managed by Scoop (Windows package manager).
#[allow(unreachable_code)]
pub fn is_scoop_install() -> bool {
    #[cfg(target_os = "windows")]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            let roots: Vec<String> = ["SCOOP", "SCOOP_GLOBAL"]
                .into_iter()
                .filter_map(|name| std::env::var_os(name))
                .map(|path| path.to_string_lossy().into_owned())
                .collect();
            return is_scoop_install_path(&exe_path.to_string_lossy(), &roots);
        }
        return false;
    }

    false
}

/// Pure path check for Scoop-managed executables (case-insensitive).
#[cfg(any(target_os = "windows", test))]
fn is_scoop_install_path(exe_path: &str, roots: &[String]) -> bool {
    let normalize = |path: &str| {
        let normalized = path.replace('/', "\\").to_lowercase();
        let normalized = if let Some(suffix) = normalized.strip_prefix(r"\\?\unc\") {
            format!(r"\\{suffix}")
        } else {
            normalized
                .strip_prefix(r"\\?\")
                .unwrap_or(&normalized)
                .to_string()
        };
        normalized.trim_end_matches('\\').to_string()
    };
    let normalized_exe = normalize(exe_path);
    normalized_exe.contains("\\scoop\\apps\\")
        || roots
            .iter()
            .filter(|root| !root.trim().is_empty())
            .any(|root| normalized_exe.starts_with(&format!("{}\\apps\\", normalize(root))))
}

/// Whether the running executable is managed by the system package manager
/// (`dpkg`), i.e. the app was installed from the release `.deb`.
///
/// `tauri-plugin-updater` picks the install strategy from the *running binary*,
/// not from the payload: an executable under `/usr/...` that `dpkg -S` claims
/// selects its `install_deb` branch, and that branch rejects the AppImage bytes
/// published under `linux-x86_64` with `InvalidUpdaterFormat`. Publishing a deb
/// payload instead would not fix it either — `install_deb` needs `pkexec`,
/// `zenity` / `kdialog`, or an interactive terminal `sudo` for elevation, none
/// of which exist in a plain GUI session (and none at all inside WSL). So a deb
/// install can never self-update; report it and let the user reinstall the
/// package or switch to the AppImage build.
#[allow(unreachable_code)]
pub fn is_deb_install() -> bool {
    #[cfg(target_os = "linux")]
    {
        let Ok(exe_path) = std::env::current_exe() else {
            return false;
        };
        let exe_path = exe_path.to_string_lossy().into_owned();
        if !is_deb_install_path(&exe_path) {
            return false;
        }
        // Only trust `/usr/...` when the package database actually claims the
        // file, so an AppImage dropped into `/usr/local` by hand still keeps the
        // in-app updater.
        return std::process::Command::new("dpkg")
            .args(["-S", exe_path.as_str()])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
    }

    false
}

/// Pure path check for dpkg-managed executables: the deb bundle always installs
/// the binary to `/usr/bin/<name>`.
#[cfg(any(target_os = "linux", test))]
fn is_deb_install_path(exe_path: &str) -> bool {
    exe_path.starts_with("/usr/")
}

/// `is_deb_install` shells out to `dpkg`, which can block for a long time behind
/// a package-manager lock; keep it off the async runtime (see "Async Runtime
/// Safety" in `AGENTS.md`).
async fn is_deb_install_async() -> bool {
    tauri::async_runtime::spawn_blocking(is_deb_install)
        .await
        .unwrap_or(false)
}

/// Next intentionally has no signed in-app installer. Fail before reading
/// proxy credentials or changing process environment variables.
#[tauri::command]
pub async fn install_update(_app: tauri::AppHandle) -> Result<bool, String> {
    Err(format!(
        "AI Toolbox Next uses manual updates. Download from {NEXT_RELEASES_URL}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str) -> ManualRelease {
        ManualRelease {
            tag_name: tag.into(),
            html_url: format!("{NEXT_RELEASES_URL}/tag/{tag}"),
            body: None,
            draft: false,
        }
    }

    #[test]
    fn next_updates_are_manual_and_compare_prerelease_versions() {
        let result = manual_release_result(
            release("v1.2.1-next.2"),
            "1.2.1-next.1".into(),
            false,
            false,
        )
        .unwrap();
        assert!(result.has_update);
        assert!(result.url.is_none());
        assert!(result.signature.is_none());
        assert!(result.release_url.starts_with(NEXT_RELEASES_URL));
        let older = manual_release_result(
            release("v1.2.1-next.1"),
            "1.2.1-next.2".into(),
            false,
            false,
        )
        .unwrap();
        assert!(!older.has_update);
    }

    #[test]
    fn next_updates_reject_upstream_or_malformed_release_metadata() {
        let mut upstream = release("v1.2.1-next.2");
        upstream.html_url = "https://github.com/coulsontl/ai-toolbox/releases/tag/v1.2.1".into();
        assert!(manual_release_result(upstream, "1.2.1-next.1".into(), false, false).is_err());
        assert!(manual_release_result(
            release("not-a-version"),
            "1.2.1-next.1".into(),
            false,
            false
        )
        .is_err());
    }

    #[test]
    fn detects_per_user_scoop_install() {
        assert!(is_scoop_install_path(
            "C:\\Users\\foo\\scoop\\apps\\ai-toolbox\\1.1.4\\ai-toolbox.exe",
            &[],
        ));
    }

    #[test]
    fn detects_global_and_verbatim_paths_case_insensitively() {
        assert!(is_scoop_install_path(
            "C:\\Program Files\\Scoop\\apps\\ai-toolbox\\1.1.4\\ai-toolbox.exe",
            &[],
        ));
        assert!(is_scoop_install_path(
            "\\\\?\\D:\\Scoop\\Apps\\ai-toolbox\\1.1.4\\ai-toolbox.exe",
            &[],
        ));
    }

    #[test]
    fn rejects_non_scoop_paths() {
        assert!(!is_scoop_install_path(
            "C:\\Program Files\\AI Toolbox\\ai-toolbox.exe",
            &[],
        ));
        assert!(!is_scoop_install_path(
            "C:\\Users\\foo\\AppData\\Local\\AI Toolbox\\ai-toolbox.exe",
            &[],
        ));
        // "scoopless\apps" must not match "\scoop\apps\"
        assert!(!is_scoop_install_path(
            "C:\\Users\\foo\\scoopless\\apps\\ai-toolbox\\ai-toolbox.exe",
            &[],
        ));
    }

    #[test]
    fn detects_deb_install_paths() {
        // The deb bundle installs the binary to `/usr/bin/<name>`.
        assert!(is_deb_install_path("/usr/bin/ai-toolbox"));
        assert!(is_deb_install_path("/usr/lib/AI Toolbox/ai-toolbox"));
    }

    #[test]
    fn rejects_non_deb_install_paths() {
        // An AppImage anywhere outside `/usr` keeps the in-app updater.
        assert!(!is_deb_install_path(
            "/home/foo/Downloads/AI.Toolbox_1.1.7_amd64.AppImage"
        ));
        assert!(!is_deb_install_path(
            "/mnt/c/Users/foo/Downloads/AI.Toolbox_1.1.7_amd64.AppImage"
        ));
        // `/usrx` must not match the `/usr/` prefix.
        assert!(!is_deb_install_path("/usrx/bin/ai-toolbox"));
        // Windows paths never match.
        assert!(!is_deb_install_path(
            "C:\\Program Files\\AI Toolbox\\ai-toolbox.exe"
        ));
    }

    #[test]
    fn detects_custom_scoop_roots_without_matching_adjacent_directories() {
        let roots = vec![r"D:\Tools\".to_string(), "E:/Global Packages".to_string()];
        assert!(is_scoop_install_path(
            r"\\?\D:\Tools\apps\ai-toolbox\current\ai-toolbox.exe",
            &roots
        ));
        assert!(is_scoop_install_path(
            "E:/Global Packages/apps/ai-toolbox/1.1.4/ai-toolbox.exe",
            &roots
        ));
        assert!(!is_scoop_install_path(
            r"D:\Tools-old\apps\ai-toolbox\ai-toolbox.exe",
            &roots
        ));
        assert!(!is_scoop_install_path(
            r"D:\Tools\apps-backup\ai-toolbox.exe",
            &roots
        ));
        assert!(is_scoop_install_path(
            r"\\?\UNC\server\tools\apps\ai-toolbox\current\ai-toolbox.exe",
            &[r"\\server\tools".to_string()],
        ));
    }
}
