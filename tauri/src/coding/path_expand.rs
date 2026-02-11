//! Common Path Expansion Utilities
//!
//! Provides standardized path expansion for local file paths across modules (WSL, SSH, etc.):
//! - `~` expands to home directory via `dirs::home_dir()`
//! - `%USERPROFILE%`, `%APPDATA%`, `%LOCALAPPDATA%` expand to Windows env vars
//! - `$HOME`, `$USERPROFILE` expand to Unix-style env vars
//!
//! **Usage**:
//! ```rust
//! use super::path_expand::expand_local_path;
//!
//! let expanded = expand_local_path("~/.config/opencode/opencode.jsonc")?;
//! ```

/// Expand local path: `~`, `$HOME`, `%USERPROFILE%`, and other common env vars.
///
/// Supports both Unix (`~/`, `$HOME`) and Windows (`%USERPROFILE%`, `%APPDATA%`) conventions,
/// ensuring cross-platform compatibility regardless of which format is stored.
pub fn expand_local_path(path: &str) -> Result<String, String> {
    let mut result = path.to_string();

    // Expand ~ to home directory
    if result.starts_with("~/") || result == "~" {
        if let Some(home) = dirs::home_dir() {
            result = result.replacen("~", &home.to_string_lossy(), 1);
        }
    }

    // Common environment variables (Windows and Unix)
    let vars = [
        ("USERPROFILE", std::env::var("USERPROFILE")),
        ("APPDATA", std::env::var("APPDATA")),
        ("LOCALAPPDATA", std::env::var("LOCALAPPDATA")),
        (
            "HOME",
            std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")),
        ),
    ];

    for (var, value) in vars {
        if let Ok(val) = value {
            // Windows style: %VAR%
            result = result.replace(&format!("%{}%", var), &val);
            // Unix style: $VAR
            result = result.replace(&format!("${}", var), &val);
        }
    }

    Ok(result)
}
