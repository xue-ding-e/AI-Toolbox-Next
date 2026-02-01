//! Skills sync to WSL
//!
//! Full sync of managed skills to WSL's central repo with symlinks to tool directories.

use std::collections::HashSet;

use log::info;
use tauri::AppHandle;

use super::adapter;
use super::sync::{
    check_wsl_symlink_exists, create_wsl_symlink, list_wsl_dir, read_wsl_file, remove_wsl_path,
    sync_directory, write_wsl_file,
};
use super::types::WSLSyncConfig;
use crate::coding::skills::central_repo::{resolve_central_repo_path, resolve_skill_central_path};
use crate::coding::skills::skill_store;
use crate::coding::tools::builtin::BUILTIN_TOOLS;
use crate::DbState;

const WSL_CENTRAL_DIR: &str = "~/.ai-toolbox/skills";

/// Read WSL sync config directly from database
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

/// Get the WSL skills directory path for a tool key
fn get_wsl_tool_skills_dir(tool_key: &str) -> Option<String> {
    BUILTIN_TOOLS
        .iter()
        .find(|t| t.key == tool_key && t.relative_skills_dir.is_some())
        .map(|t| format!("~/{}", t.relative_skills_dir.unwrap()))
}

/// Get all tool keys that support skills
fn get_all_skill_tool_keys() -> Vec<&'static str> {
    BUILTIN_TOOLS
        .iter()
        .filter(|t| t.relative_skills_dir.is_some())
        .map(|t| t.key)
        .collect()
}

/// Sync all skills to WSL (called on skills-changed event)
pub async fn sync_skills_to_wsl(state: &DbState, app: AppHandle) -> Result<(), String> {
    let config = get_wsl_config(state).await?;

    if !config.enabled || !config.sync_skills {
        return Ok(());
    }

    let distro = &config.distro;

    // Get all managed skills
    let skills = skill_store::get_managed_skills(state).await?;
    let central_dir = resolve_central_repo_path(&app, state)
        .await
        .map_err(|e| format!("{}", e))?;

    // 1. Get existing skills in WSL central repo
    let existing_wsl_skills = list_wsl_dir(distro, WSL_CENTRAL_DIR).unwrap_or_default();

    // 2. Collect Windows skill names
    let windows_skill_names: HashSet<String> = skills.iter().map(|s| s.name.clone()).collect();

    // 3. Delete skills in WSL that no longer exist in Windows
    for wsl_skill in &existing_wsl_skills {
        if !windows_skill_names.contains(wsl_skill) {
            // Remove symlinks from all tool directories first
            for tool_key in get_all_skill_tool_keys() {
                if let Some(wsl_skills_dir) = get_wsl_tool_skills_dir(tool_key) {
                    let link_path = format!("{}/{}", wsl_skills_dir, wsl_skill);
                    let _ = remove_wsl_path(distro, &link_path);
                }
            }
            // Remove from central repo
            let skill_path = format!("{}/{}", WSL_CENTRAL_DIR, wsl_skill);
            let _ = remove_wsl_path(distro, &skill_path);
        }
    }

    // 4. Sync/update each skill
    let mut synced_count = 0;
    for skill in &skills {
        let source = resolve_skill_central_path(&skill.central_path, &central_dir);
        if !source.exists() {
            continue;
        }

        let wsl_target = format!("{}/{}", WSL_CENTRAL_DIR, skill.name);
        let hash_file = format!("{}/.synced_hash", wsl_target);

        // Check if content needs updating using content_hash
        let wsl_hash = read_wsl_file(distro, &hash_file)
            .unwrap_or_default()
            .trim()
            .to_string();
        let windows_hash = skill.content_hash.as_deref().unwrap_or("");

        let needs_update = wsl_hash != windows_hash;

        if needs_update {
            // Convert Windows path to WSL-accessible path and sync
            let source_str = source.to_string_lossy().to_string();
            sync_directory(&source_str, &wsl_target, distro)?;
            // Save hash for future comparison
            write_wsl_file(distro, &hash_file, windows_hash)?;
            synced_count += 1;
        }

        // Ensure symlinks for each enabled tool
        for tool_key in &skill.enabled_tools {
            if let Some(wsl_skills_dir) = get_wsl_tool_skills_dir(tool_key) {
                let link_path = format!("{}/{}", wsl_skills_dir, skill.name);
                if !check_wsl_symlink_exists(distro, &link_path, &wsl_target) {
                    let _ = create_wsl_symlink(distro, &wsl_target, &link_path);
                }
            }
        }

        // Remove symlinks for tools that are no longer enabled
        let enabled_set: HashSet<&str> =
            skill.enabled_tools.iter().map(|s| s.as_str()).collect();
        for tool_key in get_all_skill_tool_keys() {
            if !enabled_set.contains(tool_key) {
                if let Some(wsl_skills_dir) = get_wsl_tool_skills_dir(tool_key) {
                    let link_path = format!("{}/{}", wsl_skills_dir, skill.name);
                    let _ = remove_wsl_path(distro, &link_path);
                }
            }
        }
    }

    info!(
        "Skills WSL sync completed: {} skills updated, {} total",
        synced_count,
        skills.len()
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
    let _ = tauri::Emitter::emit(&app, "wsl-skills-sync-completed", ());
    let _ = tauri::Emitter::emit(&app, "wsl-sync-completed", &sync_result);

    Ok(())
}
