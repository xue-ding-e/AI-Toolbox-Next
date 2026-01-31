//! MCP Configuration File Synchronization
//!
//! Handles reading/writing MCP server configurations to various tool config files.
//! Supports JSON and TOML formats.

use std::path::PathBuf;

use serde_json::Value;

use super::types::{McpServer, McpSyncDetail, now_ms};
use crate::coding::tools::{resolve_mcp_config_path, RuntimeTool};

/// Sync an MCP server to a specific tool's config file
pub fn sync_server_to_tool(
    server: &McpServer,
    tool: &RuntimeTool,
) -> Result<McpSyncDetail, String> {
    let config_path = resolve_mcp_config_path(tool)
        .ok_or_else(|| format!("Tool {} does not support MCP", tool.key))?;

    let format = tool.mcp_config_format.as_deref().unwrap_or("json");
    let field = tool.mcp_field.as_deref().unwrap_or("mcpServers");

    match format {
        "json" => sync_server_to_json(&config_path, server, field),
        "toml" => sync_server_to_toml(&config_path, server, field),
        _ => Err(format!("Unsupported config format: {}", format)),
    }
    .map(|_| McpSyncDetail {
        tool: tool.key.clone(),
        status: "ok".to_string(),
        synced_at: Some(now_ms()),
        error_message: None,
    })
    .map_err(|e| e.to_string())
}

/// Remove an MCP server from a specific tool's config file
pub fn remove_server_from_tool(
    server_name: &str,
    tool: &RuntimeTool,
) -> Result<(), String> {
    let config_path = resolve_mcp_config_path(tool)
        .ok_or_else(|| format!("Tool {} does not support MCP", tool.key))?;

    let format = tool.mcp_config_format.as_deref().unwrap_or("json");
    let field = tool.mcp_field.as_deref().unwrap_or("mcpServers");

    match format {
        "json" => remove_server_from_json(&config_path, server_name, field),
        "toml" => remove_server_from_toml(&config_path, server_name, field),
        _ => Err(format!("Unsupported config format: {}", format)),
    }
}

/// Sync server to JSON config file
fn sync_server_to_json(
    config_path: &PathBuf,
    server: &McpServer,
    field: &str,
) -> Result<(), String> {
    // Read existing config or create new
    let mut config: Value = if config_path.exists() {
        let content = std::fs::read_to_string(config_path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse config file: {}", e))?
    } else {
        serde_json::json!({})
    };

    // Ensure parent directory exists
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    // Get or create the MCP servers field
    let mcp_servers = config
        .as_object_mut()
        .ok_or("Config is not a JSON object")?
        .entry(field)
        .or_insert(serde_json::json!({}));

    // Build server config based on type
    let server_config = build_json_server_config(server)?;

    // Add/update server
    mcp_servers
        .as_object_mut()
        .ok_or(format!("{} is not a JSON object", field))?
        .insert(server.name.clone(), server_config);

    // Write back to file with pretty formatting
    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    std::fs::write(config_path, content)
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    Ok(())
}

/// Remove server from JSON config file
fn remove_server_from_json(
    config_path: &PathBuf,
    server_name: &str,
    field: &str,
) -> Result<(), String> {
    if !config_path.exists() {
        return Ok(()); // Nothing to remove
    }

    let content = std::fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;
    let mut config: Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config file: {}", e))?;

    // Get the MCP servers field
    if let Some(mcp_servers) = config.get_mut(field) {
        if let Some(servers_obj) = mcp_servers.as_object_mut() {
            servers_obj.remove(server_name);
        }
    }

    // Write back to file
    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    std::fs::write(config_path, content)
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    Ok(())
}

/// Sync server to TOML config file
fn sync_server_to_toml(
    config_path: &PathBuf,
    server: &McpServer,
    field: &str,
) -> Result<(), String> {
    // Read existing config or create new
    let mut config: toml::Table = if config_path.exists() {
        let content = std::fs::read_to_string(config_path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        content.parse::<toml::Table>()
            .map_err(|e| format!("Failed to parse TOML config: {}", e))?
    } else {
        toml::Table::new()
    };

    // Ensure parent directory exists
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    // Get or create the MCP servers field
    let mcp_servers = config
        .entry(field.to_string())
        .or_insert(toml::Value::Table(toml::Table::new()));

    // Build server config based on type
    let server_config = build_toml_server_config(server)?;

    // Add/update server
    if let toml::Value::Table(servers_table) = mcp_servers {
        servers_table.insert(server.name.clone(), server_config);
    }

    // Write back to file
    let content = toml::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize TOML config: {}", e))?;
    std::fs::write(config_path, content)
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    Ok(())
}

/// Remove server from TOML config file
fn remove_server_from_toml(
    config_path: &PathBuf,
    server_name: &str,
    field: &str,
) -> Result<(), String> {
    if !config_path.exists() {
        return Ok(()); // Nothing to remove
    }

    let content = std::fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;
    let mut config: toml::Table = content.parse()
        .map_err(|e| format!("Failed to parse TOML config: {}", e))?;

    // Get the MCP servers field
    if let Some(toml::Value::Table(servers_table)) = config.get_mut(field) {
        servers_table.remove(server_name);
    }

    // Write back to file
    let content = toml::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize TOML config: {}", e))?;
    std::fs::write(config_path, content)
        .map_err(|e| format!("Failed to write config file: {}", e))?;

    Ok(())
}

/// Build JSON server configuration from McpServer
fn build_json_server_config(server: &McpServer) -> Result<Value, String> {
    match server.server_type.as_str() {
        "stdio" => {
            let command = server.server_config
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or("stdio server requires 'command' field")?;

            let args: Vec<String> = server.server_config
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();

            let mut config = serde_json::json!({
                "command": command,
                "args": args,
            });

            // Add env if present
            if let Some(env) = server.server_config.get("env") {
                if !env.is_null() {
                    config["env"] = env.clone();
                }
            }

            Ok(config)
        }
        "http" | "sse" => {
            let url = server.server_config
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or(format!("{} server requires 'url' field", server.server_type))?;

            let mut config = serde_json::json!({
                "url": url,
            });

            // Add headers if present
            if let Some(headers) = server.server_config.get("headers") {
                if !headers.is_null() {
                    config["headers"] = headers.clone();
                }
            }

            Ok(config)
        }
        _ => Err(format!("Unknown server type: {}", server.server_type)),
    }
}

/// Build TOML server configuration from McpServer
fn build_toml_server_config(server: &McpServer) -> Result<toml::Value, String> {
    match server.server_type.as_str() {
        "stdio" => {
            let command = server.server_config
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or("stdio server requires 'command' field")?;

            let args: Vec<String> = server.server_config
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();

            let mut table = toml::Table::new();
            table.insert("command".to_string(), toml::Value::String(command.to_string()));
            table.insert("args".to_string(), toml::Value::Array(
                args.into_iter().map(toml::Value::String).collect()
            ));

            // Add env if present
            if let Some(env) = server.server_config.get("env") {
                if let Some(env_obj) = env.as_object() {
                    let mut env_table = toml::Table::new();
                    for (k, v) in env_obj {
                        if let Some(s) = v.as_str() {
                            env_table.insert(k.clone(), toml::Value::String(s.to_string()));
                        }
                    }
                    if !env_table.is_empty() {
                        table.insert("env".to_string(), toml::Value::Table(env_table));
                    }
                }
            }

            Ok(toml::Value::Table(table))
        }
        "http" | "sse" => {
            let url = server.server_config
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or(format!("{} server requires 'url' field", server.server_type))?;

            let mut table = toml::Table::new();
            table.insert("url".to_string(), toml::Value::String(url.to_string()));

            // Add headers if present
            if let Some(headers) = server.server_config.get("headers") {
                if let Some(headers_obj) = headers.as_object() {
                    let mut headers_table = toml::Table::new();
                    for (k, v) in headers_obj {
                        if let Some(s) = v.as_str() {
                            headers_table.insert(k.clone(), toml::Value::String(s.to_string()));
                        }
                    }
                    if !headers_table.is_empty() {
                        table.insert("headers".to_string(), toml::Value::Table(headers_table));
                    }
                }
            }

            Ok(toml::Value::Table(table))
        }
        _ => Err(format!("Unknown server type: {}", server.server_type)),
    }
}

/// Import MCP servers from a tool's config file
pub fn import_servers_from_tool(tool: &RuntimeTool) -> Result<Vec<McpServer>, String> {
    let config_path = resolve_mcp_config_path(tool)
        .ok_or_else(|| format!("Tool {} does not support MCP", tool.key))?;

    if !config_path.exists() {
        return Ok(vec![]);
    }

    let format = tool.mcp_config_format.as_deref().unwrap_or("json");
    let field = tool.mcp_field.as_deref().unwrap_or("mcpServers");

    match format {
        "json" => import_servers_from_json(&config_path, field),
        "toml" => import_servers_from_toml(&config_path, field),
        _ => Err(format!("Unsupported config format: {}", format)),
    }
}

/// Import servers from JSON config file
fn import_servers_from_json(config_path: &PathBuf, field: &str) -> Result<Vec<McpServer>, String> {
    let content = std::fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;
    let config: Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config file: {}", e))?;

    let Some(mcp_servers) = config.get(field) else {
        return Ok(vec![]);
    };

    let Some(servers_obj) = mcp_servers.as_object() else {
        return Ok(vec![]);
    };

    let now = now_ms();
    let mut servers = Vec::new();

    for (name, server_config) in servers_obj {
        // Detect server type
        let server_type = if server_config.get("command").is_some() {
            "stdio"
        } else if server_config.get("url").is_some() {
            "http"
        } else {
            continue; // Skip unknown server types
        };

        servers.push(McpServer {
            id: String::new(), // Will be assigned when saved
            name: name.clone(),
            server_type: server_type.to_string(),
            server_config: server_config.clone(),
            enabled_tools: vec![],
            sync_details: None,
            description: None,
            tags: vec![],
            sort_index: 0,
            created_at: now,
            updated_at: now,
        });
    }

    Ok(servers)
}

/// Import servers from TOML config file
fn import_servers_from_toml(config_path: &PathBuf, field: &str) -> Result<Vec<McpServer>, String> {
    let content = std::fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;
    let config: toml::Table = content.parse()
        .map_err(|e| format!("Failed to parse TOML config: {}", e))?;

    let Some(toml::Value::Table(servers_table)) = config.get(field) else {
        return Ok(vec![]);
    };

    let now = now_ms();
    let mut servers = Vec::new();

    for (name, server_config) in servers_table {
        let toml::Value::Table(config_table) = server_config else {
            continue;
        };

        // Detect server type
        let server_type = if config_table.get("command").is_some() {
            "stdio"
        } else if config_table.get("url").is_some() {
            "http"
        } else {
            continue; // Skip unknown server types
        };

        // Convert TOML to JSON for storage
        let json_config = toml_to_json(server_config);

        servers.push(McpServer {
            id: String::new(),
            name: name.clone(),
            server_type: server_type.to_string(),
            server_config: json_config,
            enabled_tools: vec![],
            sync_details: None,
            description: None,
            tags: vec![],
            sort_index: 0,
            created_at: now,
            updated_at: now,
        });
    }

    Ok(servers)
}

/// Convert TOML Value to JSON Value
fn toml_to_json(toml_val: &toml::Value) -> Value {
    match toml_val {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(i) => Value::Number((*i).into()),
        toml::Value::Float(f) => {
            serde_json::Number::from_f64(*f)
                .map(Value::Number)
                .unwrap_or(Value::Null)
        }
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Array(arr) => Value::Array(arr.iter().map(toml_to_json).collect()),
        toml::Value::Table(table) => {
            let obj: serde_json::Map<String, Value> = table
                .iter()
                .map(|(k, v)| (k.clone(), toml_to_json(v)))
                .collect();
            Value::Object(obj)
        }
        toml::Value::Datetime(dt) => Value::String(dt.to_string()),
    }
}
