use rusqlite::Connection;

use super::schema::{sql_string_literal, DbTable, JsonFieldPath, ALL_TABLES};

pub const TARGET_SCHEMA_VERSION: i32 = 2;

pub fn run_all(conn: &mut Connection) -> Result<(), String> {
    let current_version = get_user_version(conn)?;
    if current_version > TARGET_SCHEMA_VERSION {
        return Err(format!(
            "SQLite schema version {} is newer than supported version {}",
            current_version, TARGET_SCHEMA_VERSION
        ));
    }

    if current_version < 1 {
        run_migration_step(conn, 1, migrate_v1)?;
    }
    if current_version < 2 {
        run_migration_step(conn, 2, migrate_v2)?;
    }

    Ok(())
}

pub fn get_user_version(conn: &Connection) -> Result<i32, String> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| format!("Failed to read SQLite user_version: {error}"))
}

pub fn set_user_version(conn: &Connection, version: i32) -> Result<(), String> {
    conn.execute_batch(&format!("PRAGMA user_version = {version}"))
        .map_err(|error| format!("Failed to set SQLite user_version to {version}: {error}"))
}

fn run_migration_step(
    conn: &Connection,
    target_version: i32,
    migration: fn(&Connection) -> Result<(), String>,
) -> Result<(), String> {
    conn.execute_batch("SAVEPOINT ai_toolbox_schema_migration")
        .map_err(|error| format!("Failed to start schema migration savepoint: {error}"))?;

    let result = (|| {
        migration(conn)?;
        set_user_version(conn, target_version)?;
        Ok(())
    })();

    match result {
        Ok(()) => conn
            .execute_batch("RELEASE ai_toolbox_schema_migration")
            .map_err(|error| format!("Failed to release schema migration savepoint: {error}")),
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO ai_toolbox_schema_migration; RELEASE ai_toolbox_schema_migration",
            );
            Err(error)
        }
    }
}

fn migrate_v1(conn: &Connection) -> Result<(), String> {
    for table in ALL_TABLES {
        create_jsonb_table(conn, *table)?;
    }

    create_initial_indexes(conn)?;
    Ok(())
}

fn migrate_v2(conn: &Connection) -> Result<(), String> {
    create_proxy_gateway_usage_tables(conn)
}

fn create_jsonb_table(conn: &Connection, table: DbTable) -> Result<(), String> {
    let table_name = table.name();
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {table_name} (
            id TEXT PRIMARY KEY NOT NULL,
            data BLOB NOT NULL CHECK (json_valid(data, 4)),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );"
    ))
    .map_err(|error| format!("Failed to create SQLite table {table_name}: {error}"))
}

fn create_initial_indexes(conn: &Connection) -> Result<(), String> {
    for table in [
        DbTable::ClaudeProvider,
        DbTable::CodexProvider,
        DbTable::GeminiCliProvider,
        DbTable::ClaudePromptConfig,
        DbTable::CodexPromptConfig,
        DbTable::GeminiCliPromptConfig,
        DbTable::OpenCodePromptConfig,
        DbTable::OhMyOpenAgentConfig,
        DbTable::OhMyOpenCodeSlimConfig,
        DbTable::CodexOfficialAccount,
        DbTable::GeminiCliOfficialAccount,
    ] {
        create_json_index(conn, table, &JsonFieldPath::new("is_applied")?)?;
    }

    for table in [
        DbTable::ClaudeProvider,
        DbTable::CodexProvider,
        DbTable::GeminiCliProvider,
        DbTable::ClaudePromptConfig,
        DbTable::CodexPromptConfig,
        DbTable::GeminiCliPromptConfig,
        DbTable::OpenCodePromptConfig,
        DbTable::Skill,
        DbTable::SkillGroup,
        DbTable::McpServer,
        DbTable::OhMyOpenAgentConfig,
        DbTable::OhMyOpenCodeSlimConfig,
        DbTable::CodexOfficialAccount,
        DbTable::GeminiCliOfficialAccount,
    ] {
        create_json_index(conn, table, &JsonFieldPath::new("sort_index")?)?;
    }

    for table in [DbTable::ImageChannel, DbTable::SshConnection] {
        create_json_index(conn, table, &JsonFieldPath::new("sort_order")?)?;
    }

    for table in [
        DbTable::OpenCodeFavoritePlugin,
        DbTable::OpenCodeFavoriteProvider,
        DbTable::FavoriteMcp,
        DbTable::ImageJob,
        DbTable::ImageAsset,
    ] {
        create_column_index(conn, table, "created_at")?;
    }

    for (table, field) in [
        (DbTable::Skill, "name"),
        (DbTable::McpServer, "name"),
        (DbTable::FavoriteMcp, "name"),
        (DbTable::OpenCodeFavoritePlugin, "plugin_name"),
        (DbTable::OpenCodeFavoriteProvider, "provider_id"),
        (DbTable::CodexOfficialAccount, "provider_id"),
        (DbTable::GeminiCliOfficialAccount, "provider_id"),
        (DbTable::ImageAsset, "job_id"),
    ] {
        create_json_index(conn, table, &JsonFieldPath::new(field)?)?;
    }

    Ok(())
}

fn create_proxy_gateway_usage_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS proxy_request_logs (
            request_id TEXT PRIMARY KEY NOT NULL,
            provider_id TEXT NOT NULL,
            app_type TEXT NOT NULL,
            model TEXT NOT NULL,
            request_model TEXT,
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cache_read_tokens INTEGER NOT NULL DEFAULT 0,
            cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
            input_cost_usd TEXT NOT NULL DEFAULT '0',
            output_cost_usd TEXT NOT NULL DEFAULT '0',
            cache_read_cost_usd TEXT NOT NULL DEFAULT '0',
            cache_creation_cost_usd TEXT NOT NULL DEFAULT '0',
            total_cost_usd TEXT NOT NULL DEFAULT '0',
            latency_ms INTEGER NOT NULL DEFAULT 0,
            first_token_ms INTEGER,
            duration_ms INTEGER,
            status_code INTEGER NOT NULL DEFAULT 0,
            error_message TEXT,
            session_id TEXT,
            provider_type TEXT,
            is_streaming INTEGER NOT NULL DEFAULT 0,
            cost_multiplier TEXT NOT NULL DEFAULT '1.0',
            created_at INTEGER NOT NULL,
            data_source TEXT NOT NULL DEFAULT 'proxy',
            route_name TEXT,
            path TEXT,
            request_body_bytes INTEGER NOT NULL DEFAULT 0,
            response_body_bytes INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_request_logs_provider
            ON proxy_request_logs(provider_id, app_type);
        CREATE INDEX IF NOT EXISTS idx_request_logs_created_at
            ON proxy_request_logs(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_request_logs_model
            ON proxy_request_logs(model);
        CREATE INDEX IF NOT EXISTS idx_request_logs_session
            ON proxy_request_logs(session_id);
        CREATE INDEX IF NOT EXISTS idx_request_logs_status
            ON proxy_request_logs(status_code);
        CREATE INDEX IF NOT EXISTS idx_request_logs_app_created_at
            ON proxy_request_logs(app_type, created_at DESC);

        CREATE TABLE IF NOT EXISTS usage_daily_rollups (
            date TEXT NOT NULL,
            app_type TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            model TEXT NOT NULL,
            request_count INTEGER NOT NULL DEFAULT 0,
            success_count INTEGER NOT NULL DEFAULT 0,
            input_tokens INTEGER NOT NULL DEFAULT 0,
            output_tokens INTEGER NOT NULL DEFAULT 0,
            cache_read_tokens INTEGER NOT NULL DEFAULT 0,
            cache_creation_tokens INTEGER NOT NULL DEFAULT 0,
            total_cost_usd TEXT NOT NULL DEFAULT '0',
            avg_latency_ms INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (date, app_type, provider_id, model)
        );

        CREATE INDEX IF NOT EXISTS idx_usage_rollups_app_date
            ON usage_daily_rollups(app_type, date);
        CREATE INDEX IF NOT EXISTS idx_usage_rollups_provider
            ON usage_daily_rollups(provider_id, app_type);",
    )
    .map_err(|error| format!("Failed to create proxy gateway usage tables: {error}"))
}

fn create_json_index(
    conn: &Connection,
    table: DbTable,
    field_path: &JsonFieldPath,
) -> Result<(), String> {
    let table_name = table.name();
    let field_suffix = field_path.segments().join("_");
    let index_name = format!("idx_{table_name}_{field_suffix}");
    let json_path = sql_string_literal(&field_path.to_sql_path());
    conn.execute_batch(&format!(
        "CREATE INDEX IF NOT EXISTS {index_name}
         ON {table_name} (json_extract(data, {json_path}));"
    ))
    .map_err(|error| format!("Failed to create SQLite index {index_name}: {error}"))
}

fn create_column_index(conn: &Connection, table: DbTable, column: &str) -> Result<(), String> {
    let table_name = table.name();
    let index_name = format!("idx_{table_name}_{column}");
    conn.execute_batch(&format!(
        "CREATE INDEX IF NOT EXISTS {index_name} ON {table_name} ({column});"
    ))
    .map_err(|error| format!("Failed to create SQLite index {index_name}: {error}"))
}
