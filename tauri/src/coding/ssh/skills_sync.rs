//! Skills sync to SSH remote
//!
//! Full sync of managed skills to remote server's central repo with symlinks to tool directories.

use std::collections::HashSet;

use log::info;
use tauri::{AppHandle, Emitter};

use super::adapter;
use super::sync::{
    check_remote_symlink_exists, create_remote_symlink, list_remote_dir, read_remote_file,
    remove_remote_path, sync_directory, write_remote_file,
};
use super::types::{SSHConnection, SSHSyncConfig, SyncProgress};
use crate::coding::skills::central_repo::{resolve_central_repo_path, resolve_skill_central_path};
use crate::coding::skills::skill_store;
use crate::coding::tools::builtin::BUILTIN_TOOLS;
use crate::DbState;

const SSH_CENTRAL_DIR: &str = "~/.ai-toolbox/skills";

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

/// Get active connection from config
fn get_active_connection(config: &SSHSyncConfig) -> Option<SSHConnection> {
    config
        .connections
        .iter()
        .find(|c| c.id == config.active_connection_id)
        .cloned()
}

/// Get the remote skills directory path for a tool key
fn get_remote_tool_skills_dir(tool_key: &str) -> Option<String> {
    BUILTIN_TOOLS
        .iter()
        .find(|t| t.key == tool_key && t.relative_skills_dir.is_some())
        .map(|t| {
            let dir = t.relative_skills_dir.unwrap();
            if dir.starts_with("~/") || dir.starts_with("~\\") {
                dir.to_string()
            } else {
                format!("~/{}", dir)
            }
        })
}

/// Get all tool keys that support skills
fn get_all_skill_tool_keys() -> Vec<&'static str> {
    BUILTIN_TOOLS
        .iter()
        .filter(|t| t.relative_skills_dir.is_some())
        .map(|t| t.key)
        .collect()
}

/// Sync all skills to SSH remote (called on skills-changed event)
pub async fn sync_skills_to_ssh(state: &DbState, app: AppHandle) -> Result<(), String> {
    let config = get_ssh_config(state).await?;

    if !config.enabled || !config.sync_skills {
        info!(
            "Skills SSH sync skipped: enabled={}, sync_skills={}",
            config.enabled, config.sync_skills
        );
        return Ok(());
    }

    let conn = match get_active_connection(&config) {
        Some(c) => c,
        None => {
            log::warn!("SSH Skills sync skipped: no active connection");
            return Ok(());
        }
    };

    // Get all managed skills
    let skills = skill_store::get_managed_skills(state).await?;
    let central_dir = resolve_central_repo_path(&app, state)
        .await
        .map_err(|e| format!("{}", e))?;

    let total_skills = skills.len() as u32;
    info!(
        "Skills SSH sync: {} skills found, central_dir={}",
        total_skills,
        central_dir.display()
    );

    // Emit initial progress
    let _ = app.emit(
        "ssh-sync-progress",
        SyncProgress {
            phase: "skills".to_string(),
            current_item: "准备中...".to_string(),
            current: 0,
            total: total_skills,
            message: format!("Skills 同步: 0/{}", total_skills),
        },
    );

    // 1. Get existing skills in remote central repo
    let existing_remote_skills = list_remote_dir(&conn, SSH_CENTRAL_DIR).unwrap_or_default();

    // 2. Collect local skill names
    let local_skill_names: HashSet<String> = skills.iter().map(|s| s.name.clone()).collect();

    // 3. Delete skills in remote that no longer exist locally
    for remote_skill in &existing_remote_skills {
        if !local_skill_names.contains(remote_skill) {
            for tool_key in get_all_skill_tool_keys() {
                if let Some(remote_skills_dir) = get_remote_tool_skills_dir(tool_key) {
                    let link_path = format!("{}/{}", remote_skills_dir, remote_skill);
                    let _ = remove_remote_path(&conn, &link_path);
                }
            }
            let skill_path = format!("{}/{}", SSH_CENTRAL_DIR, remote_skill);
            let _ = remove_remote_path(&conn, &skill_path);
        }
    }

    // 4. Sync/update each skill
    let mut synced_count = 0;
    for (idx, skill) in skills.iter().enumerate() {
        let current_idx = (idx + 1) as u32;

        let _ = app.emit(
            "ssh-sync-progress",
            SyncProgress {
                phase: "skills".to_string(),
                current_item: skill.name.clone(),
                current: current_idx,
                total: total_skills,
                message: format!(
                    "Skills 同步: {}/{} - {}",
                    current_idx, total_skills, skill.name
                ),
            },
        );

        let source = resolve_skill_central_path(&skill.central_path, &central_dir);
        if !source.exists() {
            info!(
                "Skills SSH sync: skip '{}', source not found: {}",
                skill.name,
                source.display()
            );
            continue;
        }

        let remote_target = format!("{}/{}", SSH_CENTRAL_DIR, skill.name);
        let hash_file = format!("{}/.synced_hash", remote_target);

        // Check if content needs updating using content_hash
        let remote_hash = read_remote_file(&conn, &hash_file)
            .unwrap_or_default()
            .trim()
            .to_string();
        let local_hash = skill.content_hash.as_deref().unwrap_or("");

        let needs_update = remote_hash != local_hash;

        if needs_update {
            let source_str = source.to_string_lossy().to_string();
            info!(
                "Skills SSH sync: syncing '{}' from {} to {}",
                skill.name, source_str, remote_target
            );
            match sync_directory(&source_str, &remote_target, &conn) {
                Ok(_) => {
                    write_remote_file(&conn, &hash_file, local_hash)?;
                    synced_count += 1;
                }
                Err(e) => {
                    return Err(format!(
                        "Skills SSH sync failed for '{}': {}",
                        skill.name, e
                    ));
                }
            }
        }

        // Ensure symlinks for each enabled tool
        for tool_key in &skill.enabled_tools {
            if let Some(remote_skills_dir) = get_remote_tool_skills_dir(tool_key) {
                let link_path = format!("{}/{}", remote_skills_dir, skill.name);
                if !check_remote_symlink_exists(&conn, &link_path, &remote_target) {
                    let _ = create_remote_symlink(&conn, &remote_target, &link_path);
                }
            }
        }

        // Remove symlinks for tools that are no longer enabled
        let enabled_set: HashSet<&str> =
            skill.enabled_tools.iter().map(|s| s.as_str()).collect();
        for tool_key in get_all_skill_tool_keys() {
            if !enabled_set.contains(tool_key) {
                if let Some(remote_skills_dir) = get_remote_tool_skills_dir(tool_key) {
                    let link_path = format!("{}/{}", remote_skills_dir, skill.name);
                    let _ = remove_remote_path(&conn, &link_path);
                }
            }
        }
    }

    info!(
        "Skills SSH sync completed: {} skills updated, {} total",
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

    let _ = app.emit("ssh-skills-sync-completed", ());
    let _ = app.emit("ssh-sync-completed", &sync_result);

    Ok(())
}
