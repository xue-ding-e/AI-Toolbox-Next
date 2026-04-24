use serde_json::Value;

const KNOWN_MARKETPLACES_FILE_NAME: &str = "known_marketplaces.json";
const INSTALLED_PLUGINS_FILE_NAME: &str = "installed_plugins.json";

fn normalize_windows_style_path(raw_path: &str) -> String {
    let normalized = raw_path.trim().replace('\\', "/");
    let without_trailing = normalized.trim_end_matches('/').to_string();

    if without_trailing.len() >= 2 && without_trailing.as_bytes()[1] == b':' {
        let mut chars = without_trailing.chars();
        let drive_letter = chars
            .next()
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default();
        return format!("{}{}", drive_letter, chars.as_str());
    }

    without_trailing
}

fn map_windows_plugin_path_to_target(
    raw_path: &str,
    source_plugins_root: &str,
    target_plugins_root: &str,
) -> Option<String> {
    let normalized_source_root = normalize_windows_style_path(source_plugins_root);
    let normalized_raw_path = normalize_windows_style_path(raw_path);

    if normalized_source_root.is_empty() || normalized_raw_path.is_empty() {
        return None;
    }

    if normalized_raw_path == normalized_source_root {
        return Some(target_plugins_root.trim_end_matches('/').to_string());
    }

    let required_prefix = format!("{}/", normalized_source_root);
    if !normalized_raw_path.starts_with(&required_prefix) {
        return None;
    }

    let relative_path = normalized_raw_path[required_prefix.len()..].trim_start_matches('/');
    Some(format!(
        "{}/{}",
        target_plugins_root.trim_end_matches('/'),
        relative_path
    ))
}

fn rewrite_known_marketplaces_install_locations(
    root_value: &mut Value,
    source_plugins_root: &str,
    target_plugins_root: &str,
) -> bool {
    let Some(marketplaces) = root_value.as_object_mut() else {
        return false;
    };

    let mut changed = false;
    for marketplace_value in marketplaces.values_mut() {
        let Some(marketplace_object) = marketplace_value.as_object_mut() else {
            continue;
        };
        let Some(current_location) = marketplace_object
            .get("installLocation")
            .and_then(Value::as_str)
        else {
            continue;
        };

        let Some(next_location) = map_windows_plugin_path_to_target(
            current_location,
            source_plugins_root,
            target_plugins_root,
        ) else {
            continue;
        };

        if next_location == current_location {
            continue;
        }

        marketplace_object.insert("installLocation".to_string(), Value::String(next_location));
        changed = true;
    }

    changed
}

fn rewrite_installed_plugin_paths(
    root_value: &mut Value,
    source_plugins_root: &str,
    target_plugins_root: &str,
) -> bool {
    let Some(root_object) = root_value.as_object_mut() else {
        return false;
    };
    let Some(plugins_value) = root_object.get_mut("plugins") else {
        return false;
    };
    let Some(plugins_object) = plugins_value.as_object_mut() else {
        return false;
    };

    let mut changed = false;
    for install_entries_value in plugins_object.values_mut() {
        let Some(install_entries) = install_entries_value.as_array_mut() else {
            continue;
        };

        for install_entry_value in install_entries.iter_mut() {
            let Some(install_entry_object) = install_entry_value.as_object_mut() else {
                continue;
            };
            let Some(current_install_path) = install_entry_object
                .get("installPath")
                .and_then(Value::as_str)
            else {
                continue;
            };

            let Some(next_install_path) = map_windows_plugin_path_to_target(
                current_install_path,
                source_plugins_root,
                target_plugins_root,
            ) else {
                continue;
            };

            if next_install_path == current_install_path {
                continue;
            }

            install_entry_object
                .insert("installPath".to_string(), Value::String(next_install_path));
            changed = true;
        }
    }

    changed
}

fn rewrite_plugin_metadata_file_content(
    file_name: &str,
    raw_content: &str,
    source_plugins_root: &str,
    target_plugins_root: &str,
) -> Result<Option<String>, String> {
    let mut root_value = serde_json::from_str::<Value>(raw_content)
        .map_err(|error| format!("Failed to parse {}: {}", file_name, error))?;

    let changed = match file_name {
        KNOWN_MARKETPLACES_FILE_NAME => rewrite_known_marketplaces_install_locations(
            &mut root_value,
            source_plugins_root,
            target_plugins_root,
        ),
        INSTALLED_PLUGINS_FILE_NAME => rewrite_installed_plugin_paths(
            &mut root_value,
            source_plugins_root,
            target_plugins_root,
        ),
        _ => false,
    };

    if !changed {
        return Ok(None);
    }

    let serialized = serde_json::to_string_pretty(&root_value)
        .map_err(|error| format!("Failed to serialize {}: {}", file_name, error))?;
    Ok(Some(format!("{serialized}\n")))
}

pub fn rewrite_claude_plugin_metadata_if_needed(
    file_name: &str,
    raw_content: &str,
    source_plugins_root: &str,
    target_plugins_root: &str,
) -> Result<Option<String>, String> {
    rewrite_plugin_metadata_file_content(
        file_name,
        raw_content,
        source_plugins_root,
        target_plugins_root,
    )
}

#[cfg(test)]
mod tests {
    use super::rewrite_claude_plugin_metadata_if_needed;

    #[test]
    fn rewrites_known_marketplace_install_location_to_target_root() {
        let source_root = r"C:\Users\Tester\.claude\plugins";
        let target_root = "/home/tester/.claude/plugins";
        let raw_content = r#"{
  "claude-plugins-official": {
    "source": {
      "source": "github",
      "repo": "anthropics/claude-plugins-official"
    },
    "installLocation": "C:\\Users\\Tester\\.claude\\plugins\\marketplaces\\claude-plugins-official"
  }
}"#;

        let rewritten = rewrite_claude_plugin_metadata_if_needed(
            "known_marketplaces.json",
            raw_content,
            source_root,
            target_root,
        )
        .expect("rewrite known marketplaces")
        .expect("expected rewritten content");

        assert!(rewritten.contains(
            r#""installLocation": "/home/tester/.claude/plugins/marketplaces/claude-plugins-official""#
        ));
    }

    #[test]
    fn rewrites_installed_plugin_install_paths_to_target_root() {
        let source_root = r"C:\Users\Tester\.claude\plugins";
        let target_root = "/home/tester/.claude/plugins";
        let raw_content = r#"{
  "version": 2,
  "plugins": {
    "typescript-lsp@claude-plugins-official": [
      {
        "scope": "user",
        "installPath": "C:\\Users\\Tester\\.claude\\plugins\\cache\\claude-plugins-official\\typescript-lsp\\1.0.0",
        "version": "1.0.0"
      }
    ]
  }
}"#;

        let rewritten = rewrite_claude_plugin_metadata_if_needed(
            "installed_plugins.json",
            raw_content,
            source_root,
            target_root,
        )
        .expect("rewrite installed plugins")
        .expect("expected rewritten content");

        assert!(rewritten.contains(
            r#""installPath": "/home/tester/.claude/plugins/cache/claude-plugins-official/typescript-lsp/1.0.0""#
        ));
    }

    #[test]
    fn leaves_non_plugin_paths_unchanged() {
        let source_root = r"C:\Users\Tester\.claude\plugins";
        let target_root = "/home/tester/.claude/plugins";
        let raw_content = r#"{
  "claude-plugins-official": {
    "installLocation": "D:\\Elsewhere\\claude-plugins-official"
  }
}"#;

        let rewritten = rewrite_claude_plugin_metadata_if_needed(
            "known_marketplaces.json",
            raw_content,
            source_root,
            target_root,
        )
        .expect("rewrite known marketplaces");

        assert!(rewritten.is_none());
    }
}
