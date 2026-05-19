//! MCP Server database operations
//!
//! Provides CRUD operations for MCP server management.
//! Uses backtick-escaped record references instead of type::thing() to avoid
//! UUID parsing issues across SurrealDB versions.

use serde_json::Value;

use super::adapter::{
    from_db_favorite_mcp, from_db_mcp_preferences, from_db_mcp_server, remove_sync_detail,
    set_sync_detail, to_clean_mcp_server_payload, to_mcp_preferences_payload,
};
use super::command_normalize;
use super::types::{now_ms, FavoriteMcp, McpPreferences, McpServer, McpSyncDetail};
use crate::coding::db_id::{db_new_id, db_record_id};
use crate::db::helpers::{
    db_count, db_delete, db_get, db_list, db_max_i64, db_put, db_query_by_field,
};
use crate::db::schema::{DbTable, JsonFieldPath, OrderDirection, OrderField, OrderSpec};
use crate::db::sqlite_state::{global_sqlite_state, SqliteDbState};
use crate::DbState;

// ==================== MCP Server CRUD ====================

/// Get all MCP servers ordered by sort_index
pub async fn get_mcp_servers(state: &DbState) -> Result<Vec<McpServer>, String> {
    if let Some(sqlite_state) = global_sqlite_state() {
        return sqlite_state.with_conn(|conn| {
            let order = OrderSpec::new(vec![
                OrderField::json_integer("sort_index", OrderDirection::Asc)?,
                OrderField::id(OrderDirection::Asc),
            ]);
            let records = db_list(conn, DbTable::McpServer, Some(&order))?;
            Ok(records.into_iter().map(from_db_mcp_server).collect())
        });
    }

    let db = state.db();

    let mut result = db
        .query("SELECT *, type::string(id) as id FROM mcp_server ORDER BY sort_index ASC")
        .await
        .map_err(|e| format!("Failed to query MCP servers: {}", e))?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;
    Ok(records.into_iter().map(from_db_mcp_server).collect())
}

/// Get a single MCP server by ID
pub async fn get_mcp_server_by_id(
    state: &DbState,
    server_id: &str,
) -> Result<Option<McpServer>, String> {
    if let Some(sqlite_state) = global_sqlite_state() {
        return sqlite_state.with_conn(|conn| {
            Ok(db_get(conn, DbTable::McpServer, server_id)?.map(from_db_mcp_server))
        });
    }

    let db = state.db();
    let record_id = db_record_id("mcp_server", server_id);

    let mut result = db
        .query(&format!(
            "SELECT *, type::string(id) as id FROM {} LIMIT 1",
            record_id
        ))
        .await
        .map_err(|e| format!("Failed to query MCP server: {}", e))?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;
    Ok(records.first().map(|r| from_db_mcp_server(r.clone())))
}

/// Get MCP server by name
pub async fn get_mcp_server_by_name(
    state: &DbState,
    name: &str,
) -> Result<Option<McpServer>, String> {
    if let Some(sqlite_state) = global_sqlite_state() {
        return sqlite_state.with_conn(|conn| {
            let records = db_query_by_field(
                conn,
                DbTable::McpServer,
                &JsonFieldPath::new("name")?,
                &Value::String(name.to_string()),
                None,
                Some(1),
            )?;
            Ok(records.into_iter().next().map(from_db_mcp_server))
        });
    }

    let db = state.db();
    let name_owned = name.to_string();

    let mut result = db
        .query("SELECT *, type::string(id) as id FROM mcp_server WHERE name = $name LIMIT 1")
        .bind(("name", name_owned))
        .await
        .map_err(|e| format!("Failed to query MCP server by name: {}", e))?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;
    Ok(records.first().map(|r| from_db_mcp_server(r.clone())))
}

/// Create or update an MCP server
pub async fn upsert_mcp_server(state: &DbState, server: &McpServer) -> Result<String, String> {
    let db = state.db();

    // Normalize server_config: remove cmd /c wrapper for database storage (only for stdio type)
    let normalized_config = if server.server_type == "stdio" {
        command_normalize::unwrap_cmd_c(&server.server_config)
    } else {
        server.server_config.clone()
    };

    let sqlite_id = if let Some(sqlite_state) = global_sqlite_state() {
        let id = if server.id.is_empty() {
            db_new_id()
        } else {
            server.id.clone()
        };
        sqlite_state.with_conn(|conn| {
            let mut sqlite_server = server.clone();
            sqlite_server.id = id.clone();
            sqlite_server.server_config = normalized_config.clone();
            if server.id.is_empty() {
                let max_index =
                    db_max_i64(conn, DbTable::McpServer, &JsonFieldPath::new("sort_index")?)?
                        .unwrap_or(-1) as i32;
                sqlite_server.sort_index = max_index + 1;
            }
            db_put(
                conn,
                DbTable::McpServer,
                &id,
                &to_clean_mcp_server_payload(&sqlite_server),
            )
        })?;
        Some(id)
    } else {
        None
    };

    if server.id.is_empty() {
        // Get max sort_index for new server
        let mut max_result = db
            .query("SELECT sort_index FROM mcp_server ORDER BY sort_index DESC LIMIT 1")
            .await
            .map_err(|e| format!("Failed to query max sort_index: {}", e))?;
        let max_records: Vec<Value> = max_result.take(0).map_err(|e| e.to_string())?;
        let max_index = max_records
            .first()
            .and_then(|v| v.get("sort_index"))
            .and_then(|v| v.as_i64())
            .unwrap_or(-1) as i32;

        // Create new server with sort_index = max + 1 and normalized config
        let mut new_server = server.clone();
        new_server.sort_index = max_index + 1;
        new_server.server_config = normalized_config;
        let payload = to_clean_mcp_server_payload(&new_server);

        let id = sqlite_id.unwrap_or_else(db_new_id);
        let record_id = db_record_id("mcp_server", &id);
        db.query(&format!("CREATE {} CONTENT $data", record_id))
            .bind(("data", payload))
            .await
            .map_err(|e| format!("Failed to create MCP server: {}", e))?;
        Ok(id)
    } else {
        // Update existing server with normalized config
        let mut updated_server = server.clone();
        updated_server.server_config = normalized_config;
        let payload = to_clean_mcp_server_payload(&updated_server);
        let record_id = db_record_id("mcp_server", &server.id);
        db.query(&format!("UPDATE {} CONTENT $data", record_id))
            .bind(("data", payload))
            .await
            .map_err(|e| format!("Failed to update MCP server: {}", e))?;
        Ok(server.id.clone())
    }
}

/// Delete an MCP server
pub async fn delete_mcp_server(state: &DbState, server_id: &str) -> Result<(), String> {
    if let Some(sqlite_state) = global_sqlite_state() {
        sqlite_state
            .with_conn(|conn| db_delete(conn, DbTable::McpServer, server_id).map(|_| ()))?;
    }

    let db = state.db();
    let record_id = db_record_id("mcp_server", server_id);

    db.query(&format!("DELETE {}", record_id))
        .await
        .map_err(|e| format!("Failed to delete MCP server: {}", e))?;

    Ok(())
}

/// Update user-managed metadata for an MCP server without touching sync state.
pub async fn update_mcp_server_metadata(
    state: &DbState,
    server_id: &str,
    user_group: Option<String>,
    user_note: Option<String>,
) -> Result<(), String> {
    if global_sqlite_state().is_some() {
        if let Some(mut server) = get_mcp_server_by_id(state, server_id).await? {
            server.user_group = user_group.clone();
            server.user_note = user_note.clone();
            server.updated_at = now_ms();
            upsert_mcp_server(state, &server).await?;
            return Ok(());
        }
    }

    let db = state.db();
    let record_id = db_record_id("mcp_server", server_id);

    db.query(&format!(
        "UPDATE {} SET user_group = $user_group, user_note = $user_note",
        record_id
    ))
    .bind(("user_group", user_group))
    .bind(("user_note", user_note))
    .await
    .map_err(|e| format!("Failed to update MCP server metadata: {}", e))?;

    Ok(())
}

/// Reorder MCP servers by updating sort_index for each server
pub async fn reorder_mcp_servers(state: &DbState, ids: &[String]) -> Result<(), String> {
    if let Some(sqlite_state) = global_sqlite_state() {
        sqlite_state.with_conn(|conn| {
            for (index, id) in ids.iter().enumerate() {
                if let Some(mut record) = db_get(conn, DbTable::McpServer, id)? {
                    if let Some(object) = record.as_object_mut() {
                        object.insert(
                            "sort_index".to_string(),
                            Value::Number(serde_json::Number::from(index as i64)),
                        );
                    }
                    db_put(conn, DbTable::McpServer, id, &record)?;
                }
            }
            Ok(())
        })?;
    }

    let db = state.db();

    for (index, id) in ids.iter().enumerate() {
        let record_id = db_record_id("mcp_server", id);
        db.query(&format!("UPDATE {} SET sort_index = $index", record_id))
            .bind(("index", index as i32))
            .await
            .map_err(|e| format!("Failed to reorder MCP servers: {}", e))?;
    }

    Ok(())
}

// ==================== Sync Details Operations ====================

/// Update sync detail for a specific tool
pub async fn update_sync_detail(
    state: &DbState,
    server_id: &str,
    detail: &McpSyncDetail,
) -> Result<(), String> {
    if global_sqlite_state().is_some() {
        let mut server = get_mcp_server_by_id(state, server_id)
            .await?
            .ok_or_else(|| format!("MCP server not found: {}", server_id))?;
        server.sync_details = Some(set_sync_detail(&server.sync_details, &detail.tool, detail));
        server.updated_at = now_ms();
        upsert_mcp_server(state, &server).await?;
        return Ok(());
    }

    let db = state.db();
    let record_id = db_record_id("mcp_server", server_id);

    // Get existing server
    let mut result = db
        .query(&format!(
            "SELECT *, type::string(id) as id FROM {} LIMIT 1",
            record_id
        ))
        .await
        .map_err(|e| e.to_string())?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;
    let server = records
        .first()
        .map(|r| from_db_mcp_server(r.clone()))
        .ok_or_else(|| format!("MCP server not found: {}", server_id))?;

    // Update sync_details
    let new_sync_details = set_sync_detail(&server.sync_details, &detail.tool, detail);

    // Save updates
    db.query(&format!(
        "UPDATE {} SET sync_details = $sync_details, updated_at = $updated_at",
        record_id
    ))
    .bind(("sync_details", new_sync_details))
    .bind(("updated_at", now_ms()))
    .await
    .map_err(|e| format!("Failed to update sync detail: {}", e))?;

    Ok(())
}

/// Remove sync detail for a specific tool
pub async fn delete_sync_detail(
    state: &DbState,
    server_id: &str,
    tool: &str,
) -> Result<(), String> {
    if global_sqlite_state().is_some() {
        let Some(mut server) = get_mcp_server_by_id(state, server_id).await? else {
            return Ok(());
        };
        server.sync_details = Some(remove_sync_detail(&server.sync_details, tool));
        server.updated_at = now_ms();
        upsert_mcp_server(state, &server).await?;
        return Ok(());
    }

    let db = state.db();
    let record_id = db_record_id("mcp_server", server_id);
    let tool_owned = tool.to_string();

    // Get existing server
    let mut result = db
        .query(&format!(
            "SELECT *, type::string(id) as id FROM {} LIMIT 1",
            record_id
        ))
        .await
        .map_err(|e| e.to_string())?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;
    let Some(server) = records.first().map(|r| from_db_mcp_server(r.clone())) else {
        return Ok(()); // Server not found, nothing to delete
    };

    // Update sync_details
    let new_sync_details = remove_sync_detail(&server.sync_details, &tool_owned);

    // Save updates
    db.query(&format!(
        "UPDATE {} SET sync_details = $sync_details, updated_at = $updated_at",
        record_id
    ))
    .bind(("sync_details", new_sync_details))
    .bind(("updated_at", now_ms()))
    .await
    .map_err(|e| format!("Failed to delete sync detail: {}", e))?;

    Ok(())
}

/// Toggle a tool's enabled state for an MCP server
pub async fn toggle_tool_enabled(
    state: &DbState,
    server_id: &str,
    tool_key: &str,
) -> Result<bool, String> {
    if global_sqlite_state().is_some() {
        let mut server = get_mcp_server_by_id(state, server_id)
            .await?
            .ok_or_else(|| format!("MCP server not found: {}", server_id))?;
        let mut enabled_tools = server.enabled_tools.clone();
        let is_now_enabled = if enabled_tools.contains(&tool_key.to_string()) {
            enabled_tools.retain(|tool| tool != tool_key);
            false
        } else {
            enabled_tools.push(tool_key.to_string());
            true
        };
        server.enabled_tools = enabled_tools;
        server.updated_at = now_ms();
        upsert_mcp_server(state, &server).await?;
        return Ok(is_now_enabled);
    }

    let db = state.db();
    let record_id = db_record_id("mcp_server", server_id);

    // Get existing server
    let mut result = db
        .query(&format!(
            "SELECT *, type::string(id) as id FROM {} LIMIT 1",
            record_id
        ))
        .await
        .map_err(|e| e.to_string())?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;
    let server = records
        .first()
        .map(|r| from_db_mcp_server(r.clone()))
        .ok_or_else(|| format!("MCP server not found: {}", server_id))?;

    // Toggle tool in enabled_tools
    let mut enabled_tools = server.enabled_tools.clone();
    let is_now_enabled = if enabled_tools.contains(&tool_key.to_string()) {
        enabled_tools.retain(|t| t != tool_key);
        false
    } else {
        enabled_tools.push(tool_key.to_string());
        true
    };

    // Save updates
    db.query(&format!(
        "UPDATE {} SET enabled_tools = $enabled_tools, updated_at = $updated_at",
        record_id
    ))
    .bind(("enabled_tools", enabled_tools))
    .bind(("updated_at", now_ms()))
    .await
    .map_err(|e| format!("Failed to toggle tool: {}", e))?;

    Ok(is_now_enabled)
}

// ==================== MCP Preferences ====================

/// Get MCP preferences (singleton record)
pub async fn get_mcp_preferences(state: &DbState) -> Result<McpPreferences, String> {
    if let Some(sqlite_state) = global_sqlite_state() {
        return sqlite_state.with_conn(|conn| {
            Ok(db_get(conn, DbTable::McpPreferences, "default")?
                .map(from_db_mcp_preferences)
                .unwrap_or_default())
        });
    }

    let db = state.db();

    let mut result = db
        .query("SELECT *, type::string(id) as id FROM mcp_preferences:`default` LIMIT 1")
        .await
        .map_err(|e| format!("Failed to query MCP preferences: {}", e))?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;

    if let Some(record) = records.first() {
        Ok(from_db_mcp_preferences(record.clone()))
    } else {
        Ok(McpPreferences::default())
    }
}

/// Save MCP preferences (singleton record)
pub async fn save_mcp_preferences(state: &DbState, prefs: &McpPreferences) -> Result<(), String> {
    if let Some(sqlite_state) = global_sqlite_state() {
        sqlite_state.with_conn(|conn| {
            db_put(
                conn,
                DbTable::McpPreferences,
                "default",
                &to_mcp_preferences_payload(prefs),
            )
        })?;
    }

    let db = state.db();
    let payload = to_mcp_preferences_payload(prefs);

    db.query("UPSERT mcp_preferences:`default` CONTENT $data")
        .bind(("data", payload))
        .await
        .map_err(|e| format!("Failed to save MCP preferences: {}", e))?;

    Ok(())
}

// ==================== Favorite MCP CRUD ====================

/// Get all favorite MCP servers
pub async fn get_favorite_mcps(state: &DbState) -> Result<Vec<FavoriteMcp>, String> {
    if let Some(sqlite_state) = global_sqlite_state() {
        return sqlite_state.with_conn(|conn| {
            let order = OrderSpec::new(vec![
                OrderField::json_integer("created_at", OrderDirection::Desc)?,
                OrderField::id(OrderDirection::Asc),
            ]);
            let records = db_list(conn, DbTable::FavoriteMcp, Some(&order))?;
            Ok(records.into_iter().map(from_db_favorite_mcp).collect())
        });
    }

    let db = state.db();

    let mut result = db
        .query("SELECT *, type::string(id) as id FROM favorite_mcp ORDER BY created_at DESC")
        .await
        .map_err(|e| format!("Failed to query favorite MCPs: {}", e))?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;
    Ok(records.into_iter().map(from_db_favorite_mcp).collect())
}

/// Get a favorite MCP by name
pub async fn get_favorite_mcp_by_name(
    state: &DbState,
    name: &str,
) -> Result<Option<FavoriteMcp>, String> {
    if let Some(sqlite_state) = global_sqlite_state() {
        return sqlite_state.with_conn(|conn| {
            let records = db_query_by_field(
                conn,
                DbTable::FavoriteMcp,
                &JsonFieldPath::new("name")?,
                &Value::String(name.to_string()),
                None,
                Some(1),
            )?;
            Ok(records.into_iter().next().map(from_db_favorite_mcp))
        });
    }

    let db = state.db();
    let name_owned = name.to_string();

    let mut result = db
        .query("SELECT *, type::string(id) as id FROM favorite_mcp WHERE name = $name LIMIT 1")
        .bind(("name", name_owned))
        .await
        .map_err(|e| format!("Failed to query favorite MCP by name: {}", e))?;

    let records: Vec<Value> = result.take(0).map_err(|e| e.to_string())?;
    Ok(records.first().map(|v| from_db_favorite_mcp(v.clone())))
}

/// Create or update a favorite MCP
pub async fn upsert_favorite_mcp(state: &DbState, fav: &FavoriteMcp) -> Result<String, String> {
    let sqlite_id = if let Some(sqlite_state) = global_sqlite_state() {
        let id = if fav.id.is_empty() {
            db_new_id()
        } else {
            fav.id.clone()
        };
        let mut payload = serde_json::to_value(fav).map_err(|e| e.to_string())?;
        if let Some(obj) = payload.as_object_mut() {
            obj.remove("id");
        }
        sqlite_state.with_conn(|conn| db_put(conn, DbTable::FavoriteMcp, &id, &payload))?;
        Some(id)
    } else {
        None
    };

    let db = state.db();

    // Remove id field for database payload
    let mut payload = serde_json::to_value(fav).map_err(|e| e.to_string())?;
    if let Some(obj) = payload.as_object_mut() {
        obj.remove("id");
    }

    if fav.id.is_empty() {
        // Create new
        let id = sqlite_id.unwrap_or_else(db_new_id);
        let record_id = db_record_id("favorite_mcp", &id);
        db.query(&format!("CREATE {} CONTENT $data", record_id))
            .bind(("data", payload))
            .await
            .map_err(|e| format!("Failed to create favorite MCP: {}", e))?;
        Ok(id)
    } else {
        // Update existing
        let record_id = db_record_id("favorite_mcp", &fav.id);
        db.query(&format!("UPDATE {} CONTENT $data", record_id))
            .bind(("data", payload))
            .await
            .map_err(|e| format!("Failed to update favorite MCP: {}", e))?;
        Ok(fav.id.clone())
    }
}

/// Delete a favorite MCP
pub async fn delete_favorite_mcp(state: &DbState, id: &str) -> Result<(), String> {
    if let Some(sqlite_state) = global_sqlite_state() {
        sqlite_state.with_conn(|conn| db_delete(conn, DbTable::FavoriteMcp, id).map(|_| ()))?;
    }

    let db = state.db();
    let record_id = db_record_id("favorite_mcp", id);

    db.query(&format!("DELETE {}", record_id))
        .await
        .map_err(|e| format!("Failed to delete favorite MCP: {}", e))?;

    Ok(())
}

pub async fn sync_sqlite_mcp_from_surreal_if_missing(
    sqlite_state: &SqliteDbState,
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<(), String> {
    let has_sqlite_mcp_data = sqlite_state.with_conn(|conn| {
        Ok(db_count(conn, DbTable::McpServer)? > 0
            || db_count(conn, DbTable::McpPreferences)? > 0
            || db_count(conn, DbTable::FavoriteMcp)? > 0)
    })?;
    if has_sqlite_mcp_data {
        return Ok(());
    }

    crate::db::surreal_import::import_tables_from_surreal(
        sqlite_state,
        db,
        &[
            DbTable::McpServer,
            DbTable::McpPreferences,
            DbTable::FavoriteMcp,
        ],
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::helpers::{db_get, db_list};
    use crate::db::sqlite_state::SqliteDbState;
    use serde_json::json;
    use surrealdb::engine::local::SurrealKv;

    #[tokio::test]
    async fn sync_sqlite_mcp_from_surreal_imports_all_mcp_tables() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let db = surrealdb::Surreal::new::<SurrealKv>(temp_dir.path().join("surreal"))
            .await
            .expect("open surreal");
        db.use_ns("ai_toolbox")
            .use_db("main")
            .await
            .expect("use ns db");
        db.query("UPSERT mcp_server:`server-a` CONTENT $data")
            .bind((
                "data",
                json!({
                    "name": "Server A",
                    "server_type": "stdio",
                    "server_config": {"command": "node"},
                    "enabled_tools": ["claude"],
                    "sort_index": 3,
                    "created_at": 1,
                    "updated_at": 2
                }),
            ))
            .await
            .expect("write server");
        db.query("UPSERT mcp_preferences:`default` CONTENT $data")
            .bind((
                "data",
                json!({
                    "show_in_tray": true,
                    "preferred_tools": ["claude"],
                    "favorites_initialized": true,
                    "sync_disabled_to_opencode": true,
                    "updated_at": 9
                }),
            ))
            .await
            .expect("write preferences");
        db.query("UPSERT favorite_mcp:`fav-a` CONTENT $data")
            .bind((
                "data",
                json!({
                    "name": "Favorite A",
                    "server_type": "http",
                    "server_config": {"url": "https://example.com/mcp"},
                    "created_at": 5,
                    "updated_at": 6
                }),
            ))
            .await
            .expect("write favorite");

        let sqlite_state = SqliteDbState::in_memory_for_test().expect("sqlite");
        sync_sqlite_mcp_from_surreal_if_missing(&sqlite_state, &db)
            .await
            .expect("sync mcp");

        let server = sqlite_state
            .with_conn(|conn| db_get(conn, DbTable::McpServer, "server-a"))
            .expect("read server")
            .expect("server exists");
        assert_eq!(from_db_mcp_server(server).name, "Server A");

        let prefs = sqlite_state
            .with_conn(|conn| db_get(conn, DbTable::McpPreferences, "default"))
            .expect("read preferences")
            .expect("preferences exist");
        assert!(from_db_mcp_preferences(prefs).show_in_tray);

        let favorites = sqlite_state
            .with_conn(|conn| db_list(conn, DbTable::FavoriteMcp, None))
            .expect("read favorites");
        assert_eq!(favorites.len(), 1);
        assert_eq!(
            from_db_favorite_mcp(favorites[0].clone()).name,
            "Favorite A"
        );
    }
}
