//! Claude Code Plugin Discovery
//!
//! Reads installed Claude Code plugins from ~/.claude/plugins/installed_plugins.json
//! and returns their metadata for use by MCP scan and Skills onboarding.
//!
//! The actual file format (v2) is:
//! ```json
//! {
//!   "version": 2,
//!   "plugins": {
//!     "plugin-id@marketplace": [
//!       { "scope": "user", "installPath": "...", "version": "1.0.0", ... }
//!     ]
//!   }
//! }
//! ```

use std::path::PathBuf;

use serde::Deserialize;

/// Root structure of installed_plugins.json
#[derive(Debug, Deserialize)]
struct InstalledPluginsFile {
    plugins: std::collections::HashMap<String, Vec<PluginInstallEntry>>,
}

/// A single install entry for a plugin
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginInstallEntry {
    install_path: String,
}

/// Resolved plugin info returned to callers
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub plugin_id: String,
    pub display_name: String,
    pub install_path: PathBuf,
}

/// Read ~/.claude/plugins/installed_plugins.json and return metadata for each installed plugin.
///
/// Takes the first install entry per plugin (latest install).
/// Returns an empty Vec (not an error) when the file is missing or cannot be parsed,
/// so callers never have to worry about the "no plugins installed" case.
pub fn get_installed_plugins() -> Vec<PluginInfo> {
    let Some(home) = dirs::home_dir() else {
        return vec![];
    };

    let plugins_file = home.join(".claude").join("plugins").join("installed_plugins.json");
    if !plugins_file.exists() {
        return vec![];
    }

    let content = match std::fs::read_to_string(&plugins_file) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let file: InstalledPluginsFile = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let mut result = Vec::new();

    for (plugin_id, entries) in &file.plugins {
        // Take the first entry (latest install)
        let Some(entry) = entries.first() else {
            continue;
        };

        let install_path = PathBuf::from(&entry.install_path);
        if !install_path.exists() {
            continue;
        }

        let display_name = extract_display_name(plugin_id);

        result.push(PluginInfo {
            plugin_id: plugin_id.clone(),
            display_name,
            install_path,
        });
    }

    result
}

/// Extract a human-readable display name from a plugin_id.
/// e.g. "context7@claude-plugins-official" → "context7"
///      "my-plugin" → "my-plugin"
fn extract_display_name(plugin_id: &str) -> String {
    plugin_id
        .split('@')
        .next()
        .unwrap_or(plugin_id)
        .to_string()
}
