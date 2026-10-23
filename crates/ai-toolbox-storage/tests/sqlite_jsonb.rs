use ai_toolbox_storage::{health, migrations, model_pricing_seed, SqliteDbState};

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
