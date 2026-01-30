use serde_json::Value;

use crate::DbState;

use super::adapter::{from_db_skill, from_db_skill_target, to_clean_skill_payload, to_clean_skill_target_payload};
use super::types::{now_ms, Skill, SkillTarget};

/// Get all managed skills with their targets
pub async fn get_managed_skills(state: &DbState) -> Result<Vec<Skill>, String> {
    let db = state.0.lock().await;

    // Query all skills
    let mut result = db
        .query("SELECT *, type::string(id) as id FROM skill ORDER BY updated_at DESC")
        .await
        .map_err(|e| format!("Failed to query skills: {}", e))?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;
    let skills: Vec<Skill> = records.into_iter().map(from_db_skill).collect();

    // Query all targets
    let mut result = db
        .query("SELECT *, type::string(id) as id FROM skill_target")
        .await
        .map_err(|e| format!("Failed to query targets: {}", e))?;

    let target_records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;
    let _targets: Vec<SkillTarget> = target_records.into_iter().map(from_db_skill_target).collect();

    // Note: Targets are queried separately via get_skill_targets
    // The commands layer aggregates them when building the DTO

    Ok(skills)
}

/// Get all targets for a specific skill
pub async fn get_skill_targets(state: &DbState, skill_id: &str) -> Result<Vec<SkillTarget>, String> {
    let db = state.0.lock().await;
    let skill_id_owned = skill_id.to_string();

    let mut result = db
        .query("SELECT *, type::string(id) as id FROM skill_target WHERE skill_id = $skill_id ORDER BY tool ASC")
        .bind(("skill_id", skill_id_owned))
        .await
        .map_err(|e| format!("Failed to query targets: {}", e))?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;
    Ok(records.into_iter().map(from_db_skill_target).collect())
}

/// Get a single skill by ID
pub async fn get_skill_by_id(state: &DbState, skill_id: &str) -> Result<Option<Skill>, String> {
    let db = state.0.lock().await;
    let skill_id_owned = skill_id.to_string();

    let mut result = db
        .query("SELECT *, type::string(id) as id FROM skill WHERE id = type::thing('skill', $id) LIMIT 1")
        .bind(("id", skill_id_owned))
        .await
        .map_err(|e| format!("Failed to query skill: {}", e))?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;

    if let Some(record) = records.first() {
        Ok(Some(from_db_skill(record.clone())))
    } else {
        Ok(None)
    }
}

/// Create or update a skill
pub async fn upsert_skill(state: &DbState, skill: &Skill) -> Result<String, String> {
    let db = state.0.lock().await;
    let payload = to_clean_skill_payload(skill);

    if skill.id.is_empty() {
        // Create new skill with generated ID
        let id = uuid::Uuid::new_v4().to_string();
        db.query("CREATE type::thing('skill', $id) CONTENT $data")
            .bind(("id", id.clone()))
            .bind(("data", payload))
            .await
            .map_err(|e| format!("Failed to create skill: {}", e))?;
        Ok(id)
    } else {
        // Update existing skill
        let skill_id = skill.id.clone();
        db.query("UPDATE type::thing('skill', $id) CONTENT $data")
            .bind(("id", skill_id.clone()))
            .bind(("data", payload))
            .await
            .map_err(|e| format!("Failed to update skill: {}", e))?;
        Ok(skill.id.clone())
    }
}

/// Delete a skill
pub async fn delete_skill(state: &DbState, skill_id: &str) -> Result<(), String> {
    let db = state.0.lock().await;
    let skill_id_owned = skill_id.to_string();

    // Delete associated targets first
    db.query("DELETE FROM skill_target WHERE skill_id = $skill_id")
        .bind(("skill_id", skill_id_owned.clone()))
        .await
        .map_err(|e| format!("Failed to delete skill targets: {}", e))?;

    // Delete the skill
    db.query("DELETE FROM skill WHERE id = type::thing('skill', $id)")
        .bind(("id", skill_id_owned))
        .await
        .map_err(|e| format!("Failed to delete skill: {}", e))?;

    Ok(())
}

/// Get a skill target
pub async fn get_skill_target(
    state: &DbState,
    skill_id: &str,
    tool: &str,
) -> Result<Option<SkillTarget>, String> {
    let db = state.0.lock().await;
    let skill_id_owned = skill_id.to_string();
    let tool_owned = tool.to_string();

    let mut result = db
        .query("SELECT *, type::string(id) as id FROM skill_target WHERE skill_id = $skill_id AND tool = $tool LIMIT 1")
        .bind(("skill_id", skill_id_owned))
        .bind(("tool", tool_owned))
        .await
        .map_err(|e| format!("Failed to query target: {}", e))?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;

    if let Some(record) = records.first() {
        Ok(Some(from_db_skill_target(record.clone())))
    } else {
        Ok(None)
    }
}

/// Create or update a skill target
pub async fn upsert_skill_target(state: &DbState, target: &SkillTarget) -> Result<String, String> {
    let db = state.0.lock().await;
    let payload = to_clean_skill_target_payload(target);

    // Use UPSERT with unique constraint on skill_id + tool
    let id = if target.id.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        target.id.clone()
    };

    let skill_id_owned = target.skill_id.clone();
    let tool_owned = target.tool.clone();

    // First try to find existing target
    let mut result = db
        .query("SELECT *, type::string(id) as id FROM skill_target WHERE skill_id = $skill_id AND tool = $tool LIMIT 1")
        .bind(("skill_id", skill_id_owned.clone()))
        .bind(("tool", tool_owned))
        .await
        .map_err(|e| format!("Failed to query target: {}", e))?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;

    if let Some(existing) = records.first() {
        // Update existing
        let existing_id = super::adapter::from_db_skill_target(existing.clone()).id;
        db.query("UPDATE type::thing('skill_target', $id) CONTENT $data")
            .bind(("id", existing_id.clone()))
            .bind(("data", payload))
            .await
            .map_err(|e| format!("Failed to update target: {}", e))?;
        Ok(existing_id)
    } else {
        // Create new
        db.query("CREATE type::thing('skill_target', $id) CONTENT $data")
            .bind(("id", id.clone()))
            .bind(("data", payload))
            .await
            .map_err(|e| format!("Failed to create target: {}", e))?;
        Ok(id)
    }
}

/// Delete a skill target
pub async fn delete_skill_target(state: &DbState, skill_id: &str, tool: &str) -> Result<(), String> {
    let db = state.0.lock().await;
    let skill_id_owned = skill_id.to_string();
    let tool_owned = tool.to_string();

    db.query("DELETE FROM skill_target WHERE skill_id = $skill_id AND tool = $tool")
        .bind(("skill_id", skill_id_owned))
        .bind(("tool", tool_owned))
        .await
        .map_err(|e| format!("Failed to delete target: {}", e))?;

    Ok(())
}

/// Get setting value
pub async fn get_setting(state: &DbState, key: &str) -> Result<Option<String>, String> {
    let db = state.0.lock().await;
    let key_owned = key.to_string();

    let mut result = db
        .query("SELECT * FROM skill_settings:`skills` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query settings: {}", e))?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;

    if let Some(record) = records.first() {
        if let Some(value) = record.get(&key_owned) {
            // Handle string type directly
            if let Some(s) = value.as_str() {
                return Ok(Some(s.to_string()));
            }
            // Handle other types (array, object, etc.) by serializing to JSON string
            if !value.is_null() {
                return Ok(Some(value.to_string()));
            }
        }
    }

    Ok(None)
}

/// Set setting value
pub async fn set_setting(state: &DbState, key: &str, value: &str) -> Result<(), String> {
    let db = state.0.lock().await;
    let now = now_ms();
    let value_owned = value.to_string();

    let query = format!(
        "UPSERT skill_settings:`skills` MERGE {{ {}: $value, updated_at: $now }}",
        key
    );

    db.query(&query)
        .bind(("value", value_owned))
        .bind(("now", now))
        .await
        .map_err(|e| format!("Failed to save setting: {}", e))?;

    Ok(())
}

/// Get all skill target paths for filtering
pub async fn list_all_skill_target_paths(state: &DbState) -> Result<Vec<(String, String)>, String> {
    let db = state.0.lock().await;

    let mut result = db
        .query("SELECT tool, target_path FROM skill_target")
        .await
        .map_err(|e| format!("Failed to query target paths: {}", e))?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;

    let paths: Vec<(String, String)> = records
        .into_iter()
        .filter_map(|v| {
            let tool = v.get("tool").and_then(|t| t.as_str())?.to_string();
            let path = v.get("target_path").and_then(|p| p.as_str())?.to_string();
            Some((tool, path))
        })
        .collect();

    Ok(paths)
}
