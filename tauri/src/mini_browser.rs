//! Minimal embedded browser window for checking relay dashboards (API balance,
//! usage, backend data) without leaving the toolbox.
//!
//! Why a separate native webview instead of an `<iframe>` in the main window:
//! relay consoles almost always send `X-Frame-Options: DENY` (or a
//! `frame-ancestors` CSP), so an iframe renders a blank box. A top-level
//! navigation is not subject to those headers.
//!
//! Memory: the window is built on the platform webview the app already loads —
//! WebView2 on Windows, WebKitGTK on Linux, WKWebView on macOS — so nothing
//! ships a second browser engine. The window is created on first use and
//! destroyed when closed, so an unused toolbox pays nothing.
//!
//! Security: this window loads third-party pages, so it must never be able to
//! reach the app's own commands. Two properties hold that line:
//! 1. It is created from Rust, and its label (`mini-browser`) is deliberately
//!    absent from `capabilities/default.json`, whose `windows` list is
//!    `["main"]`. The app's ACL therefore grants it no command access.
//! 2. Navigation is restricted to `http`/`https` by [`is_navigable_url`], so a
//!    page cannot walk the window into `file://` or `javascript:` territory.

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

/// Window label. Doubles as the singleton key: opening the browser twice
/// navigates the existing window instead of stacking duplicates.
pub const MINI_BROWSER_LABEL: &str = "mini-browser";

/// Saved-page list is bounded so a runaway caller cannot grow it forever.
const MAX_URL_LEN: usize = 2048;

/// Normalise user input into an absolute `http`/`https` URL.
///
/// A bare `relay.example.com/console` gets an `https://` prefix (the common
/// case when typing a host). Anything with a non-web scheme — `javascript:`,
/// `file:`, `data:` — is rejected rather than silently rewritten, because those
/// are the vectors that would let a pasted string escape the browser sandbox.
pub fn normalise_browser_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Enter a URL to open".to_string());
    }
    if trimmed.len() > MAX_URL_LEN {
        return Err("That URL is too long".to_string());
    }
    // Reject control characters before parsing: they can be used to smuggle a
    // second value past a naive consumer.
    if trimmed.chars().any(char::is_control) {
        return Err("That URL contains control characters".to_string());
    }

    let has_scheme = trimmed.split_once("://").is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
    });
    let candidate = if has_scheme {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };

    let parsed = tauri::Url::parse(&candidate).map_err(|error| format!("Invalid URL: {error}"))?;
    if !is_navigable_url(&parsed) {
        return Err("Only http and https addresses can be opened".to_string());
    }
    if parsed.host_str().is_none() {
        return Err("That URL is missing a host".to_string());
    }
    Ok(parsed.to_string())
}

/// Whether the mini browser may navigate to `url`.
///
/// Used both for user input and as the window's `on_navigation` guard, so a
/// link clicked inside the loaded page cannot escape to `file://`.
fn is_navigable_url(url: &tauri::Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

/// Isolated profile directory so relay cookies survive restarts without mixing
/// with the toolbox's own webview storage.
fn browser_data_dir() -> std::path::PathBuf {
    crate::app_paths::resolved_data_dir().join("mini-browser")
}

fn open_window<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    url: tauri::Url,
) -> Result<(), String> {
    // Reuse the existing window when present: one browser, not a pile of them.
    if let Some(existing) = app.get_webview_window(MINI_BROWSER_LABEL) {
        existing
            .navigate(url)
            .map_err(|error| format!("Failed to navigate the browser window: {error}"))?;
        let _ = existing.show();
        let _ = existing.unminimize();
        let _ = existing.set_focus();
        return Ok(());
    }

    let builder = WebviewWindowBuilder::new(app, MINI_BROWSER_LABEL, WebviewUrl::External(url))
        .title("AI Toolbox Browser")
        .inner_size(1100.0, 800.0)
        .min_inner_size(480.0, 360.0)
        .center()
        .data_directory(browser_data_dir())
        // Reject non-web schemes here too: this also covers redirects and
        // `target=_blank` handled by the webview itself, so a page cannot walk
        // the window into `file://` or `javascript:`.
        .on_navigation(is_navigable_url);

    let window = builder
        .build()
        .map_err(|error| format!("Failed to open the browser window: {error}"))?;
    let _ = window.set_focus();
    Ok(())
}

/// Open a page in the embedded browser, creating the window if needed.
#[tauri::command]
pub async fn mini_browser_open<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    url: String,
) -> Result<(), String> {
    let normalised = normalise_browser_url(&url)?;
    let parsed = tauri::Url::parse(&normalised).map_err(|error| format!("Invalid URL: {error}"))?;
    open_window(&app, parsed)
}

/// Navigate the already-open browser window.
#[tauri::command]
pub async fn mini_browser_navigate<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    url: String,
) -> Result<(), String> {
    mini_browser_open(app, url).await
}

/// Address currently shown, or `None` when the window is not open.
#[tauri::command]
pub fn mini_browser_current_url<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<String>, String> {
    let Some(window) = app.get_webview_window(MINI_BROWSER_LABEL) else {
        return Ok(None);
    };
    window
        .url()
        .map(|url| Some(url.to_string()))
        .map_err(|error| format!("Failed to read the browser address: {error}"))
}

/// Whether the browser window currently exists.
#[tauri::command]
pub fn mini_browser_is_open<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> bool {
    app.get_webview_window(MINI_BROWSER_LABEL).is_some()
}

/// Close the browser window. No-op when it is not open.
#[tauri::command]
pub fn mini_browser_close<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(MINI_BROWSER_LABEL) {
        window
            .close()
            .map_err(|error| format!("Failed to close the browser window: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_adds_https_to_a_bare_host() {
        assert_eq!(
            normalise_browser_url("relay.example.com").unwrap(),
            "https://relay.example.com/"
        );
        assert_eq!(
            normalise_browser_url("  relay.example.com/console  ").unwrap(),
            "https://relay.example.com/console"
        );
        assert_eq!(
            normalise_browser_url("http://127.0.0.1:3000/balance").unwrap(),
            "http://127.0.0.1:3000/balance"
        );
    }

    #[test]
    fn normalise_keeps_an_explicit_scheme() {
        assert_eq!(
            normalise_browser_url("https://api.example.com/usage?tab=credits").unwrap(),
            "https://api.example.com/usage?tab=credits"
        );
    }

    #[test]
    fn normalise_rejects_non_web_schemes() {
        // These are the strings that would otherwise escape the sandbox.
        assert!(normalise_browser_url("javascript:alert(1)").is_err());
        assert!(normalise_browser_url("file:///C:/Windows/win.ini").is_err());
        assert!(normalise_browser_url("data:text/html,<h1>x</h1>").is_err());
        assert!(normalise_browser_url("ms-settings:privacy").is_err());
        assert!(normalise_browser_url("").is_err());
        assert!(normalise_browser_url("   ").is_err());
    }

    #[test]
    fn normalise_rejects_control_characters_and_overlong_input() {
        assert!(normalise_browser_url("https://ok.example.com/\nheader").is_err());
        assert!(
            normalise_browser_url(&format!("https://example.com/{}", "a".repeat(3000))).is_err()
        );
    }

    #[test]
    fn navigable_url_guard_allows_only_web_schemes() {
        assert!(is_navigable_url(
            &tauri::Url::parse("https://a.example.com").unwrap()
        ));
        assert!(is_navigable_url(
            &tauri::Url::parse("http://a.example.com").unwrap()
        ));
        assert!(!is_navigable_url(
            &tauri::Url::parse("file:///etc/passwd").unwrap()
        ));
        assert!(!is_navigable_url(
            &tauri::Url::parse("javascript:alert(1)").unwrap()
        ));
    }
}
