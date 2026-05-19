//! Custom tool database operations
//!
//! Provides CRUD operations for user-defined custom tools.

use serde_json::Value;

use super::types::CustomTool;
use crate::coding::db_extract_id;
use crate::coding::db_record_id;
use crate::db::helpers::{db_count, db_delete, db_get, db_list, db_put};
use crate::db::schema::{DbTable, OrderDirection, OrderField, OrderSpec};
use crate::db::sqlite_state::{global_sqlite_state, SqliteDbState};
use crate::DbState;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

/// Convert database record to CustomTool struct
pub fn from_db_custom_tool(value: Value) -> CustomTool {
    let key = db_extract_id(&value);
    CustomTool {
        key,
        display_name: value
            .get("display_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        relative_skills_dir: value
            .get("relative_skills_dir")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        relative_detect_dir: value
            .get("relative_detect_dir")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        force_copy: value
            .get("force_copy")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        mcp_config_path: value
            .get("mcp_config_path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        mcp_config_format: value
            .get("mcp_config_format")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        mcp_field: value
            .get("mcp_field")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        created_at: value
            .get("created_at")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
    }
}

/// Get all custom tools
pub async fn get_custom_tools(state: &DbState) -> Result<Vec<CustomTool>, String> {
    if let Some(sqlite_state) = global_sqlite_state() {
        return get_custom_tools_from_sqlite(sqlite_state);
    }

    get_custom_tools_from_surreal(&state.db()).await
}

async fn get_custom_tools_from_surreal(db: &Surreal<Db>) -> Result<Vec<CustomTool>, String> {
    let mut result = db
        .query("SELECT *, type::string(id) as id FROM custom_tool ORDER BY display_name ASC")
        .await
        .map_err(|e| format!("Failed to query custom tools: {}", e))?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;

    // Filter out any malformed records
    Ok(records
        .into_iter()
        .filter_map(|v| {
            let tool = from_db_custom_tool(v);
            // Skip records with empty key (likely corrupted)
            if tool.key.is_empty() {
                None
            } else {
                Some(tool)
            }
        })
        .collect())
}

fn get_custom_tools_from_sqlite(sqlite_state: &SqliteDbState) -> Result<Vec<CustomTool>, String> {
    sqlite_state.with_conn(|conn| {
        let order = OrderSpec::new(vec![
            OrderField::json_text("display_name", OrderDirection::Asc)?,
            OrderField::id(OrderDirection::Asc),
        ]);
        let records = db_list(conn, DbTable::CustomTool, Some(&order))?;
        Ok(records
            .into_iter()
            .filter_map(|value| {
                let tool = from_db_custom_tool(value);
                (!tool.key.is_empty()).then_some(tool)
            })
            .collect())
    })
}

/// Get custom tools that support Skills (have valid relative_skills_dir, not a root directory)
pub async fn get_skills_custom_tools(state: &DbState) -> Result<Vec<CustomTool>, String> {
    let tools = get_custom_tools(state).await?;
    Ok(tools
        .into_iter()
        .filter(|t| {
            if let Some(ref dir) = t.relative_skills_dir {
                // Skip if the path is a root directory (home or appdata)
                !super::path_utils::is_root_directory(dir)
            } else {
                false
            }
        })
        .collect())
}

/// Get custom tools that support MCP (have valid mcp_config_path, not a root directory)
pub async fn get_mcp_custom_tools(state: &DbState) -> Result<Vec<CustomTool>, String> {
    let tools = get_custom_tools(state).await?;
    Ok(tools
        .into_iter()
        .filter(|t| {
            if let (Some(ref path), Some(_format), Some(_field)) =
                (&t.mcp_config_path, &t.mcp_config_format, &t.mcp_field)
            {
                // Skip if the path is a root directory (home or appdata)
                !super::path_utils::is_root_directory(path)
            } else {
                false
            }
        })
        .collect())
}

/// Get a custom tool by key
pub async fn get_custom_tool_by_key(
    state: &DbState,
    key: &str,
) -> Result<Option<CustomTool>, String> {
    if let Some(sqlite_state) = global_sqlite_state() {
        return sqlite_state.with_conn(|conn| {
            Ok(db_get(conn, DbTable::CustomTool, key)?.map(from_db_custom_tool))
        });
    }

    get_custom_tool_by_key_from_surreal(&state.db(), key).await
}

async fn get_custom_tool_by_key_from_surreal(
    db: &Surreal<Db>,
    key: &str,
) -> Result<Option<CustomTool>, String> {
    let record_id = db_record_id("custom_tool", key);

    let mut result = db
        .query(&format!(
            "SELECT *, type::string(id) as id FROM {} LIMIT 1",
            record_id
        ))
        .await
        .map_err(|e| format!("Failed to query custom tool: {}", e))?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;

    Ok(records.into_iter().next().map(from_db_custom_tool))
}

/// Save a custom tool (create or update), merging with existing fields
pub async fn save_custom_tool(state: &DbState, tool: &CustomTool) -> Result<(), String> {
    if let Some(sqlite_state) = global_sqlite_state() {
        save_custom_tool_to_sqlite(sqlite_state, tool)?;
    }

    save_custom_tool_to_surreal(&state.db(), tool).await
}

async fn save_custom_tool_to_surreal(db: &Surreal<Db>, tool: &CustomTool) -> Result<(), String> {
    let record_id = db_record_id("custom_tool", &tool.key);
    db.query(&format!("UPSERT {} SET display_name = $display_name, relative_skills_dir = $skills_dir, relative_detect_dir = $detect_dir, force_copy = $force_copy, mcp_config_path = $mcp_path, mcp_config_format = $mcp_format, mcp_field = $mcp_field, created_at = $created_at", record_id))
        .bind(("display_name", tool.display_name.clone()))
        .bind(("skills_dir", tool.relative_skills_dir.clone()))
        .bind(("detect_dir", tool.relative_detect_dir.clone()))
        .bind(("force_copy", tool.force_copy))
        .bind(("mcp_path", tool.mcp_config_path.clone()))
        .bind(("mcp_format", tool.mcp_config_format.clone()))
        .bind(("mcp_field", tool.mcp_field.clone()))
        .bind(("created_at", tool.created_at))
        .await
        .map_err(|e| format!("Failed to save custom tool: {}", e))?;

    Ok(())
}

fn save_custom_tool_to_sqlite(
    sqlite_state: &SqliteDbState,
    tool: &CustomTool,
) -> Result<(), String> {
    sqlite_state.with_conn(|conn| {
        db_put(
            conn,
            DbTable::CustomTool,
            &tool.key,
            &custom_tool_to_value(tool),
        )
    })
}

/// Save only skills-related fields, preserving MCP fields if they exist
pub async fn save_custom_tool_skills_fields(
    state: &DbState,
    key: &str,
    display_name: &str,
    relative_skills_dir: Option<String>,
    relative_detect_dir: Option<String>,
    force_copy: bool,
    created_at: i64,
) -> Result<(), String> {
    // First check if the tool already exists
    let existing = get_custom_tool_by_key(state, key).await?;

    // Preserve existing MCP fields
    let (mcp_path, mcp_format, mcp_field) = match existing {
        Some(e) => (e.mcp_config_path, e.mcp_config_format, e.mcp_field),
        None => (None, None, None),
    };

    save_custom_tool(
        state,
        &CustomTool {
            key: key.to_string(),
            display_name: display_name.to_string(),
            relative_skills_dir,
            relative_detect_dir,
            force_copy,
            mcp_config_path: mcp_path,
            mcp_config_format: mcp_format,
            mcp_field,
            created_at,
        },
    )
    .await
}

/// Save only MCP-related fields, preserving Skills fields if they exist
pub async fn save_custom_tool_mcp_fields(
    state: &DbState,
    key: &str,
    display_name: &str,
    relative_detect_dir: Option<String>,
    mcp_config_path: Option<String>,
    mcp_config_format: Option<String>,
    mcp_field: Option<String>,
    created_at: i64,
) -> Result<(), String> {
    // First check if the tool already exists
    let existing = get_custom_tool_by_key(state, key).await?;
    let existing_force_copy = existing
        .as_ref()
        .map(|tool| tool.force_copy)
        .unwrap_or(false);

    // Preserve existing skills fields
    let (skills_dir, detect_dir) = match existing {
        Some(e) => (
            e.relative_skills_dir,
            e.relative_detect_dir.or(relative_detect_dir),
        ),
        None => (None, relative_detect_dir),
    };

    save_custom_tool(
        state,
        &CustomTool {
            key: key.to_string(),
            display_name: display_name.to_string(),
            relative_skills_dir: skills_dir,
            relative_detect_dir: detect_dir,
            force_copy: existing_force_copy,
            mcp_config_path,
            mcp_config_format,
            mcp_field,
            created_at,
        },
    )
    .await
}

/// Delete a custom tool
pub async fn delete_custom_tool(state: &DbState, key: &str) -> Result<(), String> {
    if let Some(sqlite_state) = global_sqlite_state() {
        sqlite_state.with_conn(|conn| db_delete(conn, DbTable::CustomTool, key).map(|_| ()))?;
    }

    delete_custom_tool_from_surreal(&state.db(), key).await
}

async fn delete_custom_tool_from_surreal(db: &Surreal<Db>, key: &str) -> Result<(), String> {
    let record_id = db_record_id("custom_tool", key);

    db.query(&format!("DELETE {}", record_id))
        .await
        .map_err(|e| format!("Failed to delete custom tool: {}", e))?;

    Ok(())
}

pub async fn sync_sqlite_custom_tools_from_surreal_if_missing(
    sqlite_state: &SqliteDbState,
    db: &Surreal<Db>,
) -> Result<(), String> {
    let sqlite_count = sqlite_state.with_conn(|conn| db_count(conn, DbTable::CustomTool))?;
    if sqlite_count > 0 {
        return Ok(());
    }

    let tools = get_custom_tools_from_surreal(db).await?;
    if tools.is_empty() {
        return Ok(());
    }

    sqlite_state.with_conn(|conn| {
        for tool in &tools {
            db_put(
                conn,
                DbTable::CustomTool,
                &tool.key,
                &custom_tool_to_value(tool),
            )?;
        }
        Ok(())
    })
}

fn custom_tool_to_value(tool: &CustomTool) -> Value {
    serde_json::json!({
        "display_name": tool.display_name,
        "relative_skills_dir": tool.relative_skills_dir,
        "relative_detect_dir": tool.relative_detect_dir,
        "force_copy": tool.force_copy,
        "mcp_config_path": tool.mcp_config_path,
        "mcp_config_format": tool.mcp_config_format,
        "mcp_field": tool.mcp_field,
        "created_at": tool.created_at,
    })
}

/// Check if a custom tool key conflicts with built-in tools
pub fn is_builtin_tool_key(key: &str) -> bool {
    super::builtin::builtin_tool_by_key(key).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_custom_tools_round_trip_and_order_by_display_name() {
        let sqlite_state = SqliteDbState::in_memory_for_test().expect("sqlite");
        save_custom_tool_to_sqlite(
            &sqlite_state,
            &CustomTool {
                key: "tool_b".to_string(),
                display_name: "Zulu".to_string(),
                relative_skills_dir: Some("skills".to_string()),
                relative_detect_dir: Some("detect".to_string()),
                force_copy: true,
                mcp_config_path: None,
                mcp_config_format: None,
                mcp_field: None,
                created_at: 2,
            },
        )
        .expect("save tool b");
        save_custom_tool_to_sqlite(
            &sqlite_state,
            &CustomTool {
                key: "tool_a".to_string(),
                display_name: "Alpha".to_string(),
                relative_skills_dir: None,
                relative_detect_dir: None,
                force_copy: false,
                mcp_config_path: Some("mcp.json".to_string()),
                mcp_config_format: Some("json".to_string()),
                mcp_field: Some("mcpServers".to_string()),
                created_at: 1,
            },
        )
        .expect("save tool a");

        let tools = get_custom_tools_from_sqlite(&sqlite_state).expect("list tools");
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].key, "tool_a");
        assert_eq!(tools[0].display_name, "Alpha");
        assert_eq!(tools[0].mcp_config_path.as_deref(), Some("mcp.json"));
        assert_eq!(tools[1].key, "tool_b");
        assert!(tools[1].force_copy);
        assert_eq!(tools[1].relative_skills_dir.as_deref(), Some("skills"));
    }
}
