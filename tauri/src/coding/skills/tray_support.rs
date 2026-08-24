//! Skills Tray Support Module
//!
//! Provides standardized API for tray menu integration.
//! This module handles all data fetching and processing for skills tray menu display.

use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::adapter::parse_sync_details;
use super::central_repo::{resolve_central_repo_path, resolve_skill_central_path};
use super::path_executor::{remove_skill_target_checked, sync_skill_to_target};
use super::skill_store;
use super::tool_adapters::{
    get_all_tool_adapters, is_tool_installed_with_state_async,
    resolve_runtime_skills_path_with_state_async, runtime_adapter_by_key,
};
use super::types::{now_ms, SkillTarget};
use crate::SqliteDbState;

/// Item for tool selection in skill submenu
#[derive(Debug, Clone)]
pub struct TraySkillToolItem {
    /// Tool key (e.g. "claude_code")
    pub tool_key: String,
    /// Display name (e.g. "Claude Code")
    pub display_name: String,
    /// Whether this skill is synced to this tool
    pub is_synced: bool,
    /// Whether the tool is installed
    pub is_installed: bool,
}

/// Item for skill in tray menu
#[derive(Debug, Clone)]
pub struct TraySkillItem {
    /// Skill ID
    pub id: String,
    /// Skill name for display
    pub display_name: String,
    /// Central path for sync operations
    pub central_path: String,
    /// List of available tools with sync status
    pub tools: Vec<TraySkillToolItem>,
}

/// Data for skills section in tray menu
#[derive(Debug, Clone)]
pub struct TraySkillData {
    /// Title of the skills section
    pub title: String,
    /// List of managed skills
    pub items: Vec<TraySkillItem>,
}

/// Check if skills should be shown in tray menu
/// Reads the show_skills_in_tray setting
pub async fn is_skills_enabled_for_tray<R: Runtime>(app: &AppHandle<R>) -> bool {
    let state = app.state::<SqliteDbState>();
    let raw = skill_store::get_setting(&state, "show_skills_in_tray")
        .await
        .ok()
        .flatten();
    match raw {
        Some(s) => s == "true",
        None => false,
    }
}

/// Get skills tray data
/// Returns all managed skills with their tool sync states
pub async fn get_skills_tray_data<R: Runtime>(app: &AppHandle<R>) -> Result<TraySkillData, String> {
    let state = app.state::<SqliteDbState>();

    let custom_tools = skill_store::get_custom_tools(&state)
        .await
        .unwrap_or_default();
    let all_adapters = get_all_tool_adapters(&custom_tools);

    // Precompute install status once per tool (it does not depend on the skill).
    let mut installed_map: std::collections::HashMap<String, bool> =
        std::collections::HashMap::new();
    for adapter in &all_adapters {
        let installed = is_tool_installed_with_state_async(state.db(), adapter)
            .await
            .unwrap_or(false);
        installed_map.insert(adapter.key.clone(), installed);
    }

    let limit_to_preferred = skill_store::get_setting(&state, "limit_add_more_to_preferred_tools")
        .await
        .ok()
        .flatten()
        .map(|s| s == "true")
        .unwrap_or(false);

    let preferred_tools_raw = skill_store::get_setting(&state, "preferred_tools_v1")
        .await
        .ok()
        .flatten();
    let preferred_tools: Option<Vec<String>> =
        preferred_tools_raw.and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok());

    // Determine the candidate tool set, mirroring the SkillCard "add more" rule:
    // - When "limit add-more to preferred tools" is on AND preferred tools are
    //   configured, only the preferred (common) tools are candidates.
    // - Otherwise every known tool (all adapters, incl. custom tools) is a
    //   candidate; non-installed ones are shown but greyed out.
    let candidate_keys: Vec<String> = if limit_to_preferred {
        match &preferred_tools {
            Some(keys) if !keys.is_empty() => keys.clone(),
            _ => all_adapters.iter().map(|a| a.key.clone()).collect(),
        }
    } else {
        all_adapters.iter().map(|a| a.key.clone()).collect()
    };

    let skills = skill_store::get_managed_skills(&state).await?;
    let mut items: Vec<TraySkillItem> = Vec::new();

    for skill in skills.into_iter().filter(|skill| skill.management_enabled) {
        let targets = parse_sync_details(&skill);
        let synced_tools: std::collections::HashSet<String> =
            targets.iter().map(|target| target.tool.clone()).collect();

        // Per-skill tool list = candidate ∪ already-synced tools. Synced tools
        // are always shown even when the candidate set (preferred tools) would
        // omit them, so an already-synced tool is never dropped from the menu.
        let mut tool_items: Vec<TraySkillToolItem> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Candidate tools first, in candidate order.
        for tool_key in &candidate_keys {
            if !seen.insert(tool_key.clone()) {
                continue;
            }
            let Some(adapter) = all_adapters.iter().find(|item| item.key == *tool_key) else {
                continue;
            };
            tool_items.push(TraySkillToolItem {
                tool_key: tool_key.clone(),
                display_name: adapter.display_name.clone(),
                is_synced: synced_tools.contains(tool_key),
                is_installed: installed_map.get(tool_key).copied().unwrap_or(false),
            });
        }

        // Then any synced tools not already in the candidate set, in adapter order.
        for adapter in &all_adapters {
            if !synced_tools.contains(&adapter.key) || seen.contains(&adapter.key) {
                continue;
            }
            seen.insert(adapter.key.clone());
            tool_items.push(TraySkillToolItem {
                tool_key: adapter.key.clone(),
                display_name: adapter.display_name.clone(),
                is_synced: true,
                is_installed: installed_map
                    .get(&adapter.key)
                    .copied()
                    .unwrap_or(false),
            });
        }

        items.push(TraySkillItem {
            id: skill.id,
            display_name: skill.name,
            central_path: skill.central_path,
            tools: tool_items,
        });
    }

    Ok(TraySkillData {
        title: "──── Skills ────".to_string(),
        items,
    })
}

/// Apply skill tool toggle from tray menu
/// Toggles the sync state of a skill for a specific tool
pub async fn apply_skills_tool_toggle<R: Runtime>(
    app: &AppHandle<R>,
    skill_id: &str,
    tool_key: &str,
) -> Result<(), String> {
    let state = app.state::<SqliteDbState>();

    let custom_tools = skill_store::get_custom_tools(&state)
        .await
        .unwrap_or_default();

    let skill = skill_store::get_skill_by_id(&state, skill_id)
        .await?
        .ok_or_else(|| format!("Skill not found: {}", skill_id))?;
    if !skill.management_enabled {
        return Err(format!("Skill is disabled: {}", skill_id));
    }

    let runtime_adapter = runtime_adapter_by_key(tool_key, &custom_tools)
        .ok_or_else(|| format!("Unknown tool: {}", tool_key))?;

    if !runtime_adapter.is_custom
        && !is_tool_installed_with_state_async(state.db(), &runtime_adapter)
            .await
            .unwrap_or(false)
    {
        return Err(format!("Tool not installed: {}", tool_key));
    }

    let existing_target = skill_store::get_skill_target(&state, skill_id, tool_key).await?;
    let central_dir = resolve_central_repo_path(app, &state)
        .await
        .map_err(|e| format!("{:#}", e))?;
    let skill_source_path = resolve_skill_central_path(&skill.central_path, &central_dir);

    if let Some(target) = existing_target.as_ref() {
        remove_skill_target_checked(&skill_source_path, &target.target_path)
            .map_err(|e| format!("{:#}", e))?;
        skill_store::delete_skill_target(&state, skill_id, tool_key).await?;
    } else {
        let tool_root = resolve_runtime_skills_path_with_state_async(state.db(), &runtime_adapter)
            .await
            .map_err(|e| format!("{:#}", e))?;
        let target = tool_root.join(&skill.name);

        let result = sync_skill_to_target(
            tool_key,
            &skill_source_path,
            &target,
            true,
            runtime_adapter.force_copy,
        )
        .map_err(|e| format!("{:#}", e))?;

        let record = SkillTarget {
            tool: tool_key.to_string(),
            target_path: result.target_path.to_string_lossy().to_string(),
            mode: result.mode_used.as_str().to_string(),
            status: "ok".to_string(),
            error_message: None,
            synced_at: Some(now_ms()),
        };
        skill_store::upsert_skill_target(&state, skill_id, &record).await?;
    }

    let _ = app.emit("skills-changed", "tray");

    Ok(())
}
