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
            assert!(
                count > 0,
                "standalone initialization must not omit pricing seed"
            );
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
            assert_eq!(
                migrations::get_user_version(conn)?,
                migrations::TARGET_SCHEMA_VERSION
            );
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
    let snapshot_bytes = std::fs::read(&snapshots[0]).unwrap();
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
    let entries_after_snapshot_read: Vec<_> =
        std::fs::read_dir(dir.path().join(SQLITE_MIGRATION_BACKUP_DIR))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
    eprintln!("backup directory immediately after read-only snapshot queries: {entries_after_snapshot_read:?}");
    state.with_conn_mut(migrations::run_all).unwrap();
    drop(state);
    let reopened = SqliteDbState::open(path).unwrap();
    reopened
        .with_conn(|conn| {
            assert_eq!(db_count(conn, DbTable::CodexContextOperation)?, 0);
            Ok(())
        })
        .unwrap();
    // Reading a WAL-mode backup through SQLITE_OPEN_READ_ONLY may create
    // its -wal/-shm companions. These are not additional migration backups.
    let mut entries: Vec<_> = std::fs::read_dir(dir.path().join(SQLITE_MIGRATION_BACKUP_DIR))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    let backup_name = snapshots[0].file_name().unwrap().to_str().unwrap();
    let allowed = [
        backup_name.to_owned(),
        format!("{backup_name}-wal"),
        format!("{backup_name}-shm"),
    ];
    for entry in &entries {
        assert!(entry.is_file());
        assert!(
            allowed
                .iter()
                .any(|name| entry.file_name().unwrap() == name.as_str()),
            "unexpected backup-directory entry: {:?}",
            entry.file_name()
        );
    }
    let database_snapshots: Vec<_> = entries
        .iter()
        .filter(|entry| entry.extension().is_some_and(|extension| extension == "db"))
        .cloned()
        .collect();
    assert_eq!(database_snapshots, snapshots);
    assert_eq!(std::fs::read(&snapshots[0]).unwrap(), snapshot_bytes);
    eprintln!(
        "backup directory after reopen: {:?}",
        entries
            .iter()
            .map(|entry| entry.file_name().unwrap())
            .collect::<Vec<_>>()
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

fn migration_snapshots(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut paths: Vec<_> = std::fs::read_dir(root.join(SQLITE_MIGRATION_BACKUP_DIR))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "db"))
        .collect();
    paths.sort();
    paths
}

fn readonly_database(path: &std::path::Path) -> rusqlite::Connection {
    rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap()
}

#[test]
fn v30_file_without_journal_upgrades_with_original_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v30-missing-journal.db");
    make_v29_file(&path);
    let conn = rusqlite::Connection::open(&path).unwrap();
    migrations::set_user_version(&conn, 30).unwrap();
    let original = db_get(&conn, DbTable::CodexCommonConfig, "common").unwrap();
    assert!(db_count(&conn, DbTable::CodexContextOperation).is_err());
    drop(conn);

    let state = SqliteDbState::open(path.clone()).unwrap();
    state
        .with_conn(|conn| {
            assert_eq!(migrations::get_user_version(conn)?, migrations::TARGET_SCHEMA_VERSION);
            assert_eq!(db_count(conn, DbTable::CodexContextOperation)?, 0);
            assert_eq!(db_get(conn, DbTable::CodexCommonConfig, "common")?, original);
            Ok(())
        })
        .unwrap();
    drop(state);
    let snapshots = migration_snapshots(dir.path());
    assert_eq!(snapshots.len(), 1);
    let bytes = std::fs::read(&snapshots[0]).unwrap();
    {
        let snapshot = readonly_database(&snapshots[0]);
        assert_eq!(migrations::get_user_version(&snapshot).unwrap(), 30);
        assert!(db_count(&snapshot, DbTable::CodexContextOperation).is_err());
        assert_eq!(db_get(&snapshot, DbTable::CodexCommonConfig, "common").unwrap(), original);
    }
    let reopened = SqliteDbState::open(path).unwrap();
    reopened.with_conn(|conn| {
        assert_eq!(db_count(conn, DbTable::CodexContextOperation)?, 0);
        Ok(())
    }).unwrap();
    drop(reopened);
    assert_eq!(migration_snapshots(dir.path()), snapshots);
    assert_eq!(std::fs::read(&snapshots[0]).unwrap(), bytes);
}

fn raw_journal_row(conn: &rusqlite::Connection) -> (String, Vec<u8>, String, String) {
    conn.query_row(
        "SELECT id, data, created_at, updated_at FROM codex_context_operation",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .unwrap()
}

#[test]
fn v30_file_with_journal_preserves_row_bytes_and_snapshot_on_reopen() {
    use ai_toolbox_storage::codex_context_journal::{insert_saved, JournalInput};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v30-existing-journal.db");
    let state = SqliteDbState::open(path.clone()).unwrap();
    let original = state.with_conn_mut(|conn| {
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        insert_saved(&tx, JournalInput {
            request_id: "synthetic-schema-upgrade".to_string(),
            provider_id: "synthetic-provider".to_string(),
            request_fingerprint: "a".repeat(64),
            expected_revision: "b".repeat(64),
            planned_live_digest: "c".repeat(64),
            context: Some(1_000_000),
            auto_compact: Some(900_000),
            master_changed: true,
            live_changed: true,
        }).map_err(|error| error.to_string())?;
        tx.commit().map_err(|error| error.to_string())?;
        migrations::set_user_version(conn, 30)?;
        Ok(raw_journal_row(conn))
    }).unwrap();
    drop(state);

    let mut first_snapshot = None;
    for _ in 0..2 {
        let state = SqliteDbState::open(path.clone()).unwrap();
        state.with_conn(|conn| {
            assert_eq!(migrations::get_user_version(conn)?, migrations::TARGET_SCHEMA_VERSION);
            assert_eq!(db_count(conn, DbTable::CodexContextOperation)?, 1);
            assert_eq!(raw_journal_row(conn), original);
            Ok(())
        }).unwrap();
        drop(state);
        let snapshots = migration_snapshots(dir.path());
        assert_eq!(snapshots.len(), 1);
        let current = (snapshots[0].clone(), std::fs::read(&snapshots[0]).unwrap());
        if let Some(expected) = &first_snapshot {
            assert_eq!(&current, expected);
        } else {
            first_snapshot = Some(current);
        }
    }
    let snapshots = migration_snapshots(dir.path());
    assert_eq!(snapshots.len(), 1);
    let snapshot = readonly_database(&snapshots[0]);
    assert_eq!(migrations::get_user_version(&snapshot).unwrap(), 30);
    assert_eq!(raw_journal_row(&snapshot), original);
}

fn assert_upstream_native_tools_fixture(conn: &rusqlite::Connection) {
    let native_tools: String = conn.query_row(
        "SELECT native_tools_used FROM proxy_request_logs WHERE request_id = 'synthetic-native-tools'",
        [], |row| row.get(0),
    ).unwrap();
    assert_eq!(native_tools, r#"["web_search"]"#);
    let index_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_request_logs_proxy_envelope'",
        [], |row| row.get(0),
    ).unwrap();
    assert_eq!(index_count, 1);
}

#[test]
fn upstream_v31_file_admission_requires_journal_and_prechange_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("upstream-v31.db");
    make_v29_file(&path);
    let conn = rusqlite::Connection::open(&path).unwrap();
    // Synthetic shared v29 schema plus the two upstream additions, not user data.
    // Upstream 0ca25b588d5dc177c0c3e4756160495ee420a1b1 has schema v31:
    // v30 creates the proxy-envelope index; v31 adds native_tools_used. Neither
    // creates Next's context journal. Equal user_version is not equal capability.
    conn.execute_batch(
        r#"CREATE INDEX IF NOT EXISTS idx_request_logs_proxy_envelope
         ON proxy_request_logs (
             app_type,
             substr(request_id, length('SESSION:' || app_type || ':' || provider_id || ':') + 1)
         )
         WHERE COALESCE(data_source, 'proxy') = 'proxy' AND request_kind = 'request';
         ALTER TABLE proxy_request_logs ADD COLUMN native_tools_used TEXT;
         INSERT INTO proxy_request_logs
             (request_id, provider_id, app_type, model, created_at, native_tools_used)
         VALUES ('synthetic-native-tools', 'synthetic-provider', 'codex',
                 'synthetic-model', 1, '["web_search"]');"#,
    ).unwrap();
    migrations::set_user_version(&conn, 31).unwrap();
    let original = db_get(&conn, DbTable::CodexCommonConfig, "common").unwrap();
    assert!(db_count(&conn, DbTable::CodexContextOperation).is_err());
    health::quick_check(&conn).unwrap();
    drop(conn);

    // Admission must provide the storage capability promised by a successful
    // open. quick_check alone cannot detect the absent application table.
    let state = SqliteDbState::open(path.clone()).unwrap();
    state.with_conn(|conn| {
        assert_eq!(db_count(conn, DbTable::CodexContextOperation)?, 0);
        assert_eq!(db_get(conn, DbTable::CodexCommonConfig, "common")?, original);
        assert_upstream_native_tools_fixture(conn);
        Ok(())
    }).unwrap();
    drop(state);
    let snapshots = migration_snapshots(dir.path());
    assert_eq!(snapshots.len(), 1);
    let bytes = std::fs::read(&snapshots[0]).unwrap();
    {
        let snapshot = readonly_database(&snapshots[0]);
        assert_eq!(migrations::get_user_version(&snapshot).unwrap(), 31);
        assert!(db_count(&snapshot, DbTable::CodexContextOperation).is_err());
        assert_eq!(db_get(&snapshot, DbTable::CodexCommonConfig, "common").unwrap(), original);
        assert_upstream_native_tools_fixture(&snapshot);
    }
    let reopened = SqliteDbState::open(path).unwrap();
    reopened.with_conn(|conn| {
        assert_eq!(db_count(conn, DbTable::CodexContextOperation)?, 0);
        assert_upstream_native_tools_fixture(conn);
        Ok(())
    }).unwrap();
    drop(reopened);
    assert_eq!(migration_snapshots(dir.path()), snapshots);
    assert_eq!(std::fs::read(&snapshots[0]).unwrap(), bytes);
}
