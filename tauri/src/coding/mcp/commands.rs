//! Tauri commands for MCP Server management
//!
//! Provides the public API for the MCP feature.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Runtime, State};

use super::adapter::parse_sync_details_dto;
use super::config_sync::{
    import_servers_from_path, import_servers_from_plugin_mcp_json, import_servers_from_tool_async,
    remove_server_from_tool_async, sync_server_to_tool_async,
    sync_server_to_tool_with_enabled_async,
};
use super::mcp_store;
use super::package_version;
use super::types::{
    now_ms, CreateMcpServerInput, FavoriteMcp, FavoriteMcpDto, FavoriteMcpInput,
    McpDiscoveredServerDto, McpGroup, McpGroupInventoryPreviewDto, McpImportResultDto,
    McpPackageVersionResolveRequest, McpPackageVersionResolveResult, McpScanResultDto, McpServer,
    McpServerDto, McpSyncDetail, McpSyncResultDto, UpdateMcpServerInput,
};
use crate::coding::tools::{
    custom_store, get_mcp_runtime_tools, is_tool_installed_with_db_async,
    resolve_mcp_config_path_with_db_async, runtime_tool_by_key, to_runtime_tool_dto_with_db_async,
    CustomTool, RuntimeTool, RuntimeToolDto,
};
use crate::SqliteDbState;
use crate::management_admission::{
    admit_non_codex_targets, current_profile_session, reject_managed_operation,
    CrossTargetAdmission, GuiProfileSession,
};

struct McpOperationAdmission {
    session: Option<std::sync::Arc<GuiProfileSession>>,
    targets: Option<CrossTargetAdmission>,
}

impl McpOperationAdmission {
    fn ensure_current(&self) -> Result<(), String> {
        if let Some(session) = &self.session {
            session.ensure_current()?;
        }
        if let Some(targets) = &self.targets {
            targets.ensure_current()?;
        }
        Ok(())
    }
}

fn server_admission_tools(server: &McpServer) -> Vec<String> {
    let mut keys = server.enabled_tools.clone();
    keys.extend(server.disabled_previous_tools.iter().cloned());
    if let Some(details) = server.sync_details.as_ref().and_then(serde_json::Value::as_object) {
        keys.extend(details.keys().cloned());
    }
    keys
}

/// Read the entire old/new write set before the first central DB mutation.
/// Installation checks are deliberately not a gate: stale/uninstalled tool
/// assignments may still be removed, restored, or aliased to a protected root.
async fn admit_mcp_operation(
    state: &SqliteDbState,
    mut keys: Vec<String>,
    include_disabled_opencode: bool,
) -> Result<McpOperationAdmission, String> {
    let session = current_profile_session()?;
    if !session.as_ref().is_some_and(|session| session.is_managed()) {
        // Ordinary mode retains its prior best-effort lookup/install behavior.
        return Ok(McpOperationAdmission { session, targets: None });
    }
    if include_disabled_opencode
        && mcp_store::get_mcp_preferences(state).await?.sync_disabled_to_opencode
    {
        keys.push("opencode".to_owned());
    }
    keys.sort();
    keys.dedup();
    if keys.iter().any(|key| key == "codex") {
        let _ordinary = reject_managed_operation()?;
    }
    let custom_tools = custom_store::get_custom_tools(state).await?;
    let db = state.db();
    let mut paths = Vec::new();
    for key in keys {
        let Some(tool) = runtime_tool_by_key(&key, &custom_tools) else {
            // Unknown targets may retain the ordinary best-effort behavior,
            // but cannot prove a managed operation safe before DB changes.
            let _ordinary = reject_managed_operation()?;
            continue;
        };
        let Some(path) = resolve_mcp_config_path_with_db_async(&db, &tool).await else {
            let _ordinary = reject_managed_operation()?;
            continue;
        };
        paths.extend(super::config_sync::mcp_write_targets(&tool, &path));
    }
    let targets: Vec<_> = paths.iter().map(PathBuf::as_path).collect();
    let targets = if targets.is_empty() {
        None
    } else {
        Some(admit_non_codex_targets(None, &targets)?)
    };
    Ok(McpOperationAdmission { session, targets })
}

async fn admit_custom_mcp_edit(
    state: &SqliteDbState,
    key: &str,
    next_path: Option<&str>,
) -> Result<McpOperationAdmission, String> {
    let session = current_profile_session()?;
    if !session.as_ref().is_some_and(|session| session.is_managed()) {
        return Ok(McpOperationAdmission { session, targets: None });
    }
    let previous = custom_store::get_custom_tool_by_key(state, key).await?;
    let mut tools: Vec<RuntimeTool> = previous.as_ref()
        .filter(|tool| tool.mcp_config_path.is_some())
        .map(RuntimeTool::from).into_iter().collect();
    if let Some(path) = next_path {
        tools.push(RuntimeTool {
            key: key.to_owned(), display_name: key.to_owned(), is_custom: true,
            relative_skills_dir: None, relative_detect_dir: None, force_copy: false,
            icon_url: None, mcp_config_path: Some(path.to_owned()),
            mcp_config_format: Some("json".to_owned()), mcp_field: Some("mcpServers".to_owned()),
        });
    }
    let mut paths = Vec::new();
    for tool in tools {
        let Some(path) = resolve_mcp_config_path_with_db_async(state, &tool).await else {
            let _ordinary = reject_managed_operation()?;
            continue;
        };
        paths.extend(super::config_sync::mcp_write_targets(&tool, &path));
    }
    let targets: Vec<_> = paths.iter().map(PathBuf::as_path).collect();
    let targets = if targets.is_empty() { None } else {
        Some(admit_non_codex_targets(None, &targets)?)
    };
    Ok(McpOperationAdmission { session, targets })
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

// ==================== MCP Server CRUD ====================

/// List all MCP servers
#[tauri::command]
pub async fn mcp_list_servers(
    state: State<'_, SqliteDbState>,
) -> Result<Vec<McpServerDto>, String> {
    let servers = mcp_store::get_mcp_servers(&state).await?;

    Ok(servers
        .into_iter()
        .map(|s| McpServerDto {
            id: s.id.clone(),
            name: s.name.clone(),
            server_type: s.server_type.clone(),
            server_config: s.server_config.clone(),
            enabled_tools: s.enabled_tools.clone(),
            sync_details: parse_sync_details_dto(&s),
            description: s.description.clone(),
            user_group: s.user_group.clone(),
            user_note: s.user_note.clone(),
            tags: s.tags.clone(),
            timeout: s.timeout,
            sort_index: s.sort_index,
            management_enabled: s.management_enabled,
            disabled_previous_tools: s.disabled_previous_tools.clone(),
            created_at: s.created_at,
            updated_at: s.updated_at,
        })
        .collect())
}

/// Resolve latest package versions for MCP stdio runner packages.
#[tauri::command]
pub async fn mcp_resolve_package_versions(
    state: State<'_, SqliteDbState>,
    requests: Vec<McpPackageVersionResolveRequest>,
) -> Result<Vec<McpPackageVersionResolveResult>, String> {
    Ok(package_version::resolve_package_versions(&state, requests).await)
}

/// Create a new MCP server
/// After creation, automatically sync to all enabled tools
#[tauri::command]
pub async fn mcp_create_server<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SqliteDbState>,
    input: CreateMcpServerInput,
) -> Result<McpServerDto, String> {
    let admission = admit_mcp_operation(&state, input.enabled_tools.clone(), true).await?;
    admission.ensure_current()?;
    let now = now_ms();
    let server = McpServer {
        id: String::new(), // Will be assigned by upsert
        name: input.name.clone(),
        server_type: input.server_type.clone(),
        server_config: input.server_config.clone(),
        enabled_tools: input.enabled_tools.clone(),
        sync_details: None,
        description: input.description,
        user_group: None,
        user_note: None,
        tags: input.tags,
        timeout: input.timeout,
        sort_index: 0, // Will be assigned by upsert
        management_enabled: true,
        disabled_previous_tools: Vec::new(),
        created_at: now,
        updated_at: now,
    };

    let id = mcp_store::upsert_mcp_server(&state, &server).await?;

    // Sync to all enabled tools
    let custom_tools = custom_store::get_custom_tools(&state)
        .await
        .unwrap_or_default();
    let db = state.db();
    for tool_key in &input.enabled_tools {
        admission.ensure_current()?;
        if let Some(tool) = runtime_tool_by_key(tool_key, &custom_tools) {
            if is_tool_installed_with_db_async(&db, &tool).await {
                match sync_server_to_tool_async(&db, &server, &tool).await {
                    Ok(detail) => {
                        admission.ensure_current()?;
                        let _ = mcp_store::update_sync_detail(&state, &id, &detail).await;
                    }
                    Err(e) => {
                        let detail = McpSyncDetail {
                            tool: tool_key.clone(),
                            status: "error".to_string(),
                            synced_at: Some(now_ms()),
                            error_message: Some(e),
                        };
                        admission.ensure_current()?;
                        let _ = mcp_store::update_sync_detail(&state, &id, &detail).await;
                    }
                }
            }
        }
    }

    // Sync disabled to opencode if the switch is ON and opencode is not in enabled_tools
    maybe_sync_disabled_to_opencode(&state, &server, &custom_tools).await;

    // Get the created server with sync details
    let created = mcp_store::get_mcp_server_by_id(&state, &id)
        .await?
        .ok_or("Failed to get created server")?;

    // Emit mcp-changed for WSL sync
    let _ = app.emit("config-changed", "window");
    let _ = app.emit("mcp-changed", "window");

    let sync_details = parse_sync_details_dto(&created);
    Ok(McpServerDto {
        id: created.id,
        name: created.name,
        server_type: created.server_type,
        server_config: created.server_config,
        enabled_tools: created.enabled_tools,
        sync_details,
        description: created.description,
        user_group: created.user_group,
        user_note: created.user_note,
        tags: created.tags,
        timeout: created.timeout,
        sort_index: created.sort_index,
        management_enabled: created.management_enabled,
        disabled_previous_tools: created.disabled_previous_tools,
        created_at: created.created_at,
        updated_at: created.updated_at,
    })
}

/// Update an existing MCP server
/// After update, automatically re-sync to all enabled tools
#[tauri::command]
#[allow(non_snake_case)]
pub async fn mcp_update_server<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SqliteDbState>,
    serverId: String,
    input: UpdateMcpServerInput,
) -> Result<McpServerDto, String> {
    let mut server = mcp_store::get_mcp_server_by_id(&state, &serverId)
        .await?
        .ok_or_else(|| format!("MCP server not found: {}", serverId))?;

    // Snapshot the previous name/enabled tools before applying updates so we
    // can clean up tool config files that are no longer selected (or were
    // written under the previous name).
    let previous_name = server.name.clone();
    let previous_enabled_tools = server.enabled_tools.clone();
    let mut admission_tools = server_admission_tools(&server);

    // Apply updates
    if let Some(name) = input.name {
        server.name = name;
    }
    if let Some(server_type) = input.server_type {
        server.server_type = server_type;
    }
    if let Some(server_config) = input.server_config {
        server.server_config = server_config;
    }
    if let Some(enabled_tools) = input.enabled_tools {
        server.enabled_tools = enabled_tools;
    }
    if let Some(description) = input.description {
        server.description = Some(description);
    }
    if let Some(tags) = input.tags {
        server.tags = tags;
    }
    server.timeout = input.timeout;
    server.updated_at = now_ms();

    admission_tools.extend(server_admission_tools(&server));
    let admission = admit_mcp_operation(&state, admission_tools, true).await?;
    admission.ensure_current()?;
    mcp_store::upsert_mcp_server(&state, &server).await?;

    // Re-sync to all enabled tools. A disabled server must stay out of every
    // runtime config, so the whole write-back section is skipped: the edited
    // values persist as desired state only and reach tool configs through the
    // documented re-enable + restore flow (same guard family as toggle /
    // sync / restore below).
    if server.management_enabled {
        run_update_sync(&state, &server, &previous_name, &previous_enabled_tools, &admission).await?;
    }

    // Get the updated server with sync details
    let updated = mcp_store::get_mcp_server_by_id(&state, &serverId)
        .await?
        .ok_or("Failed to get updated server")?;

    // Emit mcp-changed for WSL sync
    let _ = app.emit("config-changed", "window");
    let _ = app.emit("mcp-changed", "window");

    let sync_details = parse_sync_details_dto(&updated);
    Ok(McpServerDto {
        id: updated.id,
        name: updated.name,
        server_type: updated.server_type,
        server_config: updated.server_config,
        enabled_tools: updated.enabled_tools,
        sync_details,
        description: updated.description,
        user_group: updated.user_group,
        user_note: updated.user_note,
        tags: updated.tags,
        timeout: updated.timeout,
        sort_index: updated.sort_index,
        management_enabled: updated.management_enabled,
        disabled_previous_tools: updated.disabled_previous_tools,
        created_at: updated.created_at,
        updated_at: updated.updated_at,
    })
}

/// Write-back half of `mcp_update_server`: clean up configs for removed tools
/// and renamed servers, re-sync the remaining enabled tools, and apply the
/// opencode-disabled special case. Only called for management-enabled servers.
async fn run_update_sync(
    state: &State<'_, SqliteDbState>,
    server: &McpServer,
    previous_name: &str,
    previous_enabled_tools: &[String],
    admission: &McpOperationAdmission,
) -> Result<(), String> {
    admission.ensure_current()?;
    let custom_tools = custom_store::get_custom_tools(state)
        .await
        .unwrap_or_default();
    let db = state.db();
    let server_id = server.id.clone();

    // Clean up config files for tools that were removed from enabled_tools,
    // and for the previous name when the server was renamed: without this the
    // old entry stays in the tool config file and keeps loading at runtime.
    let removed_tools: Vec<&String> = previous_enabled_tools
        .iter()
        .filter(|tool| !server.enabled_tools.contains(tool))
        .collect();
    let name_changed = previous_name != server.name.as_str();
    for tool_key in removed_tools {
        admission.ensure_current()?;
        if let Some(tool) = runtime_tool_by_key(tool_key, &custom_tools) {
            if let Err(e) = remove_server_from_tool_async(&db, previous_name, &tool).await {
                log::warn!(
                    "Failed to remove MCP server '{}' from removed tool '{}' during update: {}",
                    previous_name,
                    tool_key,
                    e
                );
            }
            // Drop the stale sync detail so the UI no longer shows it.
            admission.ensure_current()?;
            let _ = mcp_store::delete_sync_detail(state, &server_id, tool_key).await;
        }
    }
    if name_changed {
        for tool_key in &server.enabled_tools {
            admission.ensure_current()?;
            if let Some(tool) = runtime_tool_by_key(tool_key, &custom_tools) {
                if let Err(e) = remove_server_from_tool_async(&db, previous_name, &tool).await {
                    log::warn!(
                        "Failed to remove renamed MCP server '{}' from tool '{}': {}",
                        previous_name,
                        tool_key,
                        e
                    );
                }
            }
        }
    }

    for tool_key in &server.enabled_tools {
        admission.ensure_current()?;
        if let Some(tool) = runtime_tool_by_key(tool_key, &custom_tools) {
            if is_tool_installed_with_db_async(&db, &tool).await {
                match sync_server_to_tool_async(&db, server, &tool).await {
                    Ok(detail) => {
                        admission.ensure_current()?;
                        let _ = mcp_store::update_sync_detail(state, &server_id, &detail).await;
                    }
                    Err(e) => {
                        let detail = McpSyncDetail {
                            tool: tool_key.clone(),
                            status: "error".to_string(),
                            synced_at: Some(now_ms()),
                            error_message: Some(e),
                        };
                        admission.ensure_current()?;
                        let _ = mcp_store::update_sync_detail(state, &server_id, &detail).await;
                    }
                }
            }
        }
    }

    // Sync disabled to opencode if the switch is ON and opencode is not in enabled_tools
    maybe_sync_disabled_to_opencode(state, server, &custom_tools).await;
    Ok(())
}

/// Delete an MCP server
#[tauri::command]
#[allow(non_snake_case)]
pub async fn mcp_delete_server<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SqliteDbState>,
    serverId: String,
) -> Result<(), String> {
    // Get the server first to remove from tool configs
    let server = mcp_store::get_mcp_server_by_id(&state, &serverId).await?;
    let admission = admit_mcp_operation(
        &state,
        server.as_ref().map(server_admission_tools).unwrap_or_default(),
        true,
    ).await?;
    admission.ensure_current()?;
    if let Some(server) = server {
        // Remove from all enabled tools' configs
        let custom_tools = custom_store::get_custom_tools(&state)
            .await
            .unwrap_or_default();
        let db = state.db();
        for tool_key in &server.enabled_tools {
            admission.ensure_current()?;
            if let Some(tool) = runtime_tool_by_key(tool_key, &custom_tools) {
                let _ = remove_server_from_tool_async(&db, &server.name, &tool).await;
            }
        }
        // Also remove from opencode if sync_disabled is ON
        maybe_remove_disabled_from_opencode(&state, &server, &custom_tools).await;
    }

    admission.ensure_current()?;
    mcp_store::delete_mcp_server(&state, &serverId).await?;

    // Emit mcp-changed for WSL sync
    let _ = app.emit("config-changed", "window");
    let _ = app.emit("mcp-changed", "window");

    Ok(())
}

/// Toggle a tool's enabled state for an MCP server
#[tauri::command]
#[allow(non_snake_case)]
pub async fn mcp_toggle_tool<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SqliteDbState>,
    serverId: String,
    toolKey: String,
) -> Result<bool, String> {
    let existing = mcp_store::get_mcp_server_by_id(&state, &serverId).await?;
    let mut admission_tools = existing.as_ref().map(server_admission_tools).unwrap_or_default();
    admission_tools.push(toolKey.clone());
    let admission = admit_mcp_operation(&state, admission_tools, false).await?;
    // Management-disabled servers are an invariant: rejects any tool toggle so the only
    // way back is re-enable + explicit sync. DB end state wins regardless of what the UI
    // sent, so we must check BEFORE flipping (toggling first would corrupt the record).
    if let Some(existing) = existing {
        if !existing.management_enabled {
            return Err(format!("MCP_DISABLED|{}", serverId));
        }
    }

    admission.ensure_current()?;
    let is_enabled = mcp_store::toggle_tool_enabled(&state, &serverId, &toolKey).await?;

    // Get the server
    let server = mcp_store::get_mcp_server_by_id(&state, &serverId)
        .await?
        .ok_or_else(|| format!("MCP server not found: {}", serverId))?;

    // Get the tool
    let custom_tools = custom_store::get_custom_tools(&state)
        .await
        .unwrap_or_default();
    let db = state.db();
    let tool = runtime_tool_by_key(&toolKey, &custom_tools)
        .ok_or_else(|| format!("Tool not found: {}", toolKey))?;

    // Sync or remove based on new state
    if is_enabled {
        // Sync to tool config
        match sync_server_to_tool_async(&db, &server, &tool).await {
            Ok(detail) => {
                admission.ensure_current()?;
                mcp_store::update_sync_detail(&state, &serverId, &detail).await?;
            }
            Err(e) => {
                let detail = McpSyncDetail {
                    tool: toolKey.clone(),
                    status: "error".to_string(),
                    synced_at: Some(now_ms()),
                    error_message: Some(e.clone()),
                };
                admission.ensure_current()?;
                mcp_store::update_sync_detail(&state, &serverId, &detail).await?;
                return Err(e);
            }
        }
    } else {
        // Remove from tool config (or write as disabled for opencode)
        if toolKey == "opencode" {
            let prefs = mcp_store::get_mcp_preferences(&state)
                .await
                .unwrap_or_default();
            if prefs.sync_disabled_to_opencode {
                // Write with enabled=false instead of removing
                let _ = sync_server_to_tool_with_enabled_async(&db, &server, &tool, false).await;
            } else {
                let _ = remove_server_from_tool_async(&db, &server.name, &tool).await;
            }
        } else {
            let _ = remove_server_from_tool_async(&db, &server.name, &tool).await;
        }
        admission.ensure_current()?;
        mcp_store::delete_sync_detail(&state, &serverId, &toolKey).await?;
    }

    // Emit config-changed and mcp-changed events
    let _ = app.emit("config-changed", "window");
    let _ = app.emit("mcp-changed", "window");

    Ok(is_enabled)
}

/// Set the management enabled/disabled state for an MCP server.
///
/// Disable: removes the server from every currently enabled tool config (best-effort, matching
/// delete cleanup semantics), records the current bindings into `disabled_previous_tools`
/// (keeping existing history when there are no current bindings), clears
/// `enabled_tools`/`sync_details`, sets `management_enabled = false`, and emits
/// `config-changed` + `mcp-changed`. The server stays in its group; `user_group`/`user_note`/
/// `tags` and the server config are preserved.
///
/// Enable: only flips `management_enabled = true` and returns the recorded
/// `disabled_previous_tools` so the frontend can confirm which historical tools to restore
/// through `mcp_sync_to_tool` (which emits its own events). No events are emitted here on
/// re-enable — the restore confirmation step must not trigger an early WSL projection.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn mcp_set_management_enabled<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SqliteDbState>,
    serverId: String,
    enabled: bool,
) -> Result<Vec<String>, String> {
    let server = mcp_store::get_mcp_server_by_id(&state, &serverId)
        .await?
        .ok_or_else(|| format!("MCP server not found: {}", serverId))?;
    let admission = admit_mcp_operation(&state, server_admission_tools(&server), !enabled).await?;
    admission.ensure_current()?;
    if enabled {
        // Re-enable only flips the flag; the previous tools are returned so the frontend
        // can restore them through the normal sync flow.
        return mcp_store::set_server_management_enabled(&state, &serverId, true).await;
    }

    // Disable: first remove the server from every currently enabled tool config
    // (best-effort, mirrors mcp_delete_server cleanup), then record history and clear
    // bindings in the DB. DB desired state wins even if a target removal fails.
    let custom_tools = custom_store::get_custom_tools(&state)
        .await
        .unwrap_or_default();
    let db = state.db();
    for tool_key in &server.enabled_tools {
        admission.ensure_current()?;
        let Some(tool) = runtime_tool_by_key(tool_key, &custom_tools) else {
            continue;
        };
        let _ = remove_server_from_tool_async(&db, &server.name, &tool).await;
    }
    maybe_remove_disabled_from_opencode(&state, &server, &custom_tools).await;

    admission.ensure_current()?;
    let previous_tools = mcp_store::set_server_management_enabled(&state, &serverId, false).await?;
    let _ = app.emit("config-changed", "window");
    let _ = app.emit("mcp-changed", "window");

    Ok(previous_tools)
}

/// Restore user-confirmed tool bindings for a re-enabled MCP server.
///
/// Re-enable only flips `management_enabled` and leaves `enabled_tools` empty, so the
/// generic `mcp_sync_to_tool` batch entry would skip the server (`enabled_tools.contains`
/// check). This command writes the confirmed tool subset back into `enabled_tools`, then
/// syncs each tool to its runtime config and records per-tool sync details (same shape as
/// `mcp_sync_to_tool`). Rejects servers that are not management-enabled: the only way back
/// from a disabled server is re-enable + this explicit restore flow. Emits
/// `config-changed` + `mcp-changed` exactly once so tray and WSL auto-sync follow.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn mcp_restore_tools<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SqliteDbState>,
    serverId: String,
    tools: Vec<String>,
) -> Result<Vec<McpSyncResultDto>, String> {
    let server = mcp_store::get_mcp_server_by_id(&state, &serverId)
        .await?
        .ok_or_else(|| format!("MCP server not found: {}", serverId))?;
    if !server.management_enabled {
        return Err(format!("MCP_DISABLED|{}", serverId));
    }

    let mut admission_tools = server_admission_tools(&server);
    admission_tools.extend(tools.iter().cloned());
    let admission = admit_mcp_operation(&state, admission_tools, false).await?;
    admission.ensure_current()?;

    // Persist the user-confirmed (deduplicated) subset as the new desired bindings before
    // syncing — the batch `mcp_sync_to_tool` entry filters on `enabled_tools`.
    mcp_store::set_server_enabled_tools(&state, &serverId, tools).await?;
    let server = mcp_store::get_mcp_server_by_id(&state, &serverId)
        .await?
        .ok_or_else(|| format!("MCP server not found: {}", serverId))?;

    let custom_tools = custom_store::get_custom_tools(&state)
        .await
        .unwrap_or_default();
    let db = state.db();
    let mut results = Vec::new();
    for tool_key in &server.enabled_tools {
        admission.ensure_current()?;
        let Some(tool) = runtime_tool_by_key(tool_key, &custom_tools) else {
            let detail = McpSyncDetail {
                tool: tool_key.clone(),
                status: "error".to_string(),
                synced_at: Some(now_ms()),
                error_message: Some(format!("Tool not found: {}", tool_key)),
            };
            admission.ensure_current()?;
            let _ = mcp_store::update_sync_detail(&state, &serverId, &detail).await;
            results.push(McpSyncResultDto {
                tool: tool_key.clone(),
                success: false,
                error_message: Some(format!("Tool not found: {}", tool_key)),
            });
            continue;
        };
        match sync_server_to_tool_async(&db, &server, &tool).await {
            Ok(detail) => {
                admission.ensure_current()?;
                let _ = mcp_store::update_sync_detail(&state, &serverId, &detail).await;
                results.push(McpSyncResultDto {
                    tool: tool_key.clone(),
                    success: true,
                    error_message: None,
                });
            }
            Err(e) => {
                let detail = McpSyncDetail {
                    tool: tool_key.clone(),
                    status: "error".to_string(),
                    synced_at: Some(now_ms()),
                    error_message: Some(e.clone()),
                };
                admission.ensure_current()?;
                let _ = mcp_store::update_sync_detail(&state, &serverId, &detail).await;
                results.push(McpSyncResultDto {
                    tool: tool_key.clone(),
                    success: false,
                    error_message: Some(e),
                });
            }
        }
    }

    let _ = app.emit("config-changed", "window");
    let _ = app.emit("mcp-changed", "window");

    Ok(results)
}

/// Reorder MCP servers
#[tauri::command]
pub async fn mcp_reorder_servers(
    state: State<'_, SqliteDbState>,
    ids: Vec<String>,
) -> Result<(), String> {
    mcp_store::reorder_mcp_servers(&state, &ids).await
}

/// Update MCP server user-managed metadata only.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn mcp_update_metadata(
    state: State<'_, SqliteDbState>,
    serverId: String,
    userGroup: Option<String>,
    userNote: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<(), String> {
    let normalized_tags = tags.map(|tags| {
        let mut seen = std::collections::HashSet::new();
        tags.into_iter()
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty() && seen.insert(tag.clone()))
            .collect()
    });
    mcp_store::update_mcp_server_metadata(
        &state,
        &serverId,
        normalize_optional_text(userGroup),
        normalize_optional_text(userNote),
        normalized_tags,
    )
    .await
}

// ==================== Group Inventory (export / import) ====================

/// Planned group assignment for one server during an inventory import. Note and
/// tags are snapshotted because `update_mcp_server_metadata` treats `None` as
/// "clear the field" — they must be passed back verbatim.
struct GroupInventoryAssignment {
    server_id: String,
    target_group: String,
    user_note: Option<String>,
    tags: Vec<String>,
}

/// Parsed + validated inventory ready for preview or apply.
struct GroupInventoryPlan {
    assignments: Vec<GroupInventoryAssignment>,
    group_count: usize,
    matched_server_count: usize,
    changed_count: usize,
    errors: Vec<String>,
}

impl GroupInventoryPlan {
    fn to_dto(&self) -> McpGroupInventoryPreviewDto {
        McpGroupInventoryPreviewDto {
            valid: self.errors.is_empty(),
            group_count: self.group_count,
            matched_server_count: self.matched_server_count,
            changed_count: self.changed_count,
            errors: self.errors.clone(),
        }
    }
}

/// Parse `{ "schema_version": 1, "groups": { "<group>": ["server", ...] } }`
/// into ordered (group, server names) entries.
fn parse_group_inventory_entries(content: &str) -> Result<Vec<(String, Vec<String>)>, String> {
    let value: serde_json::Value = serde_json::from_str(content)
        .map_err(|error| format!("Invalid inventory JSON: {}", error))?;
    let groups = value
        .get("groups")
        .ok_or_else(|| "Missing 'groups' object".to_string())?
        .as_object()
        .ok_or_else(|| "'groups' must be an object".to_string())?;

    let mut entries = Vec::new();
    for (group_name, members) in groups {
        let trimmed_group = group_name.trim();
        if trimmed_group.is_empty() {
            continue;
        }
        let list = members
            .as_array()
            .ok_or_else(|| format!("Group '{}' must be an array of server names", trimmed_group))?;
        let mut names = Vec::new();
        for member in list {
            let name = member
                .as_str()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .ok_or_else(|| format!("Group '{}' contains a non-string entry", trimmed_group))?;
            names.push(name.to_string());
        }
        entries.push((trimmed_group.to_string(), names));
    }
    Ok(entries)
}

async fn load_group_inventory_plan(
    state: &State<'_, SqliteDbState>,
    path: &str,
) -> Result<GroupInventoryPlan, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("Failed to read import file: {}", error))?;
    let entries = parse_group_inventory_entries(&content)?;
    let servers = mcp_store::get_mcp_servers(state).await?;

    let mut by_name: HashMap<&str, &McpServer> = HashMap::new();
    for server in &servers {
        by_name.insert(server.name.as_str(), server);
    }

    let mut plan = GroupInventoryPlan {
        assignments: Vec::new(),
        group_count: entries.len(),
        matched_server_count: 0,
        changed_count: 0,
        errors: Vec::new(),
    };

    for (target_group, names) in &entries {
        for name in names {
            let Some(server) = by_name.get(name.as_str()) else {
                plan.errors.push(format!("Server not found: {}", name));
                continue;
            };
            plan.matched_server_count += 1;
            let current_group = server.user_group.as_deref().unwrap_or("").trim();
            if current_group != target_group.as_str() {
                plan.changed_count += 1;
                plan.assignments.push(GroupInventoryAssignment {
                    server_id: server.id.clone(),
                    target_group: target_group.clone(),
                    user_note: server.user_note.clone(),
                    tags: server.tags.clone(),
                });
            }
        }
    }

    Ok(plan)
}

/// Export every managed server's group assignment as a JSON inventory file so it
/// can be curated manually (or by an AI assistant) and imported back.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn mcp_export_group_inventory(
    state: State<'_, SqliteDbState>,
    path: String,
) -> Result<String, String> {
    let export_path = path.trim().to_string();
    if export_path.is_empty() {
        return Err("Export path is empty".to_string());
    }

    let servers = mcp_store::get_mcp_servers(&state).await?;
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for server in &servers {
        let Some(group) = normalize_optional_text(server.user_group.clone()) else {
            continue;
        };
        groups.entry(group).or_default().push(server.name.clone());
    }

    let payload = serde_json::json!({ "schema_version": 1, "groups": groups });
    let pretty = serde_json::to_string_pretty(&payload).map_err(|error| error.to_string())?;
    std::fs::write(&export_path, format!("{}\n", pretty))
        .map_err(|error| format!("Failed to write export file: {}", error))?;
    Ok(export_path)
}

/// Validate a group-inventory JSON and report what would change, without writing.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn mcp_preview_group_inventory_import(
    state: State<'_, SqliteDbState>,
    path: String,
) -> Result<McpGroupInventoryPreviewDto, String> {
    let plan = load_group_inventory_plan(&state, &path).await?;
    Ok(plan.to_dto())
}

/// Apply a validated group-inventory JSON: reassign each matched server's group
/// (note and tags are preserved untouched) and emit refresh events once.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn mcp_apply_group_inventory_import<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SqliteDbState>,
    path: String,
) -> Result<McpGroupInventoryPreviewDto, String> {
    let plan = load_group_inventory_plan(&state, &path).await?;
    if !plan.errors.is_empty() {
        return Ok(plan.to_dto());
    }

    for assignment in &plan.assignments {
        mcp_store::update_mcp_server_metadata(
            &state,
            &assignment.server_id,
            Some(assignment.target_group.clone()),
            assignment.user_note.clone(),
            Some(assignment.tags.clone()),
        )
        .await?;
    }
    let _ = app.emit("config-changed", "window");
    let _ = app.emit("mcp-changed", "window");

    Ok(plan.to_dto())
}

// ==================== Managed Groups ====================

/// List managed MCP groups for the group management modal.
#[tauri::command]
pub async fn mcp_list_groups(state: State<'_, SqliteDbState>) -> Result<Vec<McpGroup>, String> {
    mcp_store::get_mcp_groups(&state).await
}

/// Create or update a managed group. Renaming an existing group also moves
/// every server whose `user_group` matched the old name to the new name so the
/// name-based membership follows the entity.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn mcp_save_group<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SqliteDbState>,
    name: String,
    note: Option<String>,
    sortIndex: i32,
    groupId: Option<String>,
) -> Result<McpGroup, String> {
    let group_name =
        normalize_optional_text(Some(name)).ok_or_else(|| "Group name is empty".to_string())?;
    let group_note = normalize_optional_text(note);

    let existing_groups = mcp_store::get_mcp_groups(&state).await?;
    if let Some(duplicate) = existing_groups
        .iter()
        .find(|group| group.name == group_name && Some(group.id.as_str()) != groupId.as_deref())
    {
        return Err(format!("Group name already exists: {}", duplicate.name));
    }

    let now = now_ms();
    let (mut group, renamed_from) = match groupId.as_deref() {
        Some(id) => {
            let current = existing_groups
                .iter()
                .find(|group| group.id == id)
                .ok_or_else(|| format!("Group not found: {}", id))?;
            let renamed_from = if current.name != group_name {
                Some(current.name.clone())
            } else {
                None
            };
            (current.clone(), renamed_from)
        }
        None => (
            McpGroup {
                id: String::new(),
                name: group_name.clone(),
                note: None,
                sort_index: sortIndex,
                created_at: now,
                updated_at: now,
            },
            None,
        ),
    };
    group.name = group_name;
    group.note = group_note;
    group.sort_index = sortIndex;
    group.updated_at = now;

    let saved_id = mcp_store::upsert_mcp_group(&state, &group).await?;

    // Keep name-based membership glued across renames. Comparison trims both
    // sides because grouping elsewhere treats user_group as normalized text.
    if renamed_from.is_some() {
        let old_name = renamed_from.as_deref().unwrap_or("");
        let servers = mcp_store::get_mcp_servers(&state).await?;
        for server in servers {
            let matches_old = server.user_group.as_deref().map(str::trim) == Some(old_name);
            if !matches_old {
                continue;
            }
            mcp_store::update_mcp_server_metadata(
                &state,
                &server.id,
                Some(group.name.clone()),
                server.user_note.clone(),
                Some(server.tags.clone()),
            )
            .await?;
        }
        let _ = app.emit("config-changed", "window");
        let _ = app.emit("mcp-changed", "window");
    }

    group.id = saved_id;
    Ok(group)
}

/// Delete a managed group. Servers keep their own config but fall back to
/// ungrouped when their `user_group` matched the deleted name.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn mcp_delete_group<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SqliteDbState>,
    groupId: String,
) -> Result<(), String> {
    let groups = mcp_store::get_mcp_groups(&state).await?;
    let group = groups
        .iter()
        .find(|group| group.id == groupId)
        .ok_or_else(|| format!("Group not found: {}", groupId))?
        .clone();
    mcp_store::delete_mcp_group(&state, &groupId).await?;

    let servers = mcp_store::get_mcp_servers(&state).await?;
    let mut changed = false;
    for server in servers {
        if server.user_group.as_deref().map(str::trim) != Some(group.name.as_str()) {
            continue;
        }
        mcp_store::update_mcp_server_metadata(
            &state,
            &server.id,
            None,
            server.user_note.clone(),
            Some(server.tags.clone()),
        )
        .await?;
        changed = true;
    }
    if changed {
        let _ = app.emit("config-changed", "window");
        let _ = app.emit("mcp-changed", "window");
    }
    Ok(())
}

// ==================== Sync Operations ====================

/// Sync all enabled servers to a specific tool
#[tauri::command]
#[allow(non_snake_case)]
pub async fn mcp_sync_to_tool<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SqliteDbState>,
    toolKey: String,
) -> Result<Vec<McpSyncResultDto>, String> {
    let admission = admit_mcp_operation(&state, vec![toolKey.clone()], false).await?;
    admission.ensure_current()?;
    let custom_tools = custom_store::get_custom_tools(&state)
        .await
        .unwrap_or_default();
    let tool = runtime_tool_by_key(&toolKey, &custom_tools)
        .ok_or_else(|| format!("Tool not found: {}", toolKey))?;

    let db = state.db();
    if !is_tool_installed_with_db_async(&db, &tool).await {
        return Err(format!("Tool {} is not installed", toolKey));
    }

    let servers = mcp_store::get_mcp_servers(&state).await?;
    let mut results = Vec::new();

    for server in servers {
        admission.ensure_current()?;
        // Management-disabled servers must never be re-synced by the batch sync entry.
        if !server.management_enabled {
            continue;
        }
        if !server.enabled_tools.contains(&toolKey) {
            continue;
        }

        match sync_server_to_tool_async(&db, &server, &tool).await {
            Ok(detail) => {
                admission.ensure_current()?;
                mcp_store::update_sync_detail(&state, &server.id, &detail).await?;
                results.push(McpSyncResultDto {
                    tool: toolKey.clone(),
                    success: true,
                    error_message: None,
                });
            }
            Err(e) => {
                let detail = McpSyncDetail {
                    tool: toolKey.clone(),
                    status: "error".to_string(),
                    synced_at: Some(now_ms()),
                    error_message: Some(e.clone()),
                };
                admission.ensure_current()?;
                mcp_store::update_sync_detail(&state, &server.id, &detail).await?;
                results.push(McpSyncResultDto {
                    tool: toolKey.clone(),
                    success: false,
                    error_message: Some(e),
                });
            }
        }
    }

    // Emit config-changed and mcp-changed events
    let _ = app.emit("config-changed", "window");
    let _ = app.emit("mcp-changed", "window");

    Ok(results)
}

/// Sync all servers to all enabled tools
#[tauri::command]
pub async fn mcp_sync_all<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SqliteDbState>,
) -> Result<Vec<McpSyncResultDto>, String> {
    mcp_sync_all_internal(app, state.inner(), true).await
}

/// Restore-only MCP projection that avoids starting event-driven WSL sync midway through the
/// serial post-restore recovery pipeline.
pub async fn mcp_sync_all_without_events<R: Runtime>(
    app: AppHandle<R>,
    state: &SqliteDbState,
) -> Result<Vec<McpSyncResultDto>, String> {
    mcp_sync_all_internal(app, state, false).await
}

async fn mcp_sync_all_internal<R: Runtime>(
    app: AppHandle<R>,
    state: &SqliteDbState,
    emit_events: bool,
) -> Result<Vec<McpSyncResultDto>, String> {
    let custom_tools = custom_store::get_custom_tools(state)
        .await
        .unwrap_or_default();
    let db = state.db();
    let servers = mcp_store::get_mcp_servers(state).await?;
    let keys = servers.iter().flat_map(server_admission_tools).collect();
    let admission = admit_mcp_operation(state, keys, true).await?;
    admission.ensure_current()?;
    let mut results = Vec::new();

    for server in servers {
        admission.ensure_current()?;
        // Management-disabled servers must never be re-synced by the full sync entry.
        if !server.management_enabled {
            continue;
        }
        for tool_key in &server.enabled_tools {
            let Some(tool) = runtime_tool_by_key(tool_key, &custom_tools) else {
                continue;
            };

            if !is_tool_installed_with_db_async(&db, &tool).await {
                continue;
            }

            match sync_server_to_tool_async(&db, &server, &tool).await {
                Ok(detail) => {
                    admission.ensure_current()?;
                    mcp_store::update_sync_detail(state, &server.id, &detail).await?;
                    results.push(McpSyncResultDto {
                        tool: tool_key.clone(),
                        success: true,
                        error_message: None,
                    });
                }
                Err(e) => {
                    let detail = McpSyncDetail {
                        tool: tool_key.clone(),
                        status: "error".to_string(),
                        synced_at: Some(now_ms()),
                        error_message: Some(e.clone()),
                    };
                    admission.ensure_current()?;
                    mcp_store::update_sync_detail(state, &server.id, &detail).await?;
                    results.push(McpSyncResultDto {
                        tool: tool_key.clone(),
                        success: false,
                        error_message: Some(e),
                    });
                }
            }
        }
    }

    // Also sync disabled servers to opencode if switch is ON
    let prefs = mcp_store::get_mcp_preferences(state)
        .await
        .unwrap_or_default();
    if prefs.sync_disabled_to_opencode {
        let all_servers = mcp_store::get_mcp_servers(state).await.unwrap_or_default();
        sync_opencode_disabled(&db, &all_servers, &custom_tools).await;
    }

    if emit_events {
        let _ = app.emit("config-changed", "window");
        let _ = app.emit("mcp-changed", "window");
    }

    Ok(results)
}

/// Import MCP servers from a tool's config file
/// After import, automatically sync to specified tools (or preferred tools if not specified)
/// If a server with the same name exists but has different config, create with suffix
///
/// For the CC Switch source with `followCcSwitchMarks` (default true), each server's
/// sync targets come from the CCS `enabled_*` markers instead of the shared
/// `enabledTools` selection. Servers with no marker are imported without any tool
/// enabled.
#[tauri::command]
#[allow(non_snake_case)]
pub async fn mcp_import_from_tool<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SqliteDbState>,
    toolKey: String,
    enabledTools: Option<Vec<String>>,
    followCcSwitchMarks: Option<bool>,
) -> Result<McpImportResultDto, String> {
    let custom_tools = custom_store::get_custom_tools(&state)
        .await
        .unwrap_or_default();

    let is_cc_switch_source = toolKey == crate::coding::cc_switch::CC_SWITCH_MCP_TOOL_KEY;
    let follow_cc_switch_marks = is_cc_switch_source && followCcSwitchMarks.unwrap_or(true);

    // Resolve imported servers: CC Switch DB / plugin .mcp.json / standard tool config
    let (imported_servers, source_display_name) = if is_cc_switch_source {
        let candidates = crate::coding::cc_switch::list_cc_switch_mcp_servers(None)?;
        let now = now_ms();
        let servers = candidates
            .into_iter()
            .map(|c| McpServer {
                id: String::new(),
                name: c.name,
                server_type: c.server_type,
                server_config: c.server_config,
                enabled_tools: c.enabled_tools,
                sync_details: None,
                description: c.description,
                user_group: None,
                user_note: None,
                tags: c.tags,
                timeout: None,
                sort_index: 0,
                management_enabled: true,
                disabled_previous_tools: Vec::new(),
                created_at: now,
                updated_at: now,
            })
            .collect::<Vec<_>>();
        (
            servers,
            crate::coding::cc_switch::CC_SWITCH_MCP_TOOL_NAME.to_string(),
        )
    } else if let Some(plugin_id) = toolKey.strip_prefix("plugin::") {
        // Plugin source: find the plugin and read its .mcp.json
        let plugins =
            crate::coding::tools::claude_plugins::get_installed_plugins(&state.db()).await;
        let plugin = plugins
            .iter()
            .find(|p| p.plugin_id == plugin_id)
            .ok_or_else(|| format!("Plugin not found: {}", plugin_id))?;
        let mcp_json_path = plugin.install_path.join(".mcp.json");
        let servers = import_servers_from_plugin_mcp_json(&mcp_json_path)?;
        (servers, format!("Plugin: {}", plugin.display_name))
    } else {
        // Standard tool source
        let tool = runtime_tool_by_key(&toolKey, &custom_tools)
            .ok_or_else(|| format!("Tool not found: {}", toolKey))?;
        let servers = import_servers_from_tool_async(&state.db(), &tool).await?;
        (
            servers,
            super::mcp_tool_display_name(&tool.key, &tool.display_name),
        )
    };

    // Keep requested bindings before the ordinary installed-tool filter: a
    // selected Codex/unknown target must not silently become a partial managed
    // import merely because its runtime directory is currently absent.
    let mut requested_target_tools = Vec::new();
    // Get target tools for sync: use enabledTools if provided, otherwise use preferred tools or all installed MCP tools
    let target_tools: Vec<String> = if let Some(enabled) = enabledTools {
        requested_target_tools.extend(enabled.iter().cloned());
        // Use provided enabled tools, but only those that are installed
        {
            let mut installed_tool_keys = Vec::new();
            for key in enabled {
                let Some(tool) = runtime_tool_by_key(&key, &custom_tools) else {
                    continue;
                };
                if is_tool_installed_with_db_async(&state.db(), &tool).await {
                    installed_tool_keys.push(key);
                }
            }
            installed_tool_keys
        }
    } else {
        // Fall back to preferred tools or all installed MCP tools
        let prefs = mcp_store::get_mcp_preferences(&state).await?;
        if !prefs.preferred_tools.is_empty() {
            requested_target_tools.extend(prefs.preferred_tools.iter().cloned());
            // Use preferred tools, but only those that are installed
            {
                let mut installed_tool_keys = Vec::new();
                for key in prefs.preferred_tools {
                    let Some(tool) = runtime_tool_by_key(&key, &custom_tools) else {
                        continue;
                    };
                    if is_tool_installed_with_db_async(&state.db(), &tool).await {
                        installed_tool_keys.push(key);
                    }
                }
                installed_tool_keys
            }
        } else {
            // Use all installed MCP tools
            {
                let mut installed_tool_keys = Vec::new();
                for tool in get_mcp_runtime_tools(&custom_tools) {
                    if is_tool_installed_with_db_async(&state.db(), &tool).await {
                        installed_tool_keys.push(tool.key);
                    }
                }
                installed_tool_keys
            }
        }
    };

    let mut servers_imported = 0;
    let mut servers_skipped = 0;
    let mut servers_duplicated = Vec::new();
    let mut errors = Vec::new();

    // Admit the whole import before the first upsert; a later Codex item may
    // not leave earlier Claude rows/files partially imported.
    let import_keys = if follow_cc_switch_marks {
        imported_servers.iter().flat_map(server_admission_tools).collect()
    } else {
        requested_target_tools.extend(target_tools.iter().cloned());
        requested_target_tools
    };
    let admission = admit_mcp_operation(&state, import_keys, false).await?;
    admission.ensure_current()?;

    for mut server in imported_servers {
        admission.ensure_current()?;
        // Check if server with same name already exists
        if let Some(existing) = mcp_store::get_mcp_server_by_name(&state, &server.name).await? {
            // Compare configurations
            if existing.server_type == server.server_type
                && existing.server_config == server.server_config
            {
                // Same config, skip
                servers_skipped += 1;
                continue;
            } else {
                // Different config, create with suffix
                let new_name = format!("{} ({})", server.name, source_display_name);
                servers_duplicated.push(new_name.clone());
                server.name = new_name;
            }
        }

        // Resolve sync targets for this server: CC Switch per-server markers
        // (kept only for installed tools), or the shared selection otherwise.
        if follow_cc_switch_marks {
            let mut installed_marks = Vec::new();
            for tool_key in &server.enabled_tools {
                let Some(tool) = runtime_tool_by_key(tool_key, &custom_tools) else {
                    continue;
                };
                if is_tool_installed_with_db_async(&state.db(), &tool).await {
                    installed_marks.push(tool_key.clone());
                }
            }
            server.enabled_tools = installed_marks;
        } else {
            // Enable the target tools
            server.enabled_tools = target_tools.clone();
        }

        admission.ensure_current()?;
        match mcp_store::upsert_mcp_server(&state, &server).await {
            Ok(server_id) => {
                servers_imported += 1;

                // Sync to each enabled tool
                for tool_key in &server.enabled_tools {
                    if let Some(target_tool) = runtime_tool_by_key(tool_key, &custom_tools) {
                        match sync_server_to_tool_async(&state.db(), &server, &target_tool).await {
                            Ok(detail) => {
                                admission.ensure_current()?;
                                let _ = mcp_store::update_sync_detail(&state, &server_id, &detail)
                                    .await;
                            }
                            Err(e) => {
                                let detail = McpSyncDetail {
                                    tool: tool_key.clone(),
                                    status: "error".to_string(),
                                    synced_at: Some(now_ms()),
                                    error_message: Some(e.clone()),
                                };
                                admission.ensure_current()?;
                                let _ = mcp_store::update_sync_detail(&state, &server_id, &detail)
                                    .await;
                                errors
                                    .push(format!("Sync '{}' to {}: {}", server.name, tool_key, e));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                errors.push(format!("Failed to import '{}': {}", server.name, e));
            }
        }
    }

    // Emit events so the tray refreshes and WSL auto-sync picks up the
    // imported servers (same contract as create/update/delete).
    let _ = app.emit("config-changed", "window");
    let _ = app.emit("mcp-changed", "window");

    Ok(McpImportResultDto {
        servers_imported,
        servers_skipped,
        servers_duplicated,
        errors,
    })
}

// ==================== Tools API ====================

/// Get all tools that support MCP
#[tauri::command]
pub async fn mcp_get_tools(state: State<'_, SqliteDbState>) -> Result<Vec<RuntimeToolDto>, String> {
    let custom_tools = custom_store::get_custom_tools(&state)
        .await
        .unwrap_or_default();
    let mcp_tools = get_mcp_runtime_tools(&custom_tools);
    let db = state.db();

    let mut tool_dtos = Vec::with_capacity(mcp_tools.len());
    for tool in &mcp_tools {
        let mut tool_dto = to_runtime_tool_dto_with_db_async(&db, tool).await;
        tool_dto.display_name = super::mcp_tool_display_name(&tool.key, &tool_dto.display_name);
        tool_dtos.push(tool_dto);
    }

    Ok(tool_dtos)
}

/// Scan all installed MCP tools and return discovered servers (excluding already imported ones)
#[tauri::command]
pub async fn mcp_scan_servers(state: State<'_, SqliteDbState>) -> Result<McpScanResultDto, String> {
    // Add 30 second timeout to prevent hanging
    match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        mcp_scan_servers_inner(&state),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            Err("Scan timed out after 30 seconds. Please check your custom tool paths.".to_string())
        }
    }
}

async fn mcp_scan_servers_inner(state: &SqliteDbState) -> Result<McpScanResultDto, String> {
    let custom_tools = custom_store::get_custom_tools(state)
        .await
        .unwrap_or_default();
    let mcp_tools = get_mcp_runtime_tools(&custom_tools);
    let scan_db = state.db();

    // Get existing server names for filtering
    let existing_servers = mcp_store::get_mcp_servers(state).await?;
    let existing_names: std::collections::HashSet<String> =
        existing_servers.iter().map(|s| s.name.clone()).collect();
    let claude_plugins =
        crate::coding::tools::claude_plugins::get_installed_plugins(&scan_db).await;

    let mut scan_targets = Vec::new();
    for tool in &mcp_tools {
        if !is_tool_installed_with_db_async(&scan_db, tool).await {
            continue;
        }

        let Some(config_path) = resolve_mcp_config_path_with_db_async(&scan_db, tool).await else {
            continue;
        };

        if !config_path.exists() {
            continue;
        }

        scan_targets.push((tool.clone(), config_path));
    }

    // Run the blocking file system operations in a dedicated thread pool
    // to avoid blocking the tokio async runtime
    let scan_result = tokio::task::spawn_blocking(move || {
        let mut total_tools_scanned = 0;
        let mut servers: Vec<McpDiscoveredServerDto> = Vec::new();

        for (tool, config_path) in &scan_targets {
            eprintln!("[DEBUG][mcp_scan_servers] scanning tool: {}", tool.key);
            total_tools_scanned += 1;

            // Try to import servers from this tool
            match import_servers_from_path(tool, config_path) {
                Ok(imported) => {
                    eprintln!(
                        "[DEBUG][mcp_scan_servers] {} imported {} servers",
                        tool.key,
                        imported.len()
                    );
                    for server in imported {
                        // Skip servers that already exist in the database
                        if existing_names.contains(&server.name) {
                            continue;
                        }
                        servers.push(McpDiscoveredServerDto {
                            name: server.name,
                            tool_key: tool.key.clone(),
                            tool_name: super::mcp_tool_display_name(&tool.key, &tool.display_name),
                            server_type: server.server_type,
                            server_config: server.server_config,
                            source_enabled_tools: Vec::new(),
                        });
                    }
                }
                Err(e) => {
                    // Log error but continue scanning
                    eprintln!("Failed to scan {}: {}", tool.key, e);
                }
            }
        }

        // Scan Claude Code plugins for MCP servers
        for plugin in &claude_plugins {
            let mcp_json_path = plugin.install_path.join(".mcp.json");
            if !mcp_json_path.exists() {
                continue;
            }

            let tool_key = format!("plugin::{}", plugin.plugin_id);
            let tool_name = format!("Plugin: {}", plugin.display_name);
            total_tools_scanned += 1;

            match import_servers_from_plugin_mcp_json(&mcp_json_path) {
                Ok(imported) => {
                    for server in imported {
                        if existing_names.contains(&server.name) {
                            continue;
                        }
                        servers.push(McpDiscoveredServerDto {
                            name: server.name,
                            tool_key: tool_key.clone(),
                            tool_name: tool_name.clone(),
                            server_type: server.server_type,
                            server_config: server.server_config,
                            source_enabled_tools: Vec::new(),
                        });
                    }
                }
                Err(e) => {
                    eprintln!("Failed to scan plugin {}: {}", plugin.plugin_id, e);
                }
            }
        }

        // Scan CC Switch central mcp_servers table (no separate import button).
        // Only count as a scanned source when at least one non-existing server is listed.
        match crate::coding::cc_switch::list_cc_switch_mcp_servers(None) {
            Ok(candidates) if !candidates.is_empty() => {
                let tool_key = crate::coding::cc_switch::CC_SWITCH_MCP_TOOL_KEY.to_string();
                let tool_name = crate::coding::cc_switch::CC_SWITCH_MCP_TOOL_NAME.to_string();
                let before = servers.len();
                for c in candidates {
                    if existing_names.contains(&c.name) {
                        continue;
                    }
                    servers.push(McpDiscoveredServerDto {
                        name: c.name,
                        tool_key: tool_key.clone(),
                        tool_name: tool_name.clone(),
                        server_type: c.server_type,
                        server_config: c.server_config,
                        source_enabled_tools: c.enabled_tools,
                    });
                }
                if servers.len() > before {
                    total_tools_scanned += 1;
                }
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Failed to scan CC Switch MCP: {}", e);
            }
        }

        McpScanResultDto {
            total_tools_scanned,
            total_servers_found: servers.len() as i32,
            servers,
        }
    })
    .await
    .map_err(|e| format!("spawn_blocking failed: {}", e))?;

    Ok(scan_result)
}

// ==================== Preferences ====================

/// Get MCP show in tray setting
#[tauri::command]
pub async fn mcp_get_show_in_tray(state: State<'_, SqliteDbState>) -> Result<bool, String> {
    let prefs = mcp_store::get_mcp_preferences(&state).await?;
    Ok(prefs.show_in_tray)
}

/// Set MCP show in tray setting
#[tauri::command]
pub async fn mcp_set_show_in_tray(
    state: State<'_, SqliteDbState>,
    enabled: bool,
) -> Result<(), String> {
    let mut prefs = mcp_store::get_mcp_preferences(&state).await?;
    prefs.show_in_tray = enabled;
    prefs.updated_at = now_ms();
    mcp_store::save_mcp_preferences(&state, &prefs).await
}

/// Get MCP preferred tools
#[tauri::command]
pub async fn mcp_get_preferred_tools(
    state: State<'_, SqliteDbState>,
) -> Result<Vec<String>, String> {
    let prefs = mcp_store::get_mcp_preferences(&state).await?;
    Ok(prefs.preferred_tools)
}

/// Set MCP preferred tools
#[tauri::command]
pub async fn mcp_set_preferred_tools(
    state: State<'_, SqliteDbState>,
    tools: Vec<String>,
) -> Result<(), String> {
    let mut prefs = mcp_store::get_mcp_preferences(&state).await?;
    prefs.preferred_tools = tools;
    prefs.updated_at = now_ms();
    mcp_store::save_mcp_preferences(&state, &prefs).await
}

/// Get whether MCP card add-more menus are limited to preferred tools.
#[tauri::command]
pub async fn mcp_get_limit_add_more_to_preferred_tools(
    state: State<'_, SqliteDbState>,
) -> Result<bool, String> {
    let prefs = mcp_store::get_mcp_preferences(&state).await?;
    Ok(prefs.limit_add_more_to_preferred_tools)
}

/// Set whether MCP card add-more menus are limited to preferred tools.
#[tauri::command]
pub async fn mcp_set_limit_add_more_to_preferred_tools(
    state: State<'_, SqliteDbState>,
    enabled: bool,
) -> Result<(), String> {
    let mut prefs = mcp_store::get_mcp_preferences(&state).await?;
    prefs.limit_add_more_to_preferred_tools = enabled;
    prefs.updated_at = now_ms();
    mcp_store::save_mcp_preferences(&state, &prefs).await
}

/// Get sync disabled to opencode setting
#[tauri::command]
pub async fn mcp_get_sync_disabled_to_opencode(
    state: State<'_, SqliteDbState>,
) -> Result<bool, String> {
    let prefs = mcp_store::get_mcp_preferences(&state).await?;
    Ok(prefs.sync_disabled_to_opencode)
}

/// Set sync disabled to opencode setting
/// When toggled ON: sync all unlinked MCP servers to opencode with enabled=false
/// When toggled OFF: remove disabled entries from opencode config
#[tauri::command]
pub async fn mcp_set_sync_disabled_to_opencode<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SqliteDbState>,
    enabled: bool,
) -> Result<(), String> {
    let admission = admit_mcp_operation(&state, vec!["opencode".to_owned()], false).await?;
    admission.ensure_current()?;
    let mut prefs = mcp_store::get_mcp_preferences(&state).await?;
    prefs.sync_disabled_to_opencode = enabled;
    prefs.updated_at = now_ms();
    admission.ensure_current()?;
    mcp_store::save_mcp_preferences(&state, &prefs).await?;

    let servers = mcp_store::get_mcp_servers(&state).await?;
    let custom_tools = custom_store::get_custom_tools(&state)
        .await
        .unwrap_or_default();
    let db = state.db();

    if enabled {
        sync_opencode_disabled(&db, &servers, &custom_tools).await;
    } else {
        cleanup_opencode_disabled(&db, &servers, &custom_tools).await;
    }

    let _ = app.emit("config-changed", "window");
    let _ = app.emit("mcp-changed", "window");

    Ok(())
}

/// If sync_disabled_to_opencode is ON and server is not linked to opencode,
/// write it to opencode config with enabled=false.
async fn maybe_sync_disabled_to_opencode(
    state: &SqliteDbState,
    server: &McpServer,
    custom_tools: &[CustomTool],
) {
    let prefs = mcp_store::get_mcp_preferences(state)
        .await
        .unwrap_or_default();
    if !prefs.sync_disabled_to_opencode || server.enabled_tools.contains(&"opencode".to_string()) {
        return;
    }
    if let Some(tool) = runtime_tool_by_key("opencode", custom_tools) {
        let db = state.db();
        if is_tool_installed_with_db_async(&db, &tool).await {
            let _ = sync_server_to_tool_with_enabled_async(&db, server, &tool, false).await;
        }
    }
}

/// If sync_disabled_to_opencode is ON and server is not linked to opencode,
/// remove it from opencode config (used when deleting a server).
async fn maybe_remove_disabled_from_opencode(
    state: &SqliteDbState,
    server: &McpServer,
    custom_tools: &[CustomTool],
) {
    let prefs = mcp_store::get_mcp_preferences(state)
        .await
        .unwrap_or_default();
    if !prefs.sync_disabled_to_opencode || server.enabled_tools.contains(&"opencode".to_string()) {
        return;
    }
    if let Some(tool) = runtime_tool_by_key("opencode", custom_tools) {
        let db = state.db();
        let _ = remove_server_from_tool_async(&db, &server.name, &tool).await;
    }
}

/// Helper: Sync all MCP servers NOT linked to opencode as disabled (enabled=false) in opencode config
async fn sync_opencode_disabled(
    db: &crate::db::SqliteDbState,
    servers: &[McpServer],
    custom_tools: &[CustomTool],
) {
    let Some(tool) = runtime_tool_by_key("opencode", custom_tools) else {
        return;
    };
    if !is_tool_installed_with_db_async(db, &tool).await {
        return;
    }
    for server in servers {
        // Disabled servers were fully removed from tool configs; never write them back
        // as opencode disabled entries.
        if !server.management_enabled {
            continue;
        }
        if !server.enabled_tools.contains(&"opencode".to_string()) {
            let _ = sync_server_to_tool_with_enabled_async(db, server, &tool, false).await;
        }
    }
}

/// Helper: Remove all MCP servers NOT linked to opencode from opencode config
async fn cleanup_opencode_disabled(
    db: &crate::db::SqliteDbState,
    servers: &[McpServer],
    custom_tools: &[CustomTool],
) {
    let Some(tool) = runtime_tool_by_key("opencode", custom_tools) else {
        return;
    };
    for server in servers {
        if !server.enabled_tools.contains(&"opencode".to_string()) {
            let _ = remove_server_from_tool_async(db, &server.name, &tool).await;
        }
    }
}

// ==================== Custom Tool Management ====================

/// Add or update a custom tool with MCP fields (preserves existing Skills fields)
#[tauri::command]
#[allow(non_snake_case)]
pub async fn mcp_add_custom_tool(
    state: State<'_, SqliteDbState>,
    key: String,
    displayName: String,
    relativeDetectDir: Option<String>,
    mcpConfigPath: String,
    mcpConfigFormat: String,
    mcpField: String,
    iconUrl: Option<String>,
) -> Result<(), String> {
    use crate::coding::tools::path_utils::{normalize_path, to_storage_path};

    // Trim whitespace from all inputs
    let key = key.trim().to_string();
    let display_name = displayName.trim().to_string();
    let mcp_format = mcpConfigFormat.trim().to_lowercase();
    let mcp_field_name = mcpField.trim().to_string();

    // Normalize the MCP config path
    let normalized_mcp_path = normalize_path(mcpConfigPath.trim());
    let mcp_path = to_storage_path(&normalized_mcp_path);

    // Normalize the detect dir if provided
    let detect_dir = relativeDetectDir.map(|s| {
        let normalized = normalize_path(s.trim());
        to_storage_path(&normalized)
    });

    // Icon: `None` preserves the stored icon; `Some("")` clears it (documented
    // store contract); `Some(url)` sets it.
    let icon_url: Option<Option<String>> = iconUrl.map(|u| Some(u.trim().to_string()));
    if let Some(Some(ref url)) = icon_url {
        if !url.is_empty() && !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err("Icon URL must start with http:// or https://".to_string());
        }
    }

    // Validate key format
    if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("Key must contain only letters, numbers, and underscores".to_string());
    }

    // Validate mcp_format
    if mcp_format != "json" && mcp_format != "toml" && mcp_format != "jsonc" {
        return Err("MCP config format must be 'json', 'jsonc' or 'toml'".to_string());
    }

    // Check for duplicate with built-in tools
    if crate::coding::tools::builtin::builtin_tool_by_key(&key).is_some() {
        return Err(format!("Key '{}' conflicts with a built-in tool", key));
    }

    let admission = admit_custom_mcp_edit(&state, &key, Some(&mcp_path)).await?;
    admission.ensure_current()?;
    custom_store::save_custom_tool_mcp_fields(
        &state,
        &key,
        &display_name,
        detect_dir,
        Some(mcp_path),
        Some(mcp_format),
        Some(mcp_field_name),
        now_ms(),
        icon_url,
    )
    .await
}

/// Remove a custom tool (only if it has no Skills fields, otherwise just clear MCP fields)
#[tauri::command]
pub async fn mcp_remove_custom_tool(
    state: State<'_, SqliteDbState>,
    key: String,
) -> Result<(), String> {
    let admission = admit_custom_mcp_edit(&state, &key, None).await?;
    admission.ensure_current()?;
    // Get the existing tool
    let existing = custom_store::get_custom_tool_by_key(&state, &key).await?;

    if let Some(tool) = existing {
        // If tool has Skills fields, just clear MCP fields
        if tool.relative_skills_dir.is_some() {
            custom_store::save_custom_tool_mcp_fields(
                &state,
                &key,
                &tool.display_name,
                tool.relative_detect_dir.clone(),
                None,
                None,
                None,
                tool.created_at,
                None,
            )
            .await
        } else {
            // No Skills fields, delete completely
            custom_store::delete_custom_tool(&state, &key).await
        }
    } else {
        Err(format!("Custom tool '{}' not found", key))
    }
}

// ==================== Favorite MCP ====================

/// List all favorite MCPs
#[tauri::command]
pub async fn mcp_list_favorites(
    state: State<'_, SqliteDbState>,
) -> Result<Vec<FavoriteMcpDto>, String> {
    let favorites = mcp_store::get_favorite_mcps(&state).await?;

    Ok(favorites
        .into_iter()
        .map(|f| FavoriteMcpDto {
            id: f.id,
            name: f.name,
            server_type: f.server_type,
            server_config: f.server_config,
            description: f.description,
            tags: f.tags,
            is_preset: f.is_preset,
            created_at: f.created_at,
            updated_at: f.updated_at,
        })
        .collect())
}

/// Create or update a favorite MCP (upsert by name)
#[tauri::command]
pub async fn mcp_upsert_favorite(
    state: State<'_, SqliteDbState>,
    input: FavoriteMcpInput,
) -> Result<FavoriteMcpDto, String> {
    let now = now_ms();

    // Check if a favorite with the same name exists
    let existing = mcp_store::get_favorite_mcp_by_name(&state, &input.name).await?;

    let fav = if let Some(existing) = existing {
        // Update existing
        FavoriteMcp {
            id: existing.id,
            name: input.name,
            server_type: input.server_type,
            server_config: input.server_config,
            description: input.description,
            tags: input.tags,
            is_preset: false,
            created_at: existing.created_at,
            updated_at: now,
        }
    } else {
        // Create new
        FavoriteMcp {
            id: String::new(),
            name: input.name,
            server_type: input.server_type,
            server_config: input.server_config,
            description: input.description,
            tags: input.tags,
            is_preset: false,
            created_at: now,
            updated_at: now,
        }
    };

    let id = mcp_store::upsert_favorite_mcp(&state, &fav).await?;

    Ok(FavoriteMcpDto {
        id,
        name: fav.name,
        server_type: fav.server_type,
        server_config: fav.server_config,
        description: fav.description,
        tags: fav.tags,
        is_preset: fav.is_preset,
        created_at: fav.created_at,
        updated_at: fav.updated_at,
    })
}

/// Delete a favorite MCP
#[tauri::command]
#[allow(non_snake_case)]
pub async fn mcp_delete_favorite(
    state: State<'_, SqliteDbState>,
    favoriteId: String,
) -> Result<(), String> {
    mcp_store::delete_favorite_mcp(&state, &favoriteId).await
}

/// Default favorite MCP presets seeded into a user's library.
const DEFAULT_FAVORITE_MCP_PRESETS: &[(&str, &str, &str)] = &[
    (
        "mcp-server-fetch",
        "stdio",
        r#"{"command":"uvx","args":["mcp-server-fetch"]}"#,
    ),
    (
        "@modelcontextprotocol/server-time",
        "stdio",
        r#"{"command":"npx","args":["-y","@modelcontextprotocol/server-time"]}"#,
    ),
    (
        "@modelcontextprotocol/server-memory",
        "stdio",
        r#"{"command":"npx","args":["-y","@modelcontextprotocol/server-memory"]}"#,
    ),
    (
        "@modelcontextprotocol/server-sequential-thinking",
        "stdio",
        r#"{"command":"npx","args":["-y","@modelcontextprotocol/server-sequential-thinking"]}"#,
    ),
    (
        "@upstash/context7-mcp",
        "stdio",
        r#"{"command":"npx","args":["-y","@upstash/context7-mcp"]}"#,
    ),
    (
        "chrome-devtools",
        "stdio",
        r#"{"command":"npx","args":["-y","chrome-devtools-mcp@latest"]}"#,
    ),
    (
        "playwright",
        "stdio",
        r#"{"command":"npx","args":["@playwright/mcp@latest"]}"#,
    ),
];

#[tauri::command]
pub async fn mcp_init_default_favorites(state: State<'_, SqliteDbState>) -> Result<usize, String> {
    let prefs = mcp_store::get_mcp_preferences(&state).await?;
    let now = now_ms();
    let mut inserted_count = 0;

    for (name, server_type, config_json) in DEFAULT_FAVORITE_MCP_PRESETS {
        if mcp_store::get_favorite_mcp_by_name(&state, name)
            .await?
            .is_some()
        {
            continue;
        }

        let server_config: serde_json::Value = serde_json::from_str(config_json)
            .map_err(|e| format!("Invalid preset config: {}", e))?;

        let fav = FavoriteMcp {
            id: String::new(),
            name: name.to_string(),
            server_type: server_type.to_string(),
            server_config,
            description: None,
            tags: vec![],
            is_preset: true,
            created_at: now,
            updated_at: now,
        };
        mcp_store::upsert_favorite_mcp(&state, &fav).await?;
        inserted_count += 1;
    }

    let mut prefs = prefs;
    prefs.favorites_initialized = true;
    prefs.updated_at = now;
    mcp_store::save_mcp_preferences(&state, &prefs).await?;

    Ok(inserted_count)
}

#[cfg(all(test, target_os = "linux"))]
mod admission_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tauri::Manager;

    const UNSUPPORTED: &str =
        "This operation is not supported while managed Codex writes are enabled";

    fn fixture() -> (tempfile::TempDir, std::sync::Arc<GuiProfileSession>) {
        let directory = tempfile::Builder::new()
            .prefix("ai-toolbox-mcp-command-admission-")
            .tempdir_in(std::env::temp_dir())
            .unwrap();
        for child in ["profile", "codex", "claude", "source"] {
            let path = directory.path().join(child);
            std::fs::create_dir(&path).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let owner = ai_toolbox_management::owner::ProfileLeaseStartup::acquire(
            directory.path().join("profile"),
        ).unwrap().into_managed().unwrap();
        let runtime = ai_toolbox_management::owner::RuntimeResourceIdentity::existing(
            directory.path().join("codex"),
        ).unwrap();
        let session = GuiProfileSession::managed_with_runtime(owner, runtime).unwrap();
        (directory, session)
    }

    async fn custom(state: &SqliteDbState, key: &str, path: &std::path::Path) {
        custom_store::save_custom_tool(state, &CustomTool {
            key: key.to_owned(), display_name: key.to_owned(),
            relative_skills_dir: None, relative_detect_dir: None, force_copy: true,
            icon_url: None, mcp_config_path: Some(path.to_string_lossy().into_owned()),
            mcp_config_format: Some("json".to_owned()), mcp_field: Some("mcpServers".to_owned()),
            created_at: 1,
        }).await.unwrap();
    }

    fn input(tools: &[&str]) -> CreateMcpServerInput {
        CreateMcpServerInput {
            name: "fixture-server".to_owned(), server_type: "stdio".to_owned(),
            server_config: serde_json::json!({"command": "fixture-command"}),
            enabled_tools: tools.iter().map(|key| (*key).to_owned()).collect(),
            description: None, tags: Vec::new(), timeout: None,
        }
    }

    async fn snapshot(state: &SqliteDbState) -> serde_json::Value {
        serde_json::to_value(mcp_store::get_mcp_servers(state).await.unwrap()).unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn managed_create_and_custom_path_edit_reject_before_central_write() {
        let (directory, session) = fixture();
        let app = tauri::test::mock_app();
        app.manage(SqliteDbState::in_memory_for_test().unwrap());
        let state = app.state::<SqliteDbState>();
        let alias = directory.path().join("custom-alias");
        std::os::unix::fs::symlink(directory.path().join("codex"), &alias).unwrap();
        custom(&state, "custom_codex", &alias.join("mcp.json")).await;
        let before = snapshot(&state).await;
        let _scope = crate::management_admission::test_session(&session);

        for key in ["codex", "custom_codex", "unknown_tool"] {
            assert_eq!(mcp_create_server(app.handle().clone(), state.clone(), input(&[key]))
                .await.unwrap_err(), UNSUPPORTED);
            assert_eq!(snapshot(&state).await, before);
        }
        assert_eq!(mcp_add_custom_tool(
            state.clone(), "new_alias".to_owned(), "Alias".to_owned(), None,
            alias.join("mcp.json").to_string_lossy().into_owned(),
            "json".to_owned(), "mcpServers".to_owned(), None,
        ).await.unwrap_err(), UNSUPPORTED);
        assert!(custom_store::get_custom_tool_by_key(&state, "new_alias").await.unwrap().is_none());
        assert!(!directory.path().join("codex/mcp.json").exists());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn managed_crud_preflights_old_and_previous_targets_before_any_mutation() {
        let (directory, session) = fixture();
        let app = tauri::test::mock_app();
        app.manage(SqliteDbState::in_memory_for_test().unwrap());
        let state = app.state::<SqliteDbState>();
        let safe = directory.path().join("claude/mcp.json");
        custom(&state, "custom_claude", &safe).await;
        // A prior assignment is part of the admission set even after the user
        // selects a disjoint replacement; this is not just a new-key check.
        let server = McpServer {
            id: "fixture-old".to_owned(), name: "fixture-server".to_owned(),
            server_type: "stdio".to_owned(), server_config: serde_json::json!({"command":"old"}),
            enabled_tools: vec!["custom_claude".to_owned()],
            sync_details: Some(serde_json::json!({"codex":{"status":"synced"}})),
            description: None, user_group: None, user_note: None, tags: Vec::new(),
            timeout: None, sort_index: 0, management_enabled: true,
            disabled_previous_tools: vec!["codex".to_owned()], created_at: 1, updated_at: 1,
        };
        mcp_store::upsert_mcp_server(&state, &server).await.unwrap();
        let before = snapshot(&state).await;
        let _scope = crate::management_admission::test_session(&session);
        let edit = UpdateMcpServerInput {
            name: Some("changed".to_owned()), server_type: None, server_config: None,
            enabled_tools: Some(vec!["custom_claude".to_owned()]), description: None,
            tags: None, timeout: None,
        };
        assert_eq!(mcp_update_server(app.handle().clone(), state.clone(), server.id.clone(), edit)
            .await.unwrap_err(), UNSUPPORTED);
        assert_eq!(mcp_toggle_tool(app.handle().clone(), state.clone(), server.id.clone(), "custom_claude".to_owned())
            .await.unwrap_err(), UNSUPPORTED);
        assert_eq!(mcp_restore_tools(app.handle().clone(), state.clone(), server.id.clone(), vec!["custom_claude".to_owned()])
            .await.unwrap_err(), UNSUPPORTED);
        assert_eq!(mcp_set_management_enabled(app.handle().clone(), state.clone(), server.id.clone(), false)
            .await.unwrap_err(), UNSUPPORTED);
        assert_eq!(mcp_delete_server(app.handle().clone(), state.clone(), server.id.clone())
            .await.unwrap_err(), UNSUPPORTED);
        assert_eq!(mcp_sync_all(app.handle().clone(), state.clone()).await.unwrap_err(), UNSUPPORTED);
        assert_eq!(snapshot(&state).await, before);
        assert!(!safe.exists());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn managed_import_admits_all_targets_before_first_upsert() {
        let (directory, session) = fixture();
        let app = tauri::test::mock_app();
        app.manage(SqliteDbState::in_memory_for_test().unwrap());
        let state = app.state::<SqliteDbState>();
        let source = directory.path().join("source/mcp.json");
        std::fs::write(&source, r#"{"mcpServers":{"one":{"command":"fixture-one"},"two":{"command":"fixture-two"}}}"#).unwrap();
        custom(&state, "source_fixture", &source).await;
        custom(&state, "custom_codex", &directory.path().join("codex/mcp.json")).await;
        custom(&state, "custom_claude", &directory.path().join("claude/mcp.json")).await;
        let before = snapshot(&state).await;
        let _scope = crate::management_admission::test_session(&session);
        assert_eq!(mcp_import_from_tool(
            app.handle().clone(), state.clone(), "source_fixture".to_owned(),
            Some(vec!["custom_claude".to_owned(), "custom_codex".to_owned()]), Some(false),
        ).await.unwrap_err(), UNSUPPORTED);
        assert_eq!(snapshot(&state).await, before);
        assert!(!directory.path().join("claude/mcp.json").exists());
        assert!(!directory.path().join("codex/mcp.json").exists());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn managed_disjoint_claude_crud_and_ordinary_best_effort_remain_available() {
        let (directory, session) = fixture();
        let app = tauri::test::mock_app();
        app.manage(SqliteDbState::in_memory_for_test().unwrap());
        let state = app.state::<SqliteDbState>();
        let safe = directory.path().join("claude/mcp.json");
        custom(&state, "custom_claude", &safe).await;
        let scope = crate::management_admission::test_session(&session);
        let created = mcp_create_server(app.handle().clone(), state.clone(), input(&["custom_claude"]))
            .await.unwrap();
        let written: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&safe).unwrap()).unwrap();
        assert_eq!(written["mcpServers"]["fixture-server"]["command"], "fixture-command");
        mcp_delete_server(app.handle().clone(), state.clone(), created.id).await.unwrap();
        assert!(mcp_store::get_mcp_servers(&state).await.unwrap().is_empty());
        drop(scope);
        drop(session);
        let lease = ai_toolbox_management::owner::ProfileLeaseStartup::acquire(directory.path().join("profile"))
            .unwrap().into_ordinary();
        let ordinary = GuiProfileSession::ordinary(lease);
        let _scope = crate::management_admission::test_session(&ordinary);
        // Ordinary mode retains its prior behavior for an unknown target:
        // persist the desired state and skip the missing runtime adapter.
        let created = mcp_create_server(app.handle().clone(), state.clone(), input(&["unknown_tool"]))
            .await.unwrap();
        assert_eq!(created.enabled_tools, vec!["unknown_tool"]);
    }
}
