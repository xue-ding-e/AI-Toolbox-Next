use serde_json::Value;

use super::types::{Skill, SkillSettings, SkillTarget};

/// Clean a SurrealDB record ID by stripping table prefix and wrapper characters.
fn db_clean_id(raw_id: &str) -> String {
    // Strip table prefix if present (e.g., "skill:xxx" -> "xxx")
    let without_prefix = if let Some(pos) = raw_id.find(':') {
        &raw_id[pos + 1..]
    } else {
        raw_id
    };
    // Strip SurrealDB wrapper characters ⟨⟩ if present
    without_prefix
        .trim_start_matches('⟨')
        .trim_end_matches('⟩')
        .to_string()
}

/// Extract a clean ID from a database record Value.
fn db_extract_id(record: &Value) -> String {
    record
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| db_clean_id(s))
        .unwrap_or_default()
}

/// Convert database record to Skill struct
pub fn from_db_skill(value: Value) -> Skill {
    Skill {
        id: db_extract_id(&value),
        name: value
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        source_type: value
            .get("source_type")
            .and_then(|v| v.as_str())
            .unwrap_or("local")
            .to_string(),
        source_ref: value
            .get("source_ref")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        source_revision: value
            .get("source_revision")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        central_path: value
            .get("central_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        content_hash: value
            .get("content_hash")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        created_at: value.get("created_at").and_then(|v| v.as_i64()).unwrap_or(0),
        updated_at: value.get("updated_at").and_then(|v| v.as_i64()).unwrap_or(0),
        last_sync_at: value.get("last_sync_at").and_then(|v| v.as_i64()),
        status: value
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("active")
            .to_string(),
    }
}

/// Convert database record to SkillTarget struct
pub fn from_db_skill_target(value: Value) -> SkillTarget {
    SkillTarget {
        id: db_extract_id(&value),
        skill_id: value
            .get("skill_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        tool: value
            .get("tool")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        target_path: value
            .get("target_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        mode: value
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("symlink")
            .to_string(),
        status: value
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("pending")
            .to_string(),
        synced_at: value.get("synced_at").and_then(|v| v.as_i64()),
        error_message: value
            .get("error_message")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    }
}

/// Convert database record to SkillSettings struct
pub fn from_db_skill_settings(value: Value) -> SkillSettings {
    SkillSettings {
        central_repo_path: value
            .get("central_repo_path")
            .and_then(|v| v.as_str())
            .unwrap_or(&SkillSettings::default().central_repo_path)
            .to_string(),
        git_cache_cleanup_days: value
            .get("git_cache_cleanup_days")
            .and_then(|v| v.as_i64())
            .unwrap_or(30) as i32,
        git_cache_ttl_secs: value
            .get("git_cache_ttl_secs")
            .and_then(|v| v.as_i64())
            .unwrap_or(60) as i32,
        known_tool_versions: value.get("known_tool_versions").cloned(),
        updated_at: value
            .get("updated_at")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
    }
}

/// Convert Skill to clean database payload (without id)
pub fn to_clean_skill_payload(skill: &Skill) -> Value {
    serde_json::json!({
        "name": skill.name,
        "source_type": skill.source_type,
        "source_ref": skill.source_ref,
        "source_revision": skill.source_revision,
        "central_path": skill.central_path,
        "content_hash": skill.content_hash,
        "created_at": skill.created_at,
        "updated_at": skill.updated_at,
        "last_sync_at": skill.last_sync_at,
        "status": skill.status,
    })
}

/// Convert SkillTarget to clean database payload (without id)
pub fn to_clean_skill_target_payload(target: &SkillTarget) -> Value {
    serde_json::json!({
        "skill_id": target.skill_id,
        "tool": target.tool,
        "target_path": target.target_path,
        "mode": target.mode,
        "status": target.status,
        "synced_at": target.synced_at,
        "error_message": target.error_message,
    })
}

/// Convert SkillSettings to database payload
pub fn to_skill_settings_payload(settings: &SkillSettings) -> Value {
    serde_json::json!({
        "central_repo_path": settings.central_repo_path,
        "git_cache_cleanup_days": settings.git_cache_cleanup_days,
        "git_cache_ttl_secs": settings.git_cache_ttl_secs,
        "known_tool_versions": settings.known_tool_versions,
        "updated_at": settings.updated_at,
    })
}
