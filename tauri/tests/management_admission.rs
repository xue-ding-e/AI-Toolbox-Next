//! The desktop library linked by an integration test is compiled without
//! cfg(test). These public entry points therefore cannot use the legacy unit
//! helper's absent-registry compatibility branch.
use ai_toolbox_lib::coding::codex::history_sync;
use std::fs;

#[test]
fn production_library_missing_session_rejects_before_runtime_or_backup_effects() {
    let fixture = tempfile::tempdir().unwrap();
    let existing = fixture.path().join("existing-synthetic-runtime");
    fs::create_dir(&existing).unwrap();
    let sentinel = existing.join("synthetic-sentinel");
    fs::write(&sentinel, b"synthetic unchanged bytes").unwrap();
    let absent = fixture.path().join("absent-synthetic-runtime");
    for root in [&existing, &absent] {
        let backup = history_sync::backup(root, "synthetic-only");
        assert!(matches!(backup, Err(error) if error == "The GUI profile owner is unavailable"));
        let sync = history_sync::sync(root);
        assert!(matches!(sync, Err(error) if error == "The GUI profile owner is unavailable"));
        let restore = history_sync::restore_latest(root);
        assert!(matches!(restore, Err(error) if error == "The GUI profile owner is unavailable"));
        assert!(!absent.exists());
        assert_eq!(fs::read(&sentinel).unwrap(), b"synthetic unchanged bytes");
        let entries: Vec<_> = fs::read_dir(&existing).unwrap()
            .map(|entry| entry.unwrap().file_name()).collect();
        assert_eq!(entries, [std::ffi::OsString::from("synthetic-sentinel")]);
    }
}
