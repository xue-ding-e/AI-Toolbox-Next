//! MCP configuration sync to SSH remote
//!
//! Syncs MCP server configurations to remote Linux server for all MCP-enabled tools:
//! - Claude Code: directly edit ~/.claude.json mcpServers field
//! - OpenCode/Codex: sync config files via file mappings

use log::info;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use super::commands::resolve_dynamic_paths;
use super::session::SshSession;
use super::sync::{read_remote_file, sync_mappings, write_remote_file};
use super::types::{SSHFileMapping, SyncProgress};
use crate::coding::mcp::command_normalize;
use crate::coding::mcp::mcp_store;
use crate::DbState;

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
            .map(super::adapter::mapping_from_db_value)
            .collect()),
        Err(_) => Ok(vec![]),
    }
}

/// Sync MCP configuration to SSH remote (called on mcp-changed event)
pub async fn sync_mcp_to_ssh(
    state: &DbState,
    session: &SshSession,
    app: AppHandle,
) -> Result<(), String> {
    let db = state.0.lock().await;
    let config = super::commands::get_ssh_config_internal(&db, false).await?;
    drop(db);

    if !config.enabled || !config.sync_mcp {
        return Ok(());
    }

    // 收集所有错误
    let mut all_errors: Vec<String> = vec![];

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

    if let Err(e) = sync_mcp_to_ssh_claude(session, &claude_servers).await {
        log::warn!("Skipped claude.json MCP sync: {}", e);
        all_errors.push(format!("Claude Code: {}", e));
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
                let result = sync_mappings(&resolved, session, None).await;
                if !result.errors.is_empty() {
                    let msg = result.errors.join("; ");
                    log::warn!("MCP file mapping sync errors: {}", msg);
                    all_errors.push(format!("OpenCode/Codex: {}", msg));
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
                            session,
                            &mapping.remote_path,
                            &mapping.module,
                        ).await {
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
            all_errors.push(format!("OpenCode/Codex: {}", e));
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

    if !all_errors.is_empty() {
        return Err(all_errors.join("; "));
    }

    let _ = app.emit("ssh-mcp-sync-completed", ());

    Ok(())
}

/// Sync MCP servers to remote Claude Code ~/.claude.json
async fn sync_mcp_to_ssh_claude(
    session: &SshSession,
    servers: &[&crate::coding::mcp::types::McpServer],
) -> Result<(), String> {
    let config_path = "~/.claude.json";

    // Read existing remote config
    let existing_content = read_remote_file(session, config_path).await?;

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
    write_remote_file(session, config_path, &content).await?;

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
async fn strip_cmd_c_from_remote_mcp_file(
    session: &SshSession,
    remote_path: &str,
    module: &str,
) -> Result<(), String> {
    let content = read_remote_file(session, remote_path).await?;
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
        write_remote_file(session, remote_path, &processed).await?;
        info!("Stripped cmd /c from remote MCP config: {}", remote_path);
    }

    Ok(())
}
