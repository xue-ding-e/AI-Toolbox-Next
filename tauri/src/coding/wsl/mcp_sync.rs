//! MCP configuration sync to WSL
//!
//! Syncs MCP server configurations to WSL for all MCP-enabled tools:
//! - Claude Code: directly edit ~/.claude.json mcpServers field
//! - OpenCode/Codex: sync config files via file mappings

use log::info;
use serde_json::Value;
use tauri::AppHandle;

use super::adapter;
use super::commands::resolve_dynamic_paths;
use super::sync::{read_wsl_file, sync_mappings, write_wsl_file};
use super::types::{FileMapping, WSLSyncConfig};
use crate::coding::mcp::command_normalize;
use crate::coding::mcp::mcp_store;
use crate::DbState;

/// Read WSL sync config directly from database (without tauri::State wrapper)
async fn get_wsl_config(state: &DbState) -> Result<WSLSyncConfig, String> {
    let db = state.0.lock().await;

    let config_result: Result<Vec<serde_json::Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM wsl_sync_config:`config` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query WSL config: {}", e))?
        .take(0);

    match config_result {
        Ok(records) => {
            if let Some(record) = records.first() {
                Ok(adapter::config_from_db_value(record.clone(), vec![]))
            } else {
                Ok(WSLSyncConfig::default())
            }
        }
        Err(_) => Ok(WSLSyncConfig::default()),
    }
}

/// Get file mappings from database
async fn get_file_mappings(state: &DbState) -> Result<Vec<FileMapping>, String> {
    let db = state.0.lock().await;

    let mappings_result: Result<Vec<serde_json::Value>, _> = db
        .query("SELECT *, type::string(id) as id FROM wsl_file_mapping ORDER BY module, name")
        .await
        .map_err(|e| format!("Failed to query file mappings: {}", e))?
        .take(0);

    match mappings_result {
        Ok(records) => Ok(records
            .into_iter()
            .map(adapter::mapping_from_db_value)
            .collect()),
        Err(_) => Ok(vec![]),
    }
}

/// Sync MCP configuration to WSL (called on mcp-changed event)
pub async fn sync_mcp_to_wsl(state: &DbState, app: AppHandle) -> Result<(), String> {
    let config = get_wsl_config(state).await?;

    if !config.enabled || !config.sync_mcp {
        return Ok(());
    }

    let distro = &config.distro;

    // 1. Claude Code: directly modify WSL ~/.claude.json
    let servers = mcp_store::get_mcp_servers(state).await?;
    let claude_servers: Vec<_> = servers
        .iter()
        .filter(|s| s.enabled_tools.contains(&"claude_code".to_string()))
        .collect();

    if let Err(e) = sync_mcp_to_wsl_claude(distro, &claude_servers) {
        log::warn!("Skipped claude.json MCP sync: {}", e);
        let _ = tauri::Emitter::emit(
            &app,
            "wsl-sync-warning",
            format!("WSL ~/.claude.json 同步已跳过：文件解析失败，请检查该文件格式是否正确。({})", e),
        );
    }

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
                let result = sync_mappings(&resolved, distro, None);
                if !result.errors.is_empty() {
                    let msg = result.errors.join("; ");
                    log::warn!("MCP file mapping sync errors: {}", msg);
                    let _ = tauri::Emitter::emit(
                        &app,
                        "wsl-sync-warning",
                        format!("OpenCode/Codex 配置同步部分失败：{}", msg),
                    );
                }

                // Post-process: strip cmd /c from synced files (WSL is Linux, doesn't need it)
                for mapping in &resolved {
                    if mapping.enabled {
                        if let Err(e) = strip_cmd_c_from_wsl_mcp_file(distro, &mapping.wsl_path, &mapping.module) {
                            log::warn!("Failed to strip cmd /c from {}: {}", mapping.wsl_path, e);
                        }
                    }
                }
            }
        }
        Err(e) => {
            log::warn!("Skipped OpenCode/Codex MCP sync: {}", e);
            let _ = tauri::Emitter::emit(
                &app,
                "wsl-sync-warning",
                format!("OpenCode/Codex MCP 同步已跳过：{}", e),
            );
        }
    }

    info!(
        "MCP WSL sync completed: {} servers synced to claude_code",
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

    // Emit event for UI feedback
    let _ = tauri::Emitter::emit(&app, "wsl-mcp-sync-completed", ());
    let _ = tauri::Emitter::emit(&app, "wsl-sync-completed", &sync_result);

    Ok(())
}

/// Sync MCP servers to WSL Claude Code ~/.claude.json
fn sync_mcp_to_wsl_claude(
    distro: &str,
    servers: &[&crate::coding::mcp::types::McpServer],
) -> Result<(), String> {
    let wsl_config_path = "~/.claude.json";

    // 1. Read existing WSL ~/.claude.json
    let existing_content = read_wsl_file(distro, wsl_config_path)?;

    // 2. Parse JSON, update mcpServers field
    let mut config: Value = if existing_content.trim().is_empty() {
        serde_json::json!({})
    } else {
        json5::from_str(&existing_content)
            .map_err(|e| format!("Failed to parse WSL claude.json: {}", e))?
    };

    // 3. Build mcpServers object
    let mut mcp_servers = serde_json::Map::new();
    for server in servers {
        let server_config = build_standard_server_config(server);
        mcp_servers.insert(server.name.clone(), server_config);
    }

    // 4. Update only the mcpServers field, preserve other fields
    config
        .as_object_mut()
        .ok_or("WSL claude.json is not a JSON object")?
        .insert("mcpServers".to_string(), Value::Object(mcp_servers));

    // 5. Write back to WSL
    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    write_wsl_file(distro, wsl_config_path, &content)?;

    Ok(())
}

/// Build standard JSON server config for Claude Code format
/// Note: Database stores normalized config (no cmd /c), but we add a safeguard here
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

            // Safeguard: ensure no cmd /c for WSL (database should already be normalized)
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

/// Strip cmd /c from WSL MCP config file after sync
fn strip_cmd_c_from_wsl_mcp_file(distro: &str, wsl_path: &str, module: &str) -> Result<(), String> {
    let content = read_wsl_file(distro, wsl_path)?;
    if content.trim().is_empty() {
        return Ok(());
    }

    let processed = match module {
        "opencode" => command_normalize::process_opencode_json(&content, false)?,
        "codex" => command_normalize::process_codex_toml(&content, false)?,
        _ => return Ok(()),
    };

    // Only write back if content changed
    if processed != content {
        write_wsl_file(distro, wsl_path, &processed)?;
        info!("Stripped cmd /c from WSL MCP config: {}", wsl_path);
    }

    Ok(())
}
