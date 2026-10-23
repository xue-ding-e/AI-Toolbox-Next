use std::path::Path;

use rusqlite::Connection;

use super::health;

pub fn backup_to_path(conn: &Connection, backup_path: &Path) -> Result<(), String> {
    health::checkpoint_truncate(conn)?;
    conn.backup(rusqlite::MAIN_DB, backup_path, None)
        .map_err(|error| {
            format!(
                "Failed to create SQLite backup {}: {error}",
                backup_path.display()
            )
        })
}

pub fn vacuum(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("VACUUM;")
        .map_err(|error| format!("Failed to vacuum SQLite database: {error}"))
}
