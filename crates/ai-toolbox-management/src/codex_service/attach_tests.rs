//! Synthetic file-database attachment tests; no application or user profile.

use super::*;
use crate::owner::PROFILE_LEASE_FILE;
use ai_toolbox_storage::{migrations::TARGET_SCHEMA_VERSION, sqlite_state::SQLITE_MIGRATION_BACKUP_DIR};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    os::unix::fs::{symlink, PermissionsExt},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

struct Profile {
    root: tempfile::TempDir,
}

impl Profile {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        Self { root }
    }

    fn path(&self) -> &Path {
        self.root.path()
    }

    fn database(&self) -> PathBuf {
        self.path().join(SQLITE_DATABASE_FILE)
    }

    fn owner(&self) -> OwnerPermit {
        ProfileOwner::acquire(ProfileIdentity::existing(self.path()).unwrap()).unwrap()
    }

    fn open(&self) -> (OwnerPermit, SqliteDbState) {
        let owner = self.owner();
        let db = SqliteDbState::open(self.database()).unwrap();
        fs::set_permissions(self.database(), fs::Permissions::from_mode(0o600)).unwrap();
        (owner, db)
    }

    fn snapshot(&self) -> BTreeMap<OsString, (u64, u32, Vec<u8>)> {
        fs::read_dir(self.path())
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                let metadata = fs::symlink_metadata(entry.path()).unwrap();
                assert!(metadata.is_file());
                (
                    entry.file_name(),
                    (
                        metadata.ino(),
                        metadata.mode(),
                        fs::read(entry.path()).unwrap(),
                    ),
                )
            })
            .collect()
    }
}

fn count(db: &SqliteDbState) -> i64 {
    db.with_conn(|c| {
        c.query_row("SELECT COUNT(*) FROM attach_probe", [], |row| row.get(0))
            .map_err(|e| e.to_string())
    })
    .unwrap()
}

#[test]
fn attach_shares_uncommitted_transaction_and_existing_owner_gate() {
    let profile = Profile::new();
    let (owner, db) = profile.open();
    db.with_conn(|c| {
        c.execute_batch(
            "CREATE TABLE attach_probe(value INTEGER); BEGIN; INSERT INTO attach_probe VALUES (7);",
        )
        .map_err(|e| e.to_string())
    })
    .unwrap();
    let writer = owner.try_writer().unwrap();
    let service = ContextService::attach(&owner, &db).unwrap();
    assert_eq!(service.owner.epoch(), owner.epoch());
    assert!(service.owner.try_writer().is_err());
    assert!(matches!(
        ProfileOwner::acquire(ProfileIdentity::existing(profile.path()).unwrap()),
        Err(crate::Error::OwnerBusy)
    ));
    assert_eq!(count(&service.db), 1);
    service
        .db
        .with_conn(|c| {
            assert!(!c.is_autocommit());
            c.execute_batch("ROLLBACK").map_err(|e| e.to_string())
        })
        .unwrap();
    assert_eq!(count(&db), 0);
    drop(writer);
    assert!(service.owner.try_writer().is_ok());
    drop(service);
    drop(db);
    drop(owner);
}

#[test]
fn attach_preserves_real_commit_veto_on_the_original_connection() {
    let profile = Profile::new();
    let (owner, db) = profile.open();
    db.with_conn(|c| {
        c.execute_batch("CREATE TABLE attach_probe(value INTEGER)")
            .map_err(|e| e.to_string())
    })
    .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let hook_calls = calls.clone();
    db.with_conn(|c| {
        c.commit_hook(Some(move || {
            hook_calls.fetch_add(1, Ordering::SeqCst);
            true
        }));
        Ok(())
    })
    .unwrap();
    let service = ContextService::attach(&owner, &db).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        service.transaction(|tx| {
            tx.execute("INSERT INTO attach_probe VALUES (7)", [])
                .map_err(|_| ServiceError::Storage)?;
            Ok(())
        }),
        Err(ServiceError::PersistenceUncertain)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(count(&db), 0);
    db.with_conn(|c| {
        c.commit_hook(None::<fn() -> bool>);
        Ok(())
    })
    .unwrap();
    service
        .transaction(|tx| {
            tx.execute("INSERT INTO attach_probe VALUES (8)", [])
                .map_err(|_| ServiceError::Storage)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(count(&db), 1);
    drop(service);
    drop(db);
    drop(owner);
}

#[test]
fn attach_does_not_migrate_seed_create_files_or_change_the_database() {
    let profile = Profile::new();
    let (owner, db) = profile.open();
    let before = db
        .with_conn(|c| {
            c.pragma_update(None, "user_version", TARGET_SCHEMA_VERSION - 1)
                .map_err(|e| e.to_string())?;
            assert_eq!(c.execute(
            "DELETE FROM model_pricing WHERE rowid = (SELECT rowid FROM model_pricing LIMIT 1)", []
        ).map_err(|e| e.to_string())?, 1);
            let count: i64 = c
                .query_row("SELECT COUNT(*) FROM model_pricing", [], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            Ok((count, c.total_changes()))
        })
        .unwrap();
    assert!(!profile.path().join(SQLITE_MIGRATION_BACKUP_DIR).exists());
    let files = profile.snapshot();
    let service = ContextService::attach(&owner, &db).unwrap();
    assert_eq!(profile.snapshot(), files);
    service
        .db
        .with_conn(|c| {
            let version: i32 = c
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .map_err(|e| e.to_string())?;
            let count: i64 = c
                .query_row("SELECT COUNT(*) FROM model_pricing", [], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            assert_eq!(version, TARGET_SCHEMA_VERSION - 1);
            assert_eq!((count, c.total_changes()), before);
            Ok(())
        })
        .unwrap();
    assert!(!profile.path().join(SQLITE_MIGRATION_BACKUP_DIR).exists());
    drop(service);
    drop(db);
    drop(owner);
}

#[test]
fn attach_rejects_missing_database_without_creating_one_or_retaining_owner() {
    let profile = Profile::new();
    let owner = profile.owner();
    let db = SqliteDbState::in_memory_for_test().unwrap();
    let before = profile.snapshot();
    assert!(matches!(
        ContextService::attach(&owner, &db),
        Err(ServiceError::Storage)
    ));
    assert_eq!(profile.snapshot(), before);
    assert!(!profile.database().exists());
    drop(owner);
    let replacement_owner = profile.owner();
    drop(db);
    drop(replacement_owner);
}

#[test]
fn attach_rejects_wrong_profile_and_does_not_bind_the_failed_owner() {
    let profile = Profile::new();
    let other = Profile::new();
    let (owner, db) = profile.open();
    let (other_owner, other_db) = other.open();
    assert!(matches!(
        ContextService::attach(&other_owner, &db),
        Err(ServiceError::Storage)
    ));
    // A failed attach must not have bound other_owner to db.
    let service = ContextService::attach(&owner, &db).unwrap();
    drop(other_db);
    drop(other_owner);
    let replacement_owner = other.owner();
    drop(replacement_owner);
    drop(service);
    drop(db);
    drop(owner);
}

#[test]
fn attach_uses_actual_main_filename_not_cached_state_path() {
    let profile = Profile::new();
    let other = Profile::new();
    let (owner, db) = profile.open();
    let (other_owner, other_db) = other.open();
    // Exercise the existing trusted Rust API's ability to replace a Connection.
    // No state path is forged; the cached path remains the original profile.
    let original = db
        .with_conn_mut(|c| {
            let replacement = Connection::open(other.database()).map_err(|e| e.to_string())?;
            Ok(std::mem::replace(c, replacement))
        })
        .unwrap();
    assert_eq!(db.db_path(), profile.database());
    assert!(matches!(
        ContextService::attach(&owner, &db),
        Err(ServiceError::Storage)
    ));
    db.with_conn_mut(|c| {
        *c = original;
        Ok(())
    })
    .unwrap();
    let service = ContextService::attach(&owner, &db).unwrap();
    drop(service);
    drop(db);
    drop(owner);
    drop(other_db);
    drop(other_owner);
}

#[test]
fn attached_service_rejects_later_connection_replacement_without_writing_wrong_database() {
    let profile = Profile::new();
    let other = Profile::new();
    let (owner, db) = profile.open();
    let (other_owner, other_db) = other.open();
    for state in [&db, &other_db] {
        state
            .with_conn(|c| {
                c.execute_batch("CREATE TABLE attach_probe(value INTEGER)")
                    .map_err(|e| e.to_string())
            })
            .unwrap();
    }
    let service = ContextService::attach(&owner, &db).unwrap();
    let original = db
        .with_conn_mut(|c| {
            let replacement = Connection::open(other.database()).map_err(|e| e.to_string())?;
            Ok(std::mem::replace(c, replacement))
        })
        .unwrap();
    let files = other.snapshot();
    let attempted = AtomicBool::new(false);
    assert_eq!(
        service.transaction(|tx| {
            attempted.store(true, Ordering::SeqCst);
            tx.execute("INSERT INTO attach_probe VALUES (99)", [])
                .map_err(|_| ServiceError::Storage)?;
            Ok(())
        }),
        Err(ServiceError::Storage)
    );
    assert!(!attempted.load(Ordering::SeqCst));
    let request = ContextRequest {
        schema_version: 1,
        request_id: "replaced-connection".into(),
        expected_provider_id: "synthetic".into(),
        expected_revision: "0".repeat(64),
        target: Target::Common,
        patch: ContextPatch {
            model_context_window: Some(2000000),
            model_auto_compact_token_limit: None,
        },
    };
    assert_eq!(service.execute(&request), Err(ServiceError::Storage));
    assert_eq!(other.snapshot(), files);
    assert_eq!(count(&other_db), 0);
    db.with_conn_mut(|c| {
        *c = original;
        Ok(())
    })
    .unwrap();
    assert_eq!(service.ensure_current(), Ok(()));
    assert_eq!(count(&db), 0);
    drop(service);
    drop(db);
    drop(owner);
    drop(other_db);
    drop(other_owner);
}

#[test]
fn attach_rejects_replaced_open_database_inode_even_when_path_still_matches() {
    let profile = Profile::new();
    let (owner, db) = profile.open();
    let original = profile.path().join("held-original.db");
    fs::rename(profile.database(), &original).unwrap();
    assert!(matches!(
        ContextService::attach(&owner, &db),
        Err(ServiceError::Storage)
    ));
    assert!(!profile.database().exists());
    fs::write(profile.database(), b"synthetic replacement").unwrap();
    fs::set_permissions(profile.database(), fs::Permissions::from_mode(0o600)).unwrap();
    assert_ne!(
        database_identity(&original).unwrap(),
        database_identity(&profile.database()).unwrap()
    );
    assert!(matches!(
        ContextService::attach(&owner, &db),
        Err(ServiceError::Storage)
    ));
    fs::rename(&original, profile.database()).unwrap();
    let service = ContextService::attach(&owner, &db).unwrap();
    drop(service);
    drop(db);
    drop(owner);
}

#[test]
fn attach_rejects_unsafe_database_and_sidecars_without_repair() {
    for leaf in [
        SQLITE_DATABASE_FILE,
        "ai-toolbox.db-wal",
        "ai-toolbox.db-shm",
        "ai-toolbox.db-journal",
    ] {
        for kind in ["mode", "symlink", "hardlink", "directory"] {
            let profile = Profile::new();
            let (owner, db) = profile.open();
            let path = profile.path().join(leaf);
            let held = profile.path().join("held-original");
            let existed = path.exists();
            if existed {
                fs::rename(&path, &held).unwrap();
            }
            match kind {
                "mode" => {
                    fs::write(&path, b"synthetic unsafe leaf").unwrap();
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
                }
                "symlink" => symlink("missing-synthetic-target", &path).unwrap(),
                "hardlink" => {
                    let target = profile.path().join("synthetic-hardlink-target");
                    fs::write(&target, b"synthetic hardlink").unwrap();
                    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
                    fs::hard_link(target, &path).unwrap();
                }
                "directory" => fs::create_dir(&path).unwrap(),
                _ => unreachable!(),
            }
            let before = fs::symlink_metadata(&path).unwrap();
            assert!(
                matches!(
                    ContextService::attach(&owner, &db),
                    Err(ServiceError::Storage)
                ),
                "{leaf}: {kind}"
            );
            let after = fs::symlink_metadata(&path).unwrap();
            assert_eq!((before.ino(), before.mode()), (after.ino(), after.mode()));
            if kind == "directory" {
                fs::remove_dir(&path).unwrap();
            } else {
                fs::remove_file(&path).unwrap();
            }
            if existed {
                fs::rename(&held, &path).unwrap();
            }
            let service = ContextService::attach(&owner, &db).unwrap();
            drop(service);
            drop(db);
            drop(owner);
        }
    }
}

#[test]
fn attached_service_rechecks_sidecar_safety_before_work() {
    for suffix in ["-wal", "-shm"] {
        let profile = Profile::new();
        let (owner, db) = profile.open();
        let service = ContextService::attach(&owner, &db).unwrap();
        let path = profile
            .path()
            .join(format!("{SQLITE_DATABASE_FILE}{suffix}"));
        assert!(path.exists());
        let original = fs::metadata(&path).unwrap().permissions();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
        let attempted = AtomicBool::new(false);
        assert_eq!(
            service.transaction(|_| {
                attempted.store(true, Ordering::SeqCst);
                Ok(())
            }),
            Err(ServiceError::Storage)
        );
        assert!(!attempted.load(Ordering::SeqCst));
        fs::set_permissions(&path, original).unwrap();
        assert_eq!(service.ensure_current(), Ok(()));
        drop(service);
        drop(db);
        drop(owner);
    }
}

#[test]
fn legal_sqlite_sidecar_lifecycle_does_not_invalidate_the_main_connection() {
    let profile = Profile::new();
    let (owner, db) = profile.open();
    db.with_conn(|c| {
        c.execute_batch("CREATE TABLE attach_probe(value INTEGER)")
            .map_err(|e| e.to_string())
    })
    .unwrap();
    let service = ContextService::attach(&owner, &db).unwrap();
    let main_inode = database_identity(&profile.database()).unwrap();
    let wal = profile.path().join("ai-toolbox.db-wal");
    let shm = profile.path().join("ai-toolbox.db-shm");
    let journal = profile.path().join("ai-toolbox.db-journal");
    assert!(wal.exists() && shm.exists());
    db.with_conn(|c| {
        c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;")
            .map_err(|e| e.to_string())
    })
    .unwrap();
    assert!(!wal.exists() && !shm.exists());
    assert_eq!(service.ensure_current(), Ok(()));
    service
        .transaction(|tx| {
            tx.execute("INSERT INTO attach_probe VALUES (1)", [])
                .map_err(|_| ServiceError::Storage)?;
            assert!(journal.exists());
            Ok(())
        })
        .unwrap();
    assert!(!journal.exists());
    assert_eq!(service.ensure_current(), Ok(()));
    db.with_conn(|c| {
        c.execute_batch("PRAGMA journal_mode = WAL; INSERT INTO attach_probe VALUES (2);")
            .map_err(|e| e.to_string())
    })
    .unwrap();
    assert!(wal.exists() && shm.exists());
    assert_eq!(database_identity(&profile.database()).unwrap(), main_inode);
    assert_eq!(service.ensure_current(), Ok(()));
    assert_eq!(count(&db), 2);
    drop(service);
    drop(db);
    drop(owner);
}

#[test]
fn attach_rejects_stale_owner_before_binding_database() {
    let profile = Profile::new();
    let (owner, db) = profile.open();
    let lease = profile.path().join(PROFILE_LEASE_FILE);
    let held = profile.path().join("held-original-lease");
    fs::rename(&lease, &held).unwrap();
    assert!(matches!(
        ContextService::attach(&owner, &db),
        Err(ServiceError::Ownership)
    ));
    assert!(!lease.exists());
    fs::rename(&held, &lease).unwrap();
    drop(owner);
    // The failed attach did not keep the old lease alive via the DB state.
    let replacement_owner = profile.owner();
    let service = ContextService::attach(&replacement_owner, &db).unwrap();
    drop(service);
    drop(db);
    drop(replacement_owner);
}

#[test]
fn original_and_later_database_clones_retain_owner_until_last_connection_drop() {
    let profile = Profile::new();
    let (owner, db) = profile.open();
    let preexisting_clone = db.clone();
    let service = ContextService::attach(&owner, &db).unwrap();
    let repeated = ContextService::attach(&owner, &preexisting_clone).unwrap();
    let later_clone = service.db.clone();
    let epoch = owner.epoch();
    drop(owner);
    drop(service);
    drop(repeated);
    drop(db);
    assert!(matches!(
        ProfileOwner::acquire(ProfileIdentity::existing(profile.path()).unwrap()),
        Err(crate::Error::OwnerBusy)
    ));
    drop(preexisting_clone);
    assert!(matches!(
        ProfileOwner::acquire(ProfileIdentity::existing(profile.path()).unwrap()),
        Err(crate::Error::OwnerBusy)
    ));
    later_clone
        .with_conn(|c| {
            c.execute_batch("CREATE TABLE last_clone_probe(value INTEGER)")
                .map_err(|e| e.to_string())
        })
        .unwrap();
    drop(later_clone);
    let replacement_owner = profile.owner();
    assert_ne!(replacement_owner.epoch(), epoch);
    assert!(!profile.path().join("ai-toolbox.db-wal").exists());
    assert!(!profile.path().join("ai-toolbox.db-shm").exists());
    drop(replacement_owner);
}
