//! MCP configuration sync to SSH remote
//!
//! Syncs MCP server configurations to remote Linux server for all MCP-enabled tools:
//! - Claude Code: directly edit ~/.claude.json mcpServers field
//! - OpenCode/Codex: sync config files via file mappings

use log::info;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use super::adapter;
use super::commands::resolve_dynamic_paths;
use super::sync::{read_remote_file, sync_mappings, write_remote_file};
use super::types::{SSHConnection, SSHFileMapping, SSHSyncConfig, SyncProgress};
use crate::coding::mcp::command_normalize;
use crate::coding::mcp::mcp_store;
use crate::DbState;

/// Read SSH sync config directly from database
async fn get_ssh_config(state: &DbState) -> Result<SSHSyncConfig, String> {
    let db = state.0.lock().await;

    let config_result: Result<Vec<serde_json::Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM ssh_sync_config:`config` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query SSH config: {}", e))?
        .take(0);

    let connections_result: Result<Vec<serde_json::Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM ssh_connection ORDER BY sort_order, name")
        .await
        .map_err(|e| format!("Failed to query SSH connections: {}", e))?
        .take(0);

    let connections = match connections_result {
        Ok(records) => records
            .into_iter()
            .map(adapter::connection_from_db_value)
            .collect(),
        Err(_) => vec![],
    };

    match config_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                Ok(adapter::config_from_db_value(record.clone(), vec![], connections))
            } else {
                Ok(SSHSyncConfig { connections, ..SSHSyncConfig::default() })
            }
        }
        Err(_) => Ok(SSHSyncConfig { connections, ..SSHSyncConfig::default() }),
    }
}

/// Get file mappings from database
async fn get_file_mappings(state: &DbState) -> Result<Vec<SSHFileMapping>, String> {
    let db = state.0.lock().await;

    let mappings_result: Result<Vec<serde_json::Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM ssh_file_mapping ORDER BY module, name")
        .await
        .map_err(|e| format!("Failed to query SSH file mappings: {}", e))?
        .take(0);

    match mappings_result {
        Ok(records) => Ok(records
            .into_iter()
            .map(adapter::mapping_from_db_value)
            .collect()),
        Err(_) => Ok(vec![]),
    }
}

/// Get active connection from config
fn get_active_connection(config: &SSHSyncConfig) -> Option<SSHConnection> {
    config
        .connections
        .iter()
        .find(|c| c.id == config.active_connection_id)
        .cloned()
}

/// Sync MCP configuration to SSH remote (called on mcp-changed event)
pub async fn sync_mcp_to_ssh(state: &DbState, app: AppHandle) -> Result<(), String> {
    let config = get_ssh_config(state).await?;

    if !config.enabled || !config.sync_mcp {
        return Ok(());
    }

    let conn = match get_active_connection(&config) {
        Some(c) => c,
        None => {
            log::warn!("SSH MCP sync skipped: no active connection");
            return Ok(());
        }
    };

    // Emit progress
    let _ = app.emit(
        "ssh-sync-progress",
        SyncProgress {
            phase: "mcp".to_string(),
            current_item: "Claude Code MCP".to_string(),
            current: 1,
            total: 2,
            message: "MCP 同步: Claude Code...".to_string(),
        },
    );

    // 1. Claude Code: directly modify remote ~/.claude.json
    let servers = mcp_store::get_mcp_servers(state).await?;
    let claude_servers: Vec<_> = servers
        .iter()
        .filter(|s| s.enabled_tools.contains(&"claude_code".to_string()))
        .collect();

    if let Err(e) = sync_mcp_to_ssh_claude(&conn, &claude_servers) {
        log::warn!("Skipped claude.json MCP sync: {}", e);
        let _ = app.emit(
            "ssh-sync-warning",
            format!(
                "SSH ~/.claude.json 同步已跳过：文件解析失败，请检查该文件格式是否正确。({})",
                e
            ),
        );
    }

    // Emit progress for OpenCode/Codex
    let _ = app.emit(
        "ssh-sync-progress",
        SyncProgress {
            phase: "mcp".to_string(),
            current_item: "OpenCode/Codex MCP".to_string(),
            current: 2,
            total: 2,
            message: "MCP 同步: OpenCode/Codex...".to_string(),
        },
    );

    // 2. OpenCode/Codex: sync config files via file mappings
    match get_file_mappings(state).await {
        Ok(file_mappings) => {
            let mcp_modules = ["opencode", "codex"];
            let mcp_mappings: Vec<_> = file_mappings
                .into_iter()
                .filter(|m| m.enabled && mcp_modules.contains(&m.module.as_str()))
                .collect();

            if !mcp_mappings.is_empty() {
                let resolved = resolve_dynamic_paths(mcp_mappings);
                let result = sync_mappings(&resolved, &conn, None);
                if !result.errors.is_empty() {
                    let msg = result.errors.join("; ");
                    log::warn!("MCP file mapping sync errors: {}", msg);
                    let _ = app.emit(
                        "ssh-sync-warning",
                        format!("OpenCode/Codex 配置同步部分失败：{}", msg),
                    );
                }

                // Post-process: strip cmd /c from synced MCP config files
                let synced_paths: std::collections::HashSet<String> = result
                    .synced_files
                    .iter()
                    .filter_map(|s| s.split(" -> ").nth(1).map(|p| p.to_string()))
                    .collect();
                for mapping in &resolved {
                    if mapping.enabled
                        && is_mcp_config_file(&mapping.id)
                        && synced_paths.contains(&mapping.remote_path)
                    {
                        if let Err(e) = strip_cmd_c_from_remote_mcp_file(
                            &conn,
                            &mapping.remote_path,
                            &mapping.module,
                        ) {
                            log::warn!(
                                "Failed to strip cmd /c from {}: {}",
                                mapping.remote_path,
                                e
                            );
                        }
                    }
                }
            }
        }
        Err(e) => {
            log::warn!("Skipped OpenCode/Codex MCP sync: {}", e);
            let _ = app.emit(
                "ssh-sync-warning",
                format!("OpenCode/Codex MCP 同步已跳过：{}", e),
            );
        }
    }

    info!(
        "MCP SSH sync completed: {} servers synced to claude_code",
        claude_servers.len()
    );

    // Update sync status
    let sync_result = super::types::SyncResult {
        success: true,
        synced_files: vec![],
        skipped_files: vec![],
        errors: vec![],
    };
    let _ = super::commands::update_sync_status(state, &sync_result).await;

    let _ = app.emit("ssh-mcp-sync-completed", ());
    let _ = app.emit("ssh-sync-completed", &sync_result);

    Ok(())
}

/// Sync MCP servers to remote Claude Code ~/.claude.json
fn sync_mcp_to_ssh_claude(
    conn: &SSHConnection,
    servers: &[&crate::coding::mcp::types::McpServer],
) -> Result<(), String> {
    let config_path = "~/.claude.json";

    // Read existing remote config
    let existing_content = read_remote_file(conn, config_path)?;

    // Parse JSON, update mcpServers field
    let mut config: Value = if existing_content.trim().is_empty() {
        serde_json::json!({})
    } else {
        json5::from_str(&existing_content)
            .map_err(|e| format!("Failed to parse remote claude.json: {}", e))?
    };

    // Build mcpServers object
    let mut mcp_servers = serde_json::Map::new();
    for server in servers {
        let server_config = build_standard_server_config(server);
        mcp_servers.insert(server.name.clone(), server_config);
    }

    // Update only mcpServers field
    config
        .as_object_mut()
        .ok_or("Remote claude.json is not a JSON object")?
        .insert("mcpServers".to_string(), Value::Object(mcp_servers));

    // Write back
    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    write_remote_file(conn, config_path, &content)?;

    Ok(())
}

/// Build standard JSON server config for Claude Code format
fn build_standard_server_config(server: &crate::coding::mcp::types::McpServer) -> Value {
    match server.server_type.as_str() {
        "stdio" => {
            let command = server
                .server_config
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let args: Vec<Value> = server
                .server_config
                .get("args")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let env = server.server_config.get("env").cloned();

            let mut result = serde_json::json!({
                "type": "stdio",
                "command": command,
                "args": args,
            });

            if let Some(env_val) = env {
                if env_val.is_object()
                    && !env_val
                        .as_object()
                        .map(|o| o.is_empty())
                        .unwrap_or(true)
                {
                    result["env"] = env_val;
                }
            }

            // Ensure no cmd /c for remote Linux
            command_normalize::unwrap_cmd_c(&result)
        }
        "http" | "sse" => {
            let url = server
                .server_config
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let headers = server.server_config.get("headers").cloned();

            let mut result = serde_json::json!({
                "type": &server.server_type,
                "url": url,
            });

            if let Some(headers_val) = headers {
                if headers_val.is_object()
                    && !headers_val
                        .as_object()
                        .map(|o| o.is_empty())
                        .unwrap_or(true)
                {
                    result["headers"] = headers_val;
                }
            }

            result
        }
        _ => server.server_config.clone(),
    }
}

/// Check if a file mapping ID corresponds to an MCP config file
fn is_mcp_config_file(mapping_id: &str) -> bool {
    matches!(
        mapping_id,
        "opencode-main" | "opencode-oh-my" | "codex-config"
    )
}

/// Strip cmd /c from remote MCP config file after sync
fn strip_cmd_c_from_remote_mcp_file(
    conn: &SSHConnection,
    remote_path: &str,
    module: &str,
) -> Result<(), String> {
    let content = read_remote_file(conn, remote_path)?;
    if content.trim().is_empty() {
        return Ok(());
    }

    let processed = match module {
        "opencode" => command_normalize::process_opencode_json(&content, false)?,
        "codex" => {
            if remote_path.ends_with(".toml") {
                command_normalize::process_codex_toml(&content, false)?
            } else {
                return Ok(());
            }
        }
        _ => return Ok(()),
    };

    if processed != content {
        write_remote_file(conn, remote_path, &processed)?;
        info!("Stripped cmd /c from remote MCP config: {}", remote_path);
    }

    Ok(())
}
