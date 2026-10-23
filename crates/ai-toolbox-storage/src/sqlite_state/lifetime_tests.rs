use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

struct CloseProbe {
    wal: PathBuf,
    shm: PathBuf,
    closed_before_release: Arc<AtomicBool>,
}

impl Drop for CloseProbe {
    fn drop(&mut self) {
        // SQLite's last real WAL connection removes these files on close.
        // Observe from the guard destructor, not after the whole state drops.
        self.closed_before_release
            .store(!self.wal.exists() && !self.shm.exists(), Ordering::SeqCst);
    }
}

#[test]
fn shared_connection_closes_before_lifetime_owner_is_dropped() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("synthetic.db");
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch(
        "PRAGMA journal_mode = WAL; CREATE TABLE probe(value INTEGER); INSERT INTO probe VALUES (1);"
    ).unwrap();
    let wal = root.path().join("synthetic.db-wal");
    let shm = root.path().join("synthetic.db-shm");
    assert!(wal.exists() && shm.exists());
    let state = SqliteDbState {
        conn: Arc::new(Mutex::new(SharedConnection {
            connection,
            lifetime_owner: None,
        })),
        db_path: path,
    };
    let old_clone = state.clone();
    let closed_before_release = Arc::new(AtomicBool::new(false));
    state
        .retain_lifetime_owner_checked(
            Arc::new(CloseProbe {
                wal,
                shm,
                closed_before_release: closed_before_release.clone(),
            }),
            |_| Ok::<_, ()>(()),
        )
        .unwrap();
    let later_clone = state.clone();
    drop(state);
    drop(old_clone);
    assert!(!closed_before_release.load(Ordering::SeqCst));
    drop(later_clone);
    assert!(closed_before_release.load(Ordering::SeqCst));
}

#[test]
fn lifetime_owner_binding_is_idempotent_and_never_overwrites_another_owner() {
    let state = SqliteDbState::in_memory_for_test().unwrap();
    let owner: Arc<dyn Send + Sync> = Arc::new(7_u8);
    let other: Arc<dyn Send + Sync> = Arc::new(7_u8);
    let original = Arc::downgrade(&owner);
    let rejected = Arc::downgrade(&other);
    state
        .retain_lifetime_owner_checked(owner.clone(), |_| Ok::<_, ()>(()))
        .unwrap();
    state
        .clone()
        .retain_lifetime_owner_checked(owner.clone(), |_| Ok::<_, ()>(()))
        .unwrap();
    assert_eq!(Arc::strong_count(&owner), 2);
    assert!(matches!(
        state.retain_lifetime_owner_checked(other.clone(), |_| Ok::<_, ()>(())),
        Err(LifetimeOwnerError::DifferentOwner)
    ));
    assert_eq!(Arc::strong_count(&other), 1);
    drop(owner);
    drop(other);
    assert!(original.upgrade().is_some());
    assert!(rejected.upgrade().is_none());
    drop(state);
    assert!(original.upgrade().is_none());
}

#[test]
fn failed_validation_never_installs_an_owner_and_runs_under_connection_mutex() {
    let state = SqliteDbState::in_memory_for_test().unwrap();
    let owner: Arc<dyn Send + Sync> = Arc::new(7_u8);
    let weak = Arc::downgrade(&owner);
    let outcome = state.retain_lifetime_owner_checked(owner, |connection| {
        assert!(state.conn.try_lock().is_err());
        assert!(connection.is_autocommit());
        Err::<(), _>("synthetic validation failure")
    });
    assert!(matches!(
        outcome,
        Err(LifetimeOwnerError::Validation(
            "synthetic validation failure"
        ))
    ));
    assert!(weak.upgrade().is_none());
    let different_owner: Arc<dyn Send + Sync> = Arc::new(8_u8);
    state
        .retain_lifetime_owner_checked(different_owner, |_| Ok::<_, ()>(()))
        .unwrap();
}
