use ai_toolbox_lib::db::surreal_import::{
    cleanup_incomplete_sqlite_database, detect_startup_migration_state,
    mark_sqlite_import_complete, write_migration_log, write_migration_warning, MigrationPaths,
    StartupMigrationState, LEGACY_DATABASE_DIR, SQLITE_DATABASE_FILE,
};

fn paths(temp_dir: &tempfile::TempDir) -> MigrationPaths {
    MigrationPaths::new(temp_dir.path())
}

#[test]
fn detects_new_install_when_no_database_exists() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let paths = paths(&temp_dir);

    assert_eq!(
        detect_startup_migration_state(&paths),
        StartupMigrationState::NewInstall
    );
}

#[test]
fn detects_initial_surreal_import_when_only_legacy_database_exists() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let paths = paths(&temp_dir);
    std::fs::create_dir(paths.app_data_dir.join(LEGACY_DATABASE_DIR)).expect("legacy dir");

    assert_eq!(
        detect_startup_migration_state(&paths),
        StartupMigrationState::NeedsSurrealImport
    );
}

#[test]
fn detects_incomplete_import_when_legacy_and_sqlite_exist_without_flag() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let paths = paths(&temp_dir);
    std::fs::create_dir(&paths.legacy_database_dir).expect("legacy dir");
    std::fs::write(&paths.sqlite_database_file, b"partial").expect("sqlite file");

    assert_eq!(
        detect_startup_migration_state(&paths),
        StartupMigrationState::IncompleteImport
    );
}

#[test]
fn detects_legacy_archive_step_when_complete_flag_exists() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let paths = paths(&temp_dir);
    std::fs::create_dir(&paths.legacy_database_dir).expect("legacy dir");
    std::fs::write(&paths.sqlite_database_file, b"sqlite").expect("sqlite file");
    mark_sqlite_import_complete(&paths).expect("complete flag");

    assert_eq!(
        detect_startup_migration_state(&paths),
        StartupMigrationState::NeedsLegacyArchive
    );
}

#[test]
fn detects_ready_when_only_sqlite_database_exists() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let paths = paths(&temp_dir);
    std::fs::write(paths.app_data_dir.join(SQLITE_DATABASE_FILE), b"sqlite").expect("sqlite file");

    assert_eq!(
        detect_startup_migration_state(&paths),
        StartupMigrationState::Ready
    );
}

#[test]
fn cleanup_incomplete_sqlite_database_removes_db_wal_shm_and_flag() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let paths = paths(&temp_dir);

    std::fs::write(&paths.sqlite_database_file, b"db").expect("db");
    std::fs::write(&paths.sqlite_wal_file, b"wal").expect("wal");
    std::fs::write(&paths.sqlite_shm_file, b"shm").expect("shm");
    mark_sqlite_import_complete(&paths).expect("flag");

    cleanup_incomplete_sqlite_database(&paths).expect("cleanup");

    assert!(!paths.sqlite_database_file.exists());
    assert!(!paths.sqlite_wal_file.exists());
    assert!(!paths.sqlite_shm_file.exists());
    assert!(!paths.complete_flag.exists());
}

#[test]
fn migration_log_and_warning_are_written_to_files() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let paths = paths(&temp_dir);

    write_migration_log(&paths, "started").expect("write log");
    write_migration_log(&paths, "finished").expect("append log");
    write_migration_warning(&paths, "unknown empty table").expect("write warning");

    let log = std::fs::read_to_string(&paths.migration_log).expect("read log");
    let warning = std::fs::read_to_string(&paths.migration_warnings).expect("read warning");

    assert!(log.contains("started"));
    assert!(log.contains("finished"));
    assert!(warning.contains("unknown empty table"));
}
