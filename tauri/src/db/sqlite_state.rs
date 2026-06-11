use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use rusqlite::Connection;
use uuid::Uuid;

use super::{backup, health, migrations, model_pricing_seed};

pub const SQLITE_MIGRATION_BACKUP_DIR: &str = "sqlite-migration-backups";

#[derive(Clone)]
pub struct SqliteDbState {
    conn: Arc<Mutex<Connection>>,
    db_path: PathBuf,
}

impl SqliteDbState {
    pub fn open(db_path: PathBuf) -> Result<Self, String> {
        let mut conn = Connection::open(&db_path).map_err(|error| {
            format!(
                "Failed to open SQLite database {}: {error}",
                db_path.display()
            )
        })?;
        initialize_file_connection(&mut conn, &db_path)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path,
        })
    }

    pub fn in_memory_for_test() -> Result<Self, String> {
        let mut conn = Connection::open_in_memory()
            .map_err(|error| format!("Failed to open in-memory SQLite database: {error}"))?;
        initialize_connection(&mut conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: PathBuf::from(":memory:"),
        })
    }

    pub fn with_conn<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| "SQLite connection mutex is poisoned".to_string())?;
        operation(&conn)
    }

    pub fn with_conn_mut<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| "SQLite connection mutex is poisoned".to_string())?;
        operation(&mut conn)
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn db(&self) -> &Self {
        self
    }
}

pub fn initialize_connection(conn: &mut Connection) -> Result<(), String> {
    initialize_connection_inner(conn, None)
}

fn initialize_file_connection(conn: &mut Connection, db_path: &Path) -> Result<(), String> {
    initialize_connection_inner(conn, Some(db_path))
}

fn initialize_connection_inner(
    conn: &mut Connection,
    db_path_for_migration_backup: Option<&Path>,
) -> Result<(), String> {
    let current_version = migrations::ensure_supported_user_version(conn)?;

    conn.busy_timeout(std::time::Duration::from_millis(5000))
        .map_err(|error| format!("Failed to set SQLite busy timeout: {error}"))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA cache_size = -8000;",
    )
    .map_err(|error| format!("Failed to initialize SQLite PRAGMA settings: {error}"))?;

    health::verify_jsonb_support(conn)?;
    if let Some(db_path) = db_path_for_migration_backup {
        create_pre_migration_backup_if_needed(conn, db_path, current_version)?;
    }
    migrations::run_all(conn)?;
    let inserted_pricing_count = model_pricing_seed::ensure_seeded(conn)?;
    if inserted_pricing_count > 0 {
        log::info!(
            "[ModelPricing] Seeded {} missing pricing rows",
            inserted_pricing_count
        );
    }
    health::quick_check(conn)?;
    Ok(())
}

fn create_pre_migration_backup_if_needed(
    conn: &Connection,
    db_path: &Path,
    current_version: i32,
) -> Result<Option<PathBuf>, String> {
    if current_version <= 0 || current_version >= migrations::TARGET_SCHEMA_VERSION {
        return Ok(None);
    }

    let db_parent = db_path.parent().ok_or_else(|| {
        format!(
            "Failed to resolve SQLite database parent directory for {}",
            db_path.display()
        )
    })?;
    let backup_dir = db_parent.join(SQLITE_MIGRATION_BACKUP_DIR);
    fs::create_dir_all(&backup_dir).map_err(|error| {
        format!(
            "Failed to create SQLite migration backup directory {}: {error}",
            backup_dir.display()
        )
    })?;

    let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
    let backup_path = backup_dir.join(format!(
        "ai-toolbox-schema-v{}-to-v{}-{}-{}.db",
        current_version,
        migrations::TARGET_SCHEMA_VERSION,
        timestamp,
        Uuid::new_v4().simple()
    ));

    backup::backup_to_path(conn, &backup_path).map_err(|error| {
        format!(
            "Failed to create pre-migration SQLite backup before upgrading schema v{} to v{}: {}",
            current_version,
            migrations::TARGET_SCHEMA_VERSION,
            error
        )
    })?;
    log::info!(
        "Created pre-migration SQLite backup before schema v{} -> v{}: {}",
        current_version,
        migrations::TARGET_SCHEMA_VERSION,
        backup_path.display()
    );

    Ok(Some(backup_path))
}
