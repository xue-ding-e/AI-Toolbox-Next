use std::path::PathBuf;
use std::time::Duration;

use tauri::State;

use super::cache_cleanup::{cleanup_git_cache_dirs, get_git_cache_cleanup_days, set_git_cache_cleanup_days as set_cleanup_days, get_git_cache_ttl_secs};
use super::central_repo::{ensure_central_repo, expand_home_path, resolve_central_repo_path};
use super::git_fetcher::set_proxy;
use super::installer::{install_git_skill, install_git_skill_from_selection, install_local_skill, list_git_skills, update_managed_skill_from_source};
use super::onboarding::build_onboarding_plan;
use super::skill_store;
use super::sync_engine::{remove_path, sync_dir_for_tool_with_overwrite};
use super::tool_adapters::{adapter_by_key, default_tool_adapters, is_tool_installed, resolve_default_path};
use super::types::{
    GitSkillCandidate, InstallResultDto, ManagedSkillDto, OnboardingPlan, SkillTarget,
    SkillTargetDto, SyncResultDto, ToolInfoDto, ToolStatusDto, UpdateResultDto, now_ms,
};
use crate::http_client;
use crate::DbState;

fn format_error(err: anyhow::Error) -> String {
    let first = err.to_string();
    // Frontend relies on these prefixes for special flows
    if first.starts_with("MULTI_SKILLS|")
        || first.starts_with("TARGET_EXISTS|")
        || first.starts_with("TOOL_NOT_INSTALLED|")
    {
        return first;
    }
    format!("{:#}", err)
}

// --- Tool Status ---

#[tauri::command]
pub async fn skills_get_tool_status(state: State<'_, DbState>) -> Result<ToolStatusDto, String> {
    let adapters = default_tool_adapters();
    let mut tools: Vec<ToolInfoDto> = Vec::new();
    let mut installed: Vec<String> = Vec::new();

    for adapter in &adapters {
        let ok = is_tool_installed(adapter).unwrap_or(false);
        let key = adapter.id.as_key().to_string();
        tools.push(ToolInfoDto {
            key: key.clone(),
            label: adapter.display_name.to_string(),
            installed: ok,
        });
        if ok {
            installed.push(key);
        }
    }

    installed.dedup();

    // Track newly installed tools
    let prev: Vec<String> = skill_store::get_setting(&state, "installed_tools_v1")
        .await
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
        .unwrap_or_default();

    let prev_set: std::collections::HashSet<String> = prev.into_iter().collect();
    let newly_installed: Vec<String> = installed
        .iter()
        .filter(|k| !prev_set.contains(*k))
        .cloned()
        .collect();

    // Persist current set
    let _ = skill_store::set_setting(
        &state,
        "installed_tools_v1",
        &serde_json::to_string(&installed).unwrap_or_else(|_| "[]".to_string()),
    )
    .await;

    Ok(ToolStatusDto {
        tools,
        installed,
        newly_installed,
    })
}

// --- Central Repo Path ---

#[tauri::command]
pub async fn skills_get_central_repo_path(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
) -> Result<String, String> {
    let path = resolve_central_repo_path(&app, &state).await.map_err(|e| format_error(e))?;
    ensure_central_repo(&path).map_err(|e| format_error(e))?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn skills_set_central_repo_path(
    state: State<'_, DbState>,
    path: String,
) -> Result<String, String> {
    let new_base = expand_home_path(&path).map_err(|e| format_error(e))?;
    if !new_base.is_absolute() {
        return Err("storage path must be absolute".to_string());
    }
    ensure_central_repo(&new_base).map_err(|e| format_error(e))?;

    // Save new path to settings
    skill_store::set_setting(&state, "central_repo_path", new_base.to_string_lossy().as_ref())
        .await
        .map_err(|e| e)?;

    Ok(new_base.to_string_lossy().to_string())
}

// --- Managed Skills ---

#[tauri::command]
pub async fn skills_get_managed_skills(state: State<'_, DbState>) -> Result<Vec<ManagedSkillDto>, String> {
    let skills = skill_store::get_managed_skills(&state).await?;

    let mut result: Vec<ManagedSkillDto> = Vec::new();
    for skill in skills {
        let targets = skill_store::get_skill_targets(&state, &skill.id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|t| SkillTargetDto {
                tool: t.tool,
                mode: t.mode,
                status: t.status,
                target_path: t.target_path,
                synced_at: t.synced_at,
            })
            .collect();

        result.push(ManagedSkillDto {
            id: skill.id,
            name: skill.name,
            source_type: skill.source_type,
            source_ref: skill.source_ref,
            central_path: skill.central_path,
            created_at: skill.created_at,
            updated_at: skill.updated_at,
            last_sync_at: skill.last_sync_at,
            status: skill.status,
            targets,
        });
    }

    Ok(result)
}

// --- Install Skills ---

#[tauri::command]
#[allow(non_snake_case)]
pub async fn skills_install_local(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    sourcePath: String,
    overwrite: Option<bool>,
) -> Result<InstallResultDto, String> {
    let result = install_local_skill(&app, &state, std::path::Path::new(&sourcePath), overwrite.unwrap_or(false))
        .await
        .map_err(|e| format_error(e))?;

    Ok(InstallResultDto {
        skill_id: result.skill_id,
        name: result.name,
        central_path: result.central_path.to_string_lossy().to_string(),
        content_hash: result.content_hash,
    })
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn skills_install_git(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    repoUrl: String,
    branch: Option<String>,
    overwrite: Option<bool>,
) -> Result<InstallResultDto, String> {
    let result = install_git_skill(&app, &state, &repoUrl, branch.as_deref(), overwrite.unwrap_or(false))
        .await
        .map_err(|e| format_error(e))?;

    Ok(InstallResultDto {
        skill_id: result.skill_id,
        name: result.name,
        central_path: result.central_path.to_string_lossy().to_string(),
        content_hash: result.content_hash,
    })
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn skills_list_git_skills(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    repoUrl: String,
    branch: Option<String>,
) -> Result<Vec<GitSkillCandidate>, String> {
    // Initialize proxy from app settings
    let proxy_url = http_client::get_proxy_from_settings(&state).await.ok();
    set_proxy(proxy_url);

    let ttl = get_git_cache_ttl_secs(&state).await;
    let branch_clone = branch.clone();

    tokio::task::spawn_blocking(move || {
        list_git_skills(&app, ttl, &repoUrl, branch_clone.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format_error(e))
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn skills_install_git_selection(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    repoUrl: String,
    subpath: String,
    branch: Option<String>,
    overwrite: Option<bool>,
) -> Result<InstallResultDto, String> {
    let result = install_git_skill_from_selection(&app, &state, &repoUrl, &subpath, branch.as_deref(), overwrite.unwrap_or(false))
        .await
        .map_err(|e| format_error(e))?;

    Ok(InstallResultDto {
        skill_id: result.skill_id,
        name: result.name,
        central_path: result.central_path.to_string_lossy().to_string(),
        content_hash: result.content_hash,
    })
}

// --- Sync Skills ---

#[tauri::command]
#[allow(non_snake_case)]
pub async fn skills_sync_to_tool(
    state: State<'_, DbState>,
    sourcePath: String,
    skillId: String,
    tool: String,
    name: String,
    overwrite: Option<bool>,
) -> Result<SyncResultDto, String> {
    let adapter = adapter_by_key(&tool).ok_or_else(|| "unknown tool".to_string())?;
    if !is_tool_installed(&adapter).unwrap_or(false) {
        return Err(format!("TOOL_NOT_INSTALLED|{}", adapter.id.as_key()));
    }
    let tool_root = resolve_default_path(&adapter).map_err(|e| format_error(e))?;
    let target = tool_root.join(&name);
    let overwrite = overwrite.unwrap_or(false);

    let result = sync_dir_for_tool_with_overwrite(&tool, std::path::Path::new(&sourcePath), &target, overwrite)
        .map_err(|err| {
            let msg = err.to_string();
            if msg.contains("target already exists") {
                format!("TARGET_EXISTS|{}", target.to_string_lossy())
            } else {
                format_error(err)
            }
        })?;

    let record = SkillTarget {
        id: String::new(),
        skill_id: skillId,
        tool,
        target_path: result.target_path.to_string_lossy().to_string(),
        mode: result.mode_used.as_str().to_string(),
        status: "ok".to_string(),
        error_message: None,
        synced_at: Some(now_ms()),
    };
    skill_store::upsert_skill_target(&state, &record).await?;

    Ok(SyncResultDto {
        mode_used: result.mode_used.as_str().to_string(),
        target_path: result.target_path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn skills_unsync_from_tool(
    state: State<'_, DbState>,
    skillId: String,
    tool: String,
) -> Result<(), String> {
    // If the tool is not installed, do nothing
    if let Some(adapter) = adapter_by_key(&tool) {
        if !is_tool_installed(&adapter).unwrap_or(false) {
            return Ok(());
        }
    }

    if let Some(target) = skill_store::get_skill_target(&state, &skillId, &tool).await? {
        // Remove the link/copy in tool directory first
        remove_path(&target.target_path)?;
        skill_store::delete_skill_target(&state, &skillId, &tool).await?;
    }

    Ok(())
}

// --- Update/Delete Skills ---

#[tauri::command]
#[allow(non_snake_case)]
pub async fn skills_update_managed(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    skillId: String,
) -> Result<UpdateResultDto, String> {
    let res = update_managed_skill_from_source(&app, &state, &skillId)
        .await
        .map_err(|e| format_error(e))?;

    Ok(UpdateResultDto {
        skill_id: res.skill_id,
        name: res.name,
        content_hash: res.content_hash,
        source_revision: res.source_revision,
        updated_targets: res.updated_targets,
    })
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn skills_delete_managed(
    state: State<'_, DbState>,
    skillId: String,
) -> Result<(), String> {
    // Delete synced targets first
    let targets = skill_store::get_skill_targets(&state, &skillId).await?;

    let mut remove_failures: Vec<String> = Vec::new();
    for target in targets {
        if let Err(err) = remove_path(&target.target_path) {
            remove_failures.push(format!("{}: {}", target.target_path, err));
        }
    }

    let record = skill_store::get_skill_by_id(&state, &skillId).await?;
    if let Some(skill) = record {
        let path = PathBuf::from(skill.central_path);
        if path.exists() {
            std::fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
        }
        skill_store::delete_skill(&state, &skillId).await?;
    }

    if !remove_failures.is_empty() {
        return Err(format!(
            "Deleted managed record, but some tool directories could not be cleaned:\n- {}",
            remove_failures.join("\n- ")
        ));
    }

    Ok(())
}

// --- Onboarding ---

#[tauri::command]
pub async fn skills_get_onboarding_plan(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
) -> Result<OnboardingPlan, String> {
    build_onboarding_plan(&app, &state)
        .await
        .map_err(|e| format_error(e))
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn skills_import_existing(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    sourcePath: String,
    overwrite: Option<bool>,
) -> Result<InstallResultDto, String> {
    let result = install_local_skill(&app, &state, std::path::Path::new(&sourcePath), overwrite.unwrap_or(false))
        .await
        .map_err(|e| format_error(e))?;

    Ok(InstallResultDto {
        skill_id: result.skill_id,
        name: result.name,
        central_path: result.central_path.to_string_lossy().to_string(),
        content_hash: result.content_hash,
    })
}

// --- Git Cache ---

#[tauri::command]
pub async fn skills_get_git_cache_cleanup_days(state: State<'_, DbState>) -> Result<i64, String> {
    Ok(get_git_cache_cleanup_days(&state).await)
}

#[tauri::command]
pub async fn skills_set_git_cache_cleanup_days(
    state: State<'_, DbState>,
    days: i64,
) -> Result<i64, String> {
    set_cleanup_days(&state, days)
        .await
        .map_err(|e| format_error(e))
}

#[tauri::command]
pub async fn skills_get_git_cache_ttl_secs(state: State<'_, DbState>) -> Result<i64, String> {
    Ok(get_git_cache_ttl_secs(&state).await)
}

#[tauri::command]
pub async fn skills_clear_git_cache(app: tauri::AppHandle) -> Result<usize, String> {
    cleanup_git_cache_dirs(&app, Duration::from_secs(0)).map_err(|e| format_error(e))
}

#[tauri::command]
pub async fn skills_get_git_cache_path(app: tauri::AppHandle) -> Result<String, String> {
    use tauri::Manager;
    let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    let cache_path = cache_dir.join("skills-git-cache");
    Ok(cache_path.to_string_lossy().to_string())
}

// --- Preferred Tools ---

#[tauri::command]
pub async fn skills_get_preferred_tools(state: State<'_, DbState>) -> Result<Option<Vec<String>>, String> {
    let raw = skill_store::get_setting(&state, "preferred_tools_v1")
        .await
        .ok()
        .flatten();
    match raw {
        Some(s) => Ok(serde_json::from_str::<Vec<String>>(&s).ok()),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn skills_set_preferred_tools(
    state: State<'_, DbState>,
    tools: Vec<String>,
) -> Result<(), String> {
    skill_store::set_setting(
        &state,
        "preferred_tools_v1",
        &serde_json::to_string(&tools).unwrap_or_else(|_| "[]".to_string()),
    )
    .await
}
