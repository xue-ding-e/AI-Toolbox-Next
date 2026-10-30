//! Common Path Expansion Utilities
//!
//! Provides standardized path expansion for local file paths across modules (WSL, SSH, etc.):
//! - `~` expands to home directory via `dirs::home_dir()`
//! - `%USERPROFILE%`, `%APPDATA%`, `%LOCALAPPDATA%` expand to Windows env vars
//! - `$HOME`, `$USERPROFILE` expand to Unix-style env vars
//!
//! **Usage**:
//! ```rust
//! use ai_toolbox_lib::coding::expand_local_path;
//!
//! let expanded = expand_local_path("~/.config/opencode/opencode.jsonc").expect("path expands");
//! assert!(!expanded.is_empty());
//! ```

/// Expand local path: `~`, `$HOME`, `%USERPROFILE%`, and other common env vars.
///
/// Supports both Unix (`~/`, `$HOME`) and Windows (`%USERPROFILE%`, `%APPDATA%`) conventions,
/// ensuring cross-platform compatibility regardless of which format is stored.
pub fn expand_local_path(path: &str) -> Result<String, String> {
    ai_toolbox_paths::expand_local_path(path)
}
