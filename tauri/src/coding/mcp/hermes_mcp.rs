//! Hermes MCP sync and import adapter.
//!
//! Handles conversion between ai-toolbox's unified MCP format and Hermes
//! `config.yaml`'s `mcp_servers:` section (YAML).
//!
//! ## Format mapping
//!
//! | Unified (JSON)                                   | Hermes config.yaml (YAML)        |
//! |--------------------------------------------------|----------------------------------|
//! | `{"type":"stdio","command":"npx","args":[...]}` | `command: npx`, `args: [...]`    |
//! | `{"type":"sse"/"http","url":"...","headers":{}}` | `url: "..."`, `headers: {}`      |
//!
//! Key differences from the standard (Claude) format:
//! - Hermes has NO explicit `type` field — it infers stdio (has `command`) vs
//!   HTTP (has `url`).
//! - Hermes has extra per-server fields (`enabled`, `timeout`, `connect_timeout`,
//!   `tools`, `sampling`, `roots`, `auth`). These are preserved on merge-on-write
//!   and stripped on import.
//!
//! This adapter uses serde_yaml round-trip (consistent with config_sync's
//! json/toml paths). Comment preservation is handled by `read_yaml_object_or_empty`
//! healing duplicate keys; the `mcp_servers` section is machine-managed, not
//! hand-commented. The comment-preserving section splice in `hermes::commands`
//! is only used for provider/model/other-settings edits.

use std::path::Path;

use serde_json::{json, Value};

use super::command_normalize;
use super::types::{now_ms, McpServer};
use super::yaml_sync::{read_yaml_object_or_empty, write_yaml_section};

/// Hermes-specific fields preserved on merge-on-write, stripped on import.
/// Update this list when Hermes adds new per-server config fields.
const HERMES_EXTRA_FIELDS: &[&str] = &[
    "enabled",
    "timeout",
    "connect_timeout",
    "tools",
    "sampling",
    "roots",
    "auth",
];

// ============================================================================
// Format Conversion: Unified -> Hermes
// ============================================================================

/// Convert a unified MCP server spec to Hermes format.
///
/// - `stdio`: output `command`, `args`, `env` (strip `type` field)
/// - `sse`/`http`: output `url`, `headers` (strip `type` field)
/// - Always add `enabled: true`
pub(crate) fn convert_to_hermes_format(server: &McpServer, enabled: bool) -> Result<Value, String> {
    let server_type = server.server_type.as_str();
    let obj = server
        .server_config
        .as_object()
        .ok_or_else(|| "MCP server_config must be a JSON object".to_string())?;

    let mut result = serde_json::Map::new();

    match server_type {
        "stdio" => {
            if let Some(command) = obj.get("command") {
                result.insert("command".to_string(), command.clone());
            }
            if let Some(args) = obj.get("args") {
                if args.is_array() && !args.as_array().map(|a| a.is_empty()).unwrap_or(true) {
                    result.insert("args".to_string(), args.clone());
                }
            }
            if let Some(env) = obj.get("env") {
                if env.is_object() && !env.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                    result.insert("env".to_string(), env.clone());
                }
            }
        }
        "sse" | "http" => {
            if let Some(url) = obj.get("url") {
                result.insert("url".to_string(), url.clone());
            }
            if let Some(headers) = obj.get("headers") {
                if headers.is_object()
                    && !headers.as_object().map(|o| o.is_empty()).unwrap_or(true)
                {
                    result.insert("headers".to_string(), headers.clone());
                }
            }
        }
        _ => {
            return Err(format!("Unknown MCP type: {server_type}"));
        }
    }

    result.insert("enabled".to_string(), json!(enabled));

    Ok(Value::Object(result))
}

// ============================================================================
// Format Conversion: Hermes -> Unified
// ============================================================================

/// Convert a Hermes MCP server spec to a unified `McpServer`.
///
/// - If `command` exists: `server_type="stdio"`, extract `command`/`args`/`env`
/// - If `url` exists: `server_type="sse"`, extract `url`/`headers`
/// - Strip Hermes-specific fields (`enabled`, `timeout`, `tools`, etc.)
pub(crate) fn convert_from_hermes_format(name: &str, spec: &Value) -> Option<McpServer> {
    let obj = spec
        .as_object()
        .ok_or_else(|| "Hermes MCP spec must be a JSON object".to_string())
        .ok()?;

    let now = now_ms();

    if obj.contains_key("command") {
        // stdio type
        let mut unified = serde_json::Map::new();
        unified.insert("type".to_string(), json!("stdio"));

        if let Some(command) = obj.get("command") {
            unified.insert("command".to_string(), command.clone());
        }
        if let Some(args) = obj.get("args") {
            if args.is_array() && !args.as_array().map(|a| a.is_empty()).unwrap_or(true) {
                unified.insert("args".to_string(), args.clone());
            }
        }
        if let Some(env) = obj.get("env") {
            if env.is_object() && !env.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                unified.insert("env".to_string(), env.clone());
            }
        }

        let server_config = command_normalize::unwrap_cmd_c(&Value::Object(unified));
        Some(McpServer {
            id: String::new(),
            name: name.to_string(),
            server_type: "stdio".to_string(),
            server_config,
            enabled_tools: vec![],
            sync_details: None,
            description: None,
            user_group: None,
            user_note: None,
            tags: vec![],
            timeout: None,
            sort_index: 0,
            management_enabled: true,
            disabled_previous_tools: Vec::new(),
            created_at: now,
            updated_at: now,
        })
    } else if obj.contains_key("url") {
        // HTTP/SSE type
        let mut unified = serde_json::Map::new();
        unified.insert("type".to_string(), json!("sse"));

        if let Some(url) = obj.get("url") {
            unified.insert("url".to_string(), url.clone());
        }
        if let Some(headers) = obj.get("headers") {
            if headers.is_object()
                && !headers.as_object().map(|o| o.is_empty()).unwrap_or(true)
            {
                unified.insert("headers".to_string(), headers.clone());
            }
        }

        Some(McpServer {
            id: String::new(),
            name: name.to_string(),
            server_type: "sse".to_string(),
            server_config: Value::Object(unified),
            enabled_tools: vec![],
            sync_details: None,
            description: None,
            user_group: None,
            user_note: None,
            tags: vec![],
            timeout: None,
            sort_index: 0,
            management_enabled: true,
            disabled_previous_tools: Vec::new(),
            created_at: now,
            updated_at: now,
        })
    } else {
        log::warn!("Hermes MCP server '{name}' has neither 'command' nor 'url' field");
        None
    }
}

// ============================================================================
// Merge-on-write
// ============================================================================

/// Merge new spec into existing Hermes spec, preserving Hermes-specific fields.
///
/// Core fields (`command`, `args`, `env`, `url`, `headers`) come from `new_spec`.
/// Hermes-specific fields are kept from `existing` — this prevents ai-toolbox
/// from overwriting user customizations.
pub(crate) fn merge_hermes_spec(existing: &Value, new_spec: &Value) -> Value {
    let mut result = serde_json::Map::new();

    // Copy Hermes-specific fields from existing config
    if let Some(existing_obj) = existing.as_object() {
        for &field in HERMES_EXTRA_FIELDS {
            if let Some(val) = existing_obj.get(field) {
                result.insert(field.to_string(), val.clone());
            }
        }
    }

    // Overwrite with core fields from new spec; for Hermes-specific fields,
    // only apply from new_spec if existing didn't already have them
    if let Some(new_obj) = new_spec.as_object() {
        for (key, val) in new_obj {
            if HERMES_EXTRA_FIELDS.contains(&key.as_str()) && result.contains_key(key) {
                continue; // Existing Hermes-specific field takes precedence
            }
            result.insert(key.clone(), val.clone());
        }
    }

    Value::Object(result)
}

// ============================================================================
// Public API: Sync / Remove / Import
// ============================================================================

/// Sync a single MCP server to Hermes config.yaml (merge-on-write).
///
/// Uses byte-level section replacement (`write_yaml_section`) so comments and
/// unrelated top-level sections (model, custom_providers, agent, etc.) survive
/// untouched — only the `mcp_servers:` section is rewritten.
pub(crate) fn sync_server_to_hermes(
    config_path: &Path,
    server: &McpServer,
    enabled: bool,
) -> Result<(), String> {
    let _guard = crate::coding::hermes::commands::hermes_write_lock()
        .lock()
        .unwrap_or_else(|poisoned| {
            log::warn!("Hermes write lock was poisoned; recovering");
            poisoned.into_inner()
        });
    let mut config = read_yaml_object_or_empty(config_path)?;

    let hermes_spec = convert_to_hermes_format(server, enabled)?;
    let name = server.name.clone();

    let mcp_servers = config
        .as_object_mut()
        .ok_or_else(|| "config.yaml root must be a YAML mapping".to_string())?
        .entry("mcp_servers".to_string())
        .or_insert_with(|| json!({}));

    let merged = if let Some(existing) = mcp_servers.get(&name) {
        merge_hermes_spec(existing, &hermes_spec)
    } else {
        hermes_spec
    };

    if let Some(servers_obj) = mcp_servers.as_object_mut() {
        servers_obj.insert(name, merged);
    } else {
        // mcp_servers was not an object (corrupt); replace it
        *mcp_servers = json!({ name: merged });
    }

    // Write only the mcp_servers section back, preserving the rest of the file.
    write_yaml_section(config_path, "mcp_servers", mcp_servers)
}

/// Remove a single MCP server from Hermes config.yaml.
///
/// Uses byte-level section replacement so comments and unrelated sections survive.
pub(crate) fn remove_server_from_hermes(
    config_path: &Path,
    server_name: &str,
) -> Result<(), String> {
    let _guard = crate::coding::hermes::commands::hermes_write_lock()
        .lock()
        .unwrap_or_else(|poisoned| {
            log::warn!("Hermes write lock was poisoned; recovering");
            poisoned.into_inner()
        });
    let mut config = read_yaml_object_or_empty(config_path)?;

    let mcp_servers = if let Some(servers) = config
        .as_object_mut()
        .and_then(|obj| obj.get_mut("mcp_servers"))
    {
        servers
    } else {
        // No mcp_servers section — nothing to remove.
        return Ok(());
    };

    let changed = if let Some(servers_obj) = mcp_servers.as_object_mut() {
        servers_obj.remove(server_name).is_some()
    } else {
        false
    };

    if !changed {
        return Ok(());
    }

    write_yaml_section(config_path, "mcp_servers", mcp_servers)
}

/// Import MCP servers from Hermes config.yaml.
pub(crate) fn import_servers_from_hermes(config_path: &Path) -> Result<Vec<McpServer>, String> {
    let config = read_yaml_object_or_empty(config_path)?;

    let Some(mcp_servers) = config.get("mcp_servers").and_then(|v| v.as_object()) else {
        return Ok(vec![]);
    };

    let mut servers = Vec::new();
    for (name, spec) in mcp_servers {
        if let Some(server) = convert_from_hermes_format(name, spec) {
            servers.push(server);
        }
    }
    Ok(servers)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stdio_server(name: &str, command: &str, args: &[&str], env: &[(&str, &str)]) -> McpServer {
        let mut server_config = json!({
            "type": "stdio",
            "command": command,
            "args": args,
        });
        if !env.is_empty() {
            let env_obj: serde_json::Map<String, Value> = env
                .iter()
                .map(|(k, v)| (k.to_string(), json!(v)))
                .collect();
            server_config["env"] = Value::Object(env_obj);
        }
        McpServer {
            id: String::new(),
            name: name.to_string(),
            server_type: "stdio".to_string(),
            server_config,
            enabled_tools: vec![],
            sync_details: None,
            description: None,
            user_group: None,
            user_note: None,
            tags: vec![],
            timeout: None,
            sort_index: 0,
            management_enabled: true,
            disabled_previous_tools: vec![],
            created_at: 0,
            updated_at: 0,
        }
    }

    fn make_http_server(name: &str, url: &str, headers: &[(&str, &str)]) -> McpServer {
        let mut server_config = json!({
            "type": "sse",
            "url": url,
        });
        if !headers.is_empty() {
            let headers_obj: serde_json::Map<String, Value> = headers
                .iter()
                .map(|(k, v)| (k.to_string(), json!(v)))
                .collect();
            server_config["headers"] = Value::Object(headers_obj);
        }
        McpServer {
            id: String::new(),
            name: name.to_string(),
            server_type: "sse".to_string(),
            server_config,
            enabled_tools: vec![],
            sync_details: None,
            description: None,
            user_group: None,
            user_note: None,
            tags: vec![],
            timeout: None,
            sort_index: 0,
            management_enabled: true,
            disabled_previous_tools: vec![],
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn test_convert_stdio_to_hermes() {
        let server = make_stdio_server("fs", "npx", &["-y", "@mcp/server-filesystem"], &[("HOME", "/u")]);
        let result = convert_to_hermes_format(&server, true).unwrap();
        assert!(result.get("type").is_none());
        assert_eq!(result["command"], "npx");
        assert_eq!(result["args"][0], "-y");
        assert_eq!(result["env"]["HOME"], "/u");
        assert_eq!(result["enabled"], true);
    }

    #[test]
    fn test_convert_http_to_hermes() {
        let server = make_http_server("remote", "https://example.com/mcp", &[("Authorization", "Bearer x")]);
        let result = convert_to_hermes_format(&server, true).unwrap();
        assert!(result.get("type").is_none());
        assert_eq!(result["url"], "https://example.com/mcp");
        assert_eq!(result["headers"]["Authorization"], "Bearer x");
        assert_eq!(result["enabled"], true);
    }

    #[test]
    fn test_convert_stdio_empty_env_to_hermes() {
        let server = make_stdio_server("node", "node", &[], &[]);
        let result = convert_to_hermes_format(&server, true).unwrap();
        assert_eq!(result["command"], "node");
        assert!(result.get("args").is_none());
        assert!(result.get("env").is_none());
        assert_eq!(result["enabled"], true);
    }

    #[test]
    fn test_convert_from_hermes_stdio_strips_extra_fields() {
        let spec = json!({
            "command": "npx",
            "args": ["-y", "@mcp/server-filesystem"],
            "env": { "HOME": "/u" },
            "enabled": true,
            "timeout": 30,
            "connect_timeout": 10,
            "tools": { "include": ["read_file"] },
            "sampling": { "enabled": true },
            "roots": { "uri": "file:///x" },
            "auth": "oauth",
        });
        let result = convert_from_hermes_format("filesystem", &spec).unwrap();
        assert_eq!(result.server_type, "stdio");
        assert_eq!(result.server_config["command"], "npx");
        assert_eq!(result.server_config["args"][0], "-y");
        assert_eq!(result.server_config["env"]["HOME"], "/u");
        // Hermes-specific fields stripped
        for field in HERMES_EXTRA_FIELDS {
            assert!(result.server_config.get(field).is_none(), "field {field} should be stripped");
        }
    }

    #[test]
    fn test_convert_from_hermes_http_strips_extra_fields() {
        let spec = json!({
            "url": "https://example.com/mcp",
            "headers": { "Authorization": "Bearer x" },
            "enabled": true,
            "timeout": 60,
            "auth": "oauth",
        });
        let result = convert_from_hermes_format("remote", &spec).unwrap();
        assert_eq!(result.server_type, "sse");
        assert_eq!(result.server_config["url"], "https://example.com/mcp");
        assert_eq!(result.server_config["headers"]["Authorization"], "Bearer x");
        for field in HERMES_EXTRA_FIELDS {
            assert!(result.server_config.get(field).is_none(), "field {field} should be stripped");
        }
    }

    #[test]
    fn test_convert_from_hermes_no_command_no_url_returns_none() {
        let spec = json!({ "enabled": true, "timeout": 30 });
        assert!(convert_from_hermes_format("bad", &spec).is_none());
    }

    #[test]
    fn test_merge_preserves_hermes_specific_fields() {
        let existing = json!({
            "command": "old-cmd",
            "args": ["old-arg"],
            "enabled": true,
            "timeout": 30,
            "connect_timeout": 10,
            "tools": { "include": ["read_file"] },
            "sampling": { "enabled": true },
            "roots": { "uri": "file:///x" },
            "auth": "oauth",
        });
        let new_spec = json!({
            "command": "new-cmd",
            "args": ["new-arg"],
            "env": { "KEY": "value" },
            "enabled": true,
        });
        let merged = merge_hermes_spec(&existing, &new_spec);
        // Core fields overwritten
        assert_eq!(merged["command"], "new-cmd");
        assert_eq!(merged["args"][0], "new-arg");
        assert_eq!(merged["env"]["KEY"], "value");
        // Hermes-specific fields preserved from existing
        assert_eq!(merged["timeout"], 30);
        assert_eq!(merged["connect_timeout"], 10);
        assert_eq!(merged["tools"]["include"][0], "read_file");
        assert_eq!(merged["sampling"]["enabled"], true);
        assert_eq!(merged["roots"]["uri"], "file:///x");
        assert_eq!(merged["auth"], "oauth");
        assert_eq!(merged["enabled"], true);
    }

    #[test]
    fn test_merge_new_server_no_existing_extra_fields() {
        let existing = json!({ "command": "old-cmd" });
        let new_spec = json!({
            "command": "new-cmd",
            "args": ["arg1"],
            "enabled": true,
        });
        let merged = merge_hermes_spec(&existing, &new_spec);
        assert_eq!(merged["command"], "new-cmd");
        assert_eq!(merged["args"][0], "arg1");
        assert_eq!(merged["enabled"], true);
        assert!(merged.get("timeout").is_none());
    }

    #[test]
    fn test_sync_then_import_roundtrip() {
        let dir = std::env::temp_dir().join(format!("hermes_mcp_test_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.yaml");

        let server = make_stdio_server("fs", "npx", &["-y", "@mcp/server-fs"], &[]);
        sync_server_to_hermes(&config_path, &server, true).unwrap();

        let imported = import_servers_from_hermes(&config_path).unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].name, "fs");
        assert_eq!(imported[0].server_type, "stdio");
        assert_eq!(imported[0].server_config["command"], "npx");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_sync_preserves_other_mcp_servers() {
        let dir = std::env::temp_dir().join(format!("hermes_mcp_test_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.yaml");

        let server_a = make_stdio_server("fs", "npx", &["-y", "@mcp/a"], &[]);
        sync_server_to_hermes(&config_path, &server_a, true).unwrap();
        let server_b = make_http_server("remote", "https://example.com/mcp", &[]);
        sync_server_to_hermes(&config_path, &server_b, true).unwrap();

        let imported = import_servers_from_hermes(&config_path).unwrap();
        assert_eq!(imported.len(), 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_sync_update_preserves_hermes_extra_fields() {
        let dir = std::env::temp_dir().join(format!("hermes_mcp_test_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.yaml");

        // First sync
        let server = make_stdio_server("fs", "npx", &["-y", "@mcp/a"], &[]);
        sync_server_to_hermes(&config_path, &server, true).unwrap();

        // Manually add a Hermes-specific field
        let config = read_yaml_object_or_empty(&config_path).unwrap();
        let mut config = config;
        config["mcp_servers"]["fs"]["timeout"] = json!(45);
        let mcp_servers = config.get("mcp_servers").cloned().unwrap_or(json!({}));
        write_yaml_section(&config_path, "mcp_servers", &mcp_servers).unwrap();

        // Re-sync with updated command; timeout must survive
        let server_updated = make_stdio_server("fs", "node", &["-y", "@mcp/b"], &[]);
        sync_server_to_hermes(&config_path, &server_updated, true).unwrap();

        let config = read_yaml_object_or_empty(&config_path).unwrap();
        assert_eq!(config["mcp_servers"]["fs"]["command"], "node");
        assert_eq!(config["mcp_servers"]["fs"]["timeout"], 45);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_remove_server() {
        let dir = std::env::temp_dir().join(format!("hermes_mcp_test_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.yaml");

        let server = make_stdio_server("fs", "npx", &["-y", "@mcp/a"], &[]);
        sync_server_to_hermes(&config_path, &server, true).unwrap();

        remove_server_from_hermes(&config_path, "fs").unwrap();
        let imported = import_servers_from_hermes(&config_path).unwrap();
        assert!(imported.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_remove_missing_server_is_noop() {
        let dir = std::env::temp_dir().join(format!("hermes_mcp_test_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.yaml");

        let server = make_stdio_server("fs", "npx", &["-y", "@mcp/a"], &[]);
        sync_server_to_hermes(&config_path, &server, true).unwrap();

        // Remove a non-existent server
        remove_server_from_hermes(&config_path, "nonexistent").unwrap();
        let imported = import_servers_from_hermes(&config_path).unwrap();
        assert_eq!(imported.len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_sync_preserves_comments_and_unrelated_sections() {
        let dir = std::env::temp_dir().join(format!("hermes_mcp_test_{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.yaml");

        // Pre-populate with comments and unrelated sections
        let initial = "\
# Hermes configuration
model:
  default: gpt-4  # user comment
  provider: openai
mcp_servers:
  existing:
    command: node
    enabled: true
agent:
  max_turns: 50
# trailing comment
";
        std::fs::write(&config_path, initial).unwrap();

        // Sync a new server
        let server = make_stdio_server("memory", "npx", &["-y", "@mcp/memory"], &[]);
        sync_server_to_hermes(&config_path, &server, true).unwrap();

        let result = std::fs::read_to_string(&config_path).unwrap();
        // Comments must survive
        assert!(result.contains("# Hermes configuration"), "top comment lost");
        assert!(result.contains("# user comment"), "inline comment lost");
        assert!(result.contains("# trailing comment"), "trailing comment lost");
        // Unrelated sections must survive
        assert!(result.contains("model:"), "model section lost");
        assert!(result.contains("default: gpt-4"), "model content lost");
        assert!(result.contains("provider: openai"), "provider content lost");
        // New server must be present
        assert!(result.contains("memory:"));
        assert!(result.contains("command: npx"));
        // Existing server must survive
        assert!(result.contains("existing:"));
        assert!(result.contains("command: node"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
