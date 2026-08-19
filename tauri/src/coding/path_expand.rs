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
            result = replace_unix_env_reference(&result, var, &val);
        }
    }

    Ok(result)
}

fn replace_unix_env_reference(input: &str, variable_name: &str, value: &str) -> String {
    let braced_reference = format!("${{{variable_name}}}");
    let input = input.replace(&braced_reference, value);
    let plain_reference = format!("${variable_name}");
    let mut output = String::with_capacity(input.len());
    let mut remaining = input.as_str();

    while let Some(index) = remaining.find(&plain_reference) {
        output.push_str(&remaining[..index]);
        let after_reference = &remaining[index + plain_reference.len()..];
        let continues_identifier = after_reference
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_');
        if continues_identifier {
            output.push_str(&plain_reference);
        } else {
            output.push_str(value);
        }
        remaining = after_reference;
    }
    output.push_str(remaining);
    output
}

#[cfg(test)]
mod tests {
    use super::replace_unix_env_reference;

    #[test]
    fn unix_env_expansion_matches_complete_variable_tokens_only() {
        assert_eq!(
            replace_unix_env_reference("$HOME/bin", "HOME", "/home/tester"),
            "/home/tester/bin"
        );
        assert_eq!(
            replace_unix_env_reference("${HOME}/bin", "HOME", "/home/tester"),
            "/home/tester/bin"
        );
        assert_eq!(
            replace_unix_env_reference("$HOMEBREW_PREFIX/bin", "HOME", "/home/tester"),
            "$HOMEBREW_PREFIX/bin"
        );
        assert_eq!(
            replace_unix_env_reference("$HOME_SUFFIX/bin", "HOME", "/home/tester"),
            "$HOME_SUFFIX/bin"
        );
    }
}
