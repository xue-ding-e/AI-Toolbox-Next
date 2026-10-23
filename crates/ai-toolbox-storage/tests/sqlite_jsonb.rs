use ai_toolbox_storage::helpers::{db_count, db_get, db_put};
use ai_toolbox_storage::schema::{DbTable, ALL_TABLES};
use ai_toolbox_storage::sqlite_state::SQLITE_MIGRATION_BACKUP_DIR;
use ai_toolbox_storage::{health, migrations, model_pricing_seed, SqliteDbState};
use serde_json::json;

#[test]
fn standalone_state_initializes_schema_jsonb_and_bundled_pricing() {
    let state = SqliteDbState::in_memory_for_test().expect("initialize shared storage");
    state
        .with_conn(|conn| {
            assert_eq!(
                migrations::get_user_version(conn)?,
                migrations::TARGET_SCHEMA_VERSION
            );
            assert_eq!(health::verify_jsonb_support(conn)?.jsonb_type, "blob");
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM model_pricing", [], |row| row.get(0))
                .map_err(|error| error.to_string())?;
            assert!(count > 0, "standalone initialization must not omit pricing seed");
            assert_eq!(model_pricing_seed::ensure_seeded_from_bundled(conn)?, 0);
            assert!(ALL_TABLES.contains(&DbTable::CodexContextOperation));
            assert_eq!(db_count(conn, DbTable::CodexContextOperation)?, 0);
            health::quick_check(conn)
        })
        .unwrap();
}

#[test]
fn standalone_file_open_rejects_future_schema_without_modifying_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("future.db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    migrations::set_user_version(&conn, migrations::TARGET_SCHEMA_VERSION + 1).unwrap();
    drop(conn);
    let before = std::fs::read(&path).unwrap();
    let error = SqliteDbState::open(path.clone())
        .err()
        .expect("reject future schema");
    assert!(migrations::is_future_schema_error(&error));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

fn make_v29_file(path: &std::path::Path) {
    let state = SqliteDbState::open(path.to_path_buf()).unwrap();
    state
        .with_conn(|conn| {
            // The sole v30 schema addition is this table. Remove it to model a
            // real v29 database that has never seen the journal migration.
            conn.execute_batch("DROP TABLE codex_context_operation")
                .map_err(|error| error.to_string())?;
            migrations::set_user_version(conn, 29)?;
            db_put(
                conn,
                DbTable::CodexCommonConfig,
                "common",
                &json!({
                    "config": "model_context_window=500000\n",
                    "root_dir": "synthetic-root",
                    "unknown": {"retained": true}
                }),
            )
        })
        .unwrap();
}

#[test]
fn v29_file_upgrade_creates_journal_and_preserves_original_snapshot_and_records() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v29.db");
    make_v29_file(&path);
    let original = {
        let conn = rusqlite::Connection::open(&path).unwrap();
        assert_eq!(migrations::get_user_version(&conn).unwrap(), 29);
        assert!(db_count(&conn, DbTable::CodexContextOperation).is_err());
        db_get(&conn, DbTable::CodexCommonConfig, "common")
            .unwrap()
            .unwrap()
    };
    let state = SqliteDbState::open(path.clone()).unwrap();
    state
        .with_conn(|conn| {
            assert_eq!(migrations::get_user_version(conn)?, 30);
            assert_eq!(db_count(conn, DbTable::CodexContextOperation)?, 0);
            assert_eq!(
                db_get(conn, DbTable::CodexCommonConfig, "common")?,
                Some(original.clone())
            );
            Ok(())
        })
        .unwrap();
    let snapshots: Vec<_> = std::fs::read_dir(dir.path().join(SQLITE_MIGRATION_BACKUP_DIR))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(snapshots.len(), 1);
    let snapshot = rusqlite::Connection::open_with_flags(
        &snapshots[0],
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    assert_eq!(migrations::get_user_version(&snapshot).unwrap(), 29);
    assert!(db_count(&snapshot, DbTable::CodexContextOperation).is_err());
    assert_eq!(
        db_get(&snapshot, DbTable::CodexCommonConfig, "common").unwrap(),
        Some(original)
    );
    state.with_conn_mut(migrations::run_all).unwrap();
    drop(state);
    let reopened = SqliteDbState::open(path).unwrap();
    reopened
        .with_conn(|conn| {
            assert_eq!(db_count(conn, DbTable::CodexContextOperation)?, 0);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        std::fs::read_dir(dir.path().join(SQLITE_MIGRATION_BACKUP_DIR))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn v29_upgrade_snapshot_failure_blocks_schema_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v29.db");
    make_v29_file(&path);
    // An occupied non-directory path deterministically prevents a backup on
    // every platform, unlike permission bits when tests run as root.
    std::fs::write(dir.path().join(SQLITE_MIGRATION_BACKUP_DIR), b"blocked").unwrap();
    assert!(SqliteDbState::open(path.clone()).is_err());
    let conn = rusqlite::Connection::open(path).unwrap();
    assert_eq!(migrations::get_user_version(&conn).unwrap(), 29);
    assert!(db_count(&conn, DbTable::CodexContextOperation).is_err());
    assert_eq!(
        db_get(&conn, DbTable::CodexCommonConfig, "common")
            .unwrap()
            .unwrap()["unknown"],
        json!({"retained": true})
    );
}
