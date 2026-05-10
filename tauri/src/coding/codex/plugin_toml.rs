use std::collections::HashMap;
use std::fs;
use std::path::Path;

use toml_edit::{value, DocumentMut, Item, Table};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodexPluginConfigState {
    pub plugins_feature_enabled: bool,
}

fn read_document(config_path: &Path) -> Result<DocumentMut, String> {
    if !config_path.exists() {
        return Ok(DocumentMut::new());
    }

    let content = fs::read_to_string(config_path)
        .map_err(|error| format!("Failed to read {}: {}", config_path.display(), error))?;
    content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse {}: {}", config_path.display(), error))
}

fn write_document(config_path: &Path, document: &DocumentMut) -> Result<(), String> {
    if let Some(parent_dir) = config_path.parent() {
        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir)
                .map_err(|error| format!("Failed to create {}: {}", parent_dir.display(), error))?;
        }
    }

    let rendered = document.to_string();
    fs::write(config_path, rendered)
        .map_err(|error| format!("Failed to write {}: {}", config_path.display(), error))
}

fn ensure_table<'a>(item: &'a mut Item) -> &'a mut Table {
    if !item.is_table() {
        *item = Item::Table(Table::new());
    }
    item.as_table_mut().expect("table ensured")
}

pub fn read_plugin_enabled_map(config_path: &Path) -> Result<HashMap<String, bool>, String> {
    let document = read_document(config_path)?;
    let mut enabled_map = HashMap::new();

    if let Some(plugins_table) = document.get("plugins").and_then(Item::as_table_like) {
        for (plugin_id, plugin_item) in plugins_table.iter() {
            let enabled = plugin_item
                .as_table_like()
                .and_then(|table| table.get("enabled"))
                .and_then(Item::as_bool)
                .unwrap_or(true);
            enabled_map.insert(plugin_id.to_string(), enabled);
        }
    }

    Ok(enabled_map)
}

pub fn read_plugin_config_state(config_path: &Path) -> Result<CodexPluginConfigState, String> {
    let document = read_document(config_path)?;
    let plugins_feature_enabled = document
        .get("features")
        .and_then(Item::as_table_like)
        .and_then(|table| table.get("plugins"))
        .and_then(Item::as_bool)
        .unwrap_or(false);

    Ok(CodexPluginConfigState {
        plugins_feature_enabled,
    })
}

pub fn set_plugins_feature_enabled(config_path: &Path, enabled: bool) -> Result<(), String> {
    let mut document = read_document(config_path)?;
    let features_table = ensure_table(document.entry("features").or_insert(Item::None));
    features_table["plugins"] = value(enabled);
    write_document(config_path, &document)
}

pub fn set_plugin_enabled(
    config_path: &Path,
    plugin_id: &str,
    enabled: bool,
) -> Result<(), String> {
    let mut document = read_document(config_path)?;
    let plugins_table = ensure_table(document.entry("plugins").or_insert(Item::None));
    let plugin_table = ensure_table(plugins_table.entry(plugin_id).or_insert(Item::None));
    plugin_table["enabled"] = value(enabled);
    write_document(config_path, &document)
}

pub fn remove_plugin_entry(config_path: &Path, plugin_id: &str) -> Result<(), String> {
    let mut document = read_document(config_path)?;
    if let Some(plugins_table) = document.get_mut("plugins").and_then(Item::as_table_mut) {
        plugins_table.remove(plugin_id);
    }
    write_document(config_path, &document)
}
