use rusqlite::Connection;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonbSupport {
    pub sqlite_version: String,
    pub jsonb_type: String,
    pub jsonb_valid: i64,
}

pub fn verify_jsonb_support(conn: &Connection) -> Result<JsonbSupport, String> {
    let support = conn
        .query_row(
            "SELECT sqlite_version(), typeof(jsonb('{}')), json_valid(jsonb('{}'), 4)",
            [],
            |row| {
                Ok(JsonbSupport {
                    sqlite_version: row.get(0)?,
                    jsonb_type: row.get(1)?,
                    jsonb_valid: row.get(2)?,
                })
            },
        )
        .map_err(|error| format!("SQLite JSONB support probe failed: {error}"))?;

    if support.jsonb_type != "blob" || support.jsonb_valid != 1 {
        return Err(format!(
            "SQLite JSONB support probe returned invalid result: version={}, type={}, valid={}",
            support.sqlite_version, support.jsonb_type, support.jsonb_valid
        ));
    }

    Ok(support)
}

pub fn quick_check(conn: &Connection) -> Result<(), String> {
    let result: String = conn
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|error| format!("Failed to run SQLite quick_check: {error}"))?;

    if result == "ok" {
        Ok(())
    } else {
        Err(format!("SQLite quick_check failed: {result}"))
    }
}

pub fn checkpoint_truncate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|error| format!("Failed to checkpoint SQLite WAL: {error}"))
}
