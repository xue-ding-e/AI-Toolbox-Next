//! Synthetic Linux admission tests. These do not launch a GUI or a tool CLI.
//! Wire this file as a cfg(test, target_os = "linux") child of management_admission.

use super::*;
use ai_toolbox_management::{
    owner::{OrdinaryRuntimeLeaseGroup, ProfileLeaseStartup, RuntimeResourceIdentity},
    Error,
};
use std::{
    fs::{self, DirBuilder, OpenOptions},
    io::Read,
    os::unix::fs::{symlink, DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const WAIT: Duration = Duration::from_secs(10);
const ADMISSION_ACTOR: &str = "management_admission::tests::isolated_admission_actor";
const MARKER: &[u8] = b"synthetic-gui-admission-v1";

/// No automatic deletion: a panic or an unconfirmed worker/actor leaves the
/// fixture intact. Only the successful, fully joined path calls cleanup.
struct Fixture {
    path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir()
            .join(format!("ai-toolbox-gui-admission-{}", uuid::Uuid::new_v4(),));
        private_directory(&path);
        write_private(&path.join("fixture-marker"), MARKER);
        Self { path }
    }

    fn session(&self, managed: bool) -> Arc<GuiProfileSession> {
        session_at(&self.path, managed)
    }

    fn cleanup(self) {
        assert!(!thread::panicking());
        assert_eq!(fs::read(self.path.join("fixture-marker")).unwrap(), MARKER);
        // This is a final lease check, not a substitute for joining workers.
        drop(ProfileLeaseStartup::acquire(&self.path).unwrap());
        fs::remove_dir_all(&self.path).unwrap();
    }
}

fn private_directory(path: &Path) {
    DirBuilder::new().mode(0o700).create(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(fs::symlink_metadata(path).unwrap().mode() & 0o7777, 0o700);
}

fn write_private(path: &Path, content: &[u8]) {
    use std::io::Write;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .unwrap();
    file.write_all(content).unwrap();
}

fn session_at(path: &Path, managed: bool) -> Arc<GuiProfileSession> {
    let startup = ProfileLeaseStartup::acquire(path).unwrap();
    if managed {
        GuiProfileSession::managed(startup.into_managed().expect(
            "synthetic TMPDIR profile must satisfy real managed admission; use a private supported tmpfs",
        ))
    } else {
        GuiProfileSession::ordinary(startup.into_ordinary())
    }
}

fn assert_busy(path: &Path) {
    assert!(matches!(
        ProfileLeaseStartup::acquire(path),
        Err(Error::OwnerBusy)
    ));
}

fn assert_released(path: &Path) {
    let lease = ProfileLeaseStartup::acquire(path).unwrap().into_ordinary();
    lease.ensure_current().unwrap();
    drop(lease);
}

fn registry_pointer() -> Option<*const GuiProfileSession> {
    SESSION.get().map(Weak::as_ptr)
}

fn override_pointer() -> Option<*const GuiProfileSession> {
    TEST_SESSION.with(|slot| slot.borrow().as_ref().map(Weak::as_ptr))
}

#[test]
fn nested_test_scopes_restore_override_without_installing_production_registry() {
    let production_before = registry_pointer();
    let override_before = override_pointer();
    let ordinary_fixture = Fixture::new();
    let managed_fixture = Fixture::new();
    let ordinary = ordinary_fixture.session(false);
    let managed = managed_fixture.session(true);
    {
        let _ordinary_scope = test_session(&ordinary);
        assert!(Arc::ptr_eq(&current_session().unwrap().unwrap(), &ordinary));
        {
            let _managed_scope = test_session(&managed);
            assert!(Arc::ptr_eq(&current_session().unwrap().unwrap(), &managed));
        }
        assert!(Arc::ptr_eq(&current_session().unwrap().unwrap(), &ordinary));
    }
    assert_eq!(override_pointer(), override_before);
    assert_eq!(registry_pointer(), production_before);
    drop((ordinary, managed));
    ordinary_fixture.cleanup();
    managed_fixture.cleanup();
}

#[test]
fn unwinding_test_scope_restores_previous_override() {
    let production_before = registry_pointer();
    let fixture = Fixture::new();
    let nested_fixture = Fixture::new();
    let session = fixture.session(false);
    let nested = nested_fixture.session(true);
    let scope = test_session(&session);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _nested_scope = test_session(&nested);
        panic!("intentional synthetic scope unwind");
    }));
    assert!(outcome.is_err());
    assert!(Arc::ptr_eq(&current_session().unwrap().unwrap(), &session));
    assert_eq!(registry_pointer(), production_before);
    drop((scope, session, nested));
    fixture.cleanup();
    nested_fixture.cleanup();
}

#[test]
fn thread_local_override_is_not_inherited_by_an_os_thread() {
    let fixture = Fixture::new();
    let session = fixture.session(false);
    let scope = test_session(&session);
    let (sent, received) = mpsc::channel();
    let worker = thread::spawn(move || {
        let no_override = TEST_SESSION.with(|slot| slot.borrow().is_none());
        sent.send(no_override).unwrap();
    });
    let no_override = received
        .recv_timeout(WAIT)
        .expect("bounded TLS probe response");
    // Receiving a message alone is not proof that a thread has exited.
    let deadline = Instant::now() + WAIT;
    while !worker.is_finished() {
        assert!(
            Instant::now() < deadline,
            "TLS probe exit was not confirmed"
        );
        thread::yield_now();
    }
    worker.join().expect("TLS probe must join without panic");
    assert!(no_override);
    assert!(Arc::ptr_eq(&current_session().unwrap().unwrap(), &session));
    drop((scope, session));
    fixture.cleanup();
}

#[test]
fn expired_override_fails_closed_instead_of_falling_back_to_ordinary() {
    let production_before = registry_pointer();
    let fixture = Fixture::new();
    let session = fixture.session(false);
    let weak = Arc::downgrade(&session);
    let scope = test_session(&session);
    drop(session);
    assert!(weak.upgrade().is_none());
    assert!(
        override_pointer().is_some(),
        "the expired override must remain installed"
    );
    assert!(matches!(current_session(), Err(error) if error == UNAVAILABLE));
    assert!(matches!(begin_codex_write(), Err(error) if error == UNAVAILABLE));
    assert!(matches!(reject_managed_operation(), Err(error) if error == UNAVAILABLE));
    assert_eq!(registry_pointer(), production_before);
    drop(scope);
    assert_released(&fixture.path);
    fixture.cleanup();
}

#[test]
fn ordinary_admission_and_unsupported_operation_guard_retain_real_lease() {
    let fixture = Fixture::new();
    let session = fixture.session(false);
    let scope = test_session(&session);
    let unsupported_guard = reject_managed_operation().unwrap().unwrap();
    let write_guard = begin_codex_write().unwrap();
    assert!(write_guard.managed_writer().is_none());
    write_guard.ensure_current().unwrap();
    drop((scope, session));
    assert_busy(&fixture.path);
    drop(unsupported_guard);
    assert_busy(&fixture.path);
    drop(write_guard);
    assert_released(&fixture.path);
    fixture.cleanup();
}

#[test]
fn managed_admission_rejects_unsupported_work_and_serializes_writer_guards() {
    let fixture = Fixture::new();
    let session = fixture.session(true);
    let scope = test_session(&session);
    assert!(matches!(reject_managed_operation(), Err(error) if error == MANAGED_UNSUPPORTED));
    let first = begin_codex_write().unwrap();
    assert!(first.managed_writer().is_some());
    first.ensure_current().unwrap();
    assert!(matches!(begin_codex_write(), Err(error)
        if error == "Another managed Codex operation is active"));
    drop(first);
    let second = begin_codex_write().unwrap();
    second.ensure_current().unwrap();
    drop((scope, session));
    assert_busy(&fixture.path);
    drop(second);
    assert_released(&fixture.path);
    fixture.cleanup();
}

/// Runtime Drop must not turn a failure into an unbounded wait for a blocking
/// worker. Timeout keeps the fixture; it never certifies that a task stopped.
struct BoundedRuntime(Option<tokio::runtime::Runtime>);

impl BoundedRuntime {
    fn new() -> Self {
        Self(Some(
            tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .max_blocking_threads(1)
                .build()
                .unwrap(),
        ))
    }

    fn runtime(&self) -> &tokio::runtime::Runtime {
        self.0.as_ref().unwrap()
    }
}

impl Drop for BoundedRuntime {
    fn drop(&mut self) {
        if let Some(runtime) = self.0.take() {
            runtime.shutdown_timeout(WAIT);
        }
    }
}

struct ReleaseWorker(Option<mpsc::Sender<()>>);

impl ReleaseWorker {
    fn release(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

impl Drop for ReleaseWorker {
    fn drop(&mut self) {
        self.release();
    }
}

struct ObserveOuterDrop(Arc<AtomicBool>);

impl Drop for ObserveOuterDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

enum MovedAdmission {
    Codex(CodexWriteAdmission),
    Legacy(LegacyRuntimeAdmission),
}

impl MovedAdmission {
    fn ensure_current(&self) -> Result<(), String> {
        match self {
            Self::Codex(admission) => admission.ensure_current(),
            Self::Legacy(admission) => admission.ensure_current(),
        }
    }
}

fn blocking_worker_outlives_cancelled_outer(managed: bool, legacy: bool) {
    let fixture = Fixture::new();
    let session = fixture.session(managed);
    let weak = Arc::downgrade(&session);
    let scope = test_session(&session);
    let legacy_root = fixture.path.join("legacy-runtime");
    let contender_fixture = legacy.then(Fixture::new);
    let contender_session = contender_fixture
        .as_ref()
        .map(|fixture| fixture.session(false));
    let admission = if legacy {
        assert!(!managed);
        private_directory(&legacy_root);
        MovedAdmission::Legacy(admit_legacy_runtime(&legacy_root).unwrap())
    } else {
        let admission = begin_codex_write().unwrap();
        assert_eq!(admission.managed_writer().is_some(), managed);
        MovedAdmission::Codex(admission)
    };
    let runtime = BoundedRuntime::new();
    let (release_tx, release_rx) = mpsc::channel();
    let mut release = ReleaseWorker(Some(release_tx));
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (handle_tx, handle_rx) = tokio::sync::oneshot::channel();
    let (completed_tx, completed_rx) = tokio::sync::oneshot::channel();
    let outer_dropped = Arc::new(AtomicBool::new(false));
    let observed_drop = outer_dropped.clone();
    let caller_thread = thread::current().id();

    let mut outer = Box::pin(async move {
        let _observe_drop = ObserveOuterDrop(observed_drop);
        let worker = tokio::task::spawn_blocking(move || -> Result<(), String> {
            // The accepted capability, not the caller's TLS override, crosses
            // the blocking boundary. Never reacquire admission on this thread.
            admission.ensure_current()?;
            let no_override = TEST_SESSION.with(|slot| slot.borrow().is_none());
            started_tx
                .send((thread::current().id(), no_override))
                .map_err(|_| "synthetic start observer disappeared".to_owned())?;
            release_rx
                .recv_timeout(WAIT)
                .map_err(|_| "synthetic worker release deadline exceeded".to_owned())?;
            admission.ensure_current()?;
            drop(admission);
            // Cancellation may have dropped the outer receiver. Work still
            // completes, and the separately retained JoinHandle proves it did.
            let _ = completed_tx.send(());
            Ok(())
        });
        assert!(handle_tx.send(worker).is_ok());
        let _ = completed_rx.await;
    });

    let (worker, worker_thread, no_override) = runtime.runtime().block_on(async {
        tokio::time::timeout(WAIT, async {
            let (worker_thread, no_override) = tokio::select! {
                started = started_rx => started.expect("worker must acknowledge admission"),
                _ = &mut outer => panic!("outer completed before the worker was released"),
            };
            (
                handle_rx
                    .await
                    .expect("retain worker handle for bounded joining"),
                worker_thread,
                no_override,
            )
        })
        .await
        .expect("bounded blocking-worker start")
    });
    assert_ne!(worker_thread, caller_thread);
    assert!(no_override);
    assert!(!outer_dropped.load(Ordering::SeqCst));
    assert!(!worker.is_finished());

    // This actually drops a polled, waiting async future. No elapsed-time
    // inference or sleep is used to claim cancellation coverage.
    drop(outer);
    assert!(outer_dropped.load(Ordering::SeqCst));
    drop((scope, session));
    assert_busy(&fixture.path);
    if let Some(contender) = &contender_session {
        assert!(matches!(
            probe_runtime(contender, &legacy_root),
            Err(Error::ResourceBusy)
        ));
    }
    {
        let retained = weak
            .upgrade()
            .expect("accepted worker must retain the session");
        retained.ensure_current().unwrap();
        if let LinuxMode::Managed(owner) = &retained.mode {
            assert!(matches!(owner.try_writer(), Err(Error::WriterBusy)));
        }
    }
    release.release();
    runtime.runtime().block_on(async {
        tokio::time::timeout(WAIT, worker)
            .await
            .expect("worker completion was not confirmed; retain fixture")
            .expect("blocking worker panicked")
            .expect("blocking worker failed");
    });
    // JoinHandle success is completion proof; a readiness message is not.
    assert!(weak.upgrade().is_none());
    assert_released(&fixture.path);
    if let Some(contender) = &contender_session {
        drop(probe_runtime(contender, &legacy_root).unwrap());
    }
    drop(runtime);
    drop(contender_session);
    if let Some(contender) = contender_fixture {
        contender.cleanup();
    }
    fixture.cleanup();
}

#[test]
fn ordinary_blocking_guard_survives_outer_future_cancellation() {
    blocking_worker_outlives_cancelled_outer(false, false);
}

#[test]
fn managed_blocking_guard_survives_outer_future_cancellation() {
    blocking_worker_outlives_cancelled_outer(true, false);
}

#[test]
fn legacy_runtime_blocking_guard_survives_outer_future_cancellation() {
    blocking_worker_outlives_cancelled_outer(false, true);
}

fn probe_runtime(
    session: &Arc<GuiProfileSession>,
    root: &Path,
) -> ai_toolbox_management::Result<OrdinaryRuntimeLeaseGroup> {
    let LinuxMode::Ordinary(profile) = &session.mode else {
        panic!("runtime contender must use an ordinary synthetic profile");
    };
    OrdinaryRuntimeLeaseGroup::acquire(profile, vec![root.to_path_buf()])
}

#[test]
fn ordinary_nested_runtime_and_alias_admission_reuse_the_same_arc() {
    let fixture = Fixture::new();
    let root = fixture.path.join("runtime");
    let alias = fixture.path.join("runtime-alias");
    private_directory(&root);
    symlink(&root, &alias).unwrap();
    let session = fixture.session(false);
    let scope = test_session(&session);
    let first = admit_legacy_runtime(&root).unwrap();
    let nested = admit_legacy_runtime(&root).unwrap();
    let aliased = admit_legacy_runtime(&alias).unwrap();
    assert!(Arc::ptr_eq(
        first.runtime.as_ref().unwrap(),
        nested.runtime.as_ref().unwrap()
    ));
    assert!(Arc::ptr_eq(
        first.runtime.as_ref().unwrap(),
        aliased.runtime.as_ref().unwrap()
    ));
    assert_eq!(session.ordinary_roots.lock().unwrap().len(), 1);
    first.ensure_current().unwrap();
    nested.ensure_current().unwrap();
    aliased.ensure_current().unwrap();
    drop((first, nested, aliased, scope, session));
    fixture.cleanup();
}

#[test]
fn different_ordinary_profiles_contend_on_one_physical_runtime() {
    let fixture = Fixture::new();
    let contender_fixture = Fixture::new();
    let root = fixture.path.join("shared-runtime");
    let alias = fixture.path.join("shared-alias");
    private_directory(&root);
    symlink(&root, &alias).unwrap();
    let first_session = fixture.session(false);
    let contender = contender_fixture.session(false);
    let first_scope = test_session(&first_session);
    let first = admit_legacy_runtime(&root).unwrap();
    {
        let _contender_scope = test_session(&contender);
        assert!(admit_legacy_runtime(&alias).is_err());
        assert!(contender.ordinary_roots.lock().unwrap().is_empty());
        assert!(matches!(
            probe_runtime(&contender, &root),
            Err(Error::ResourceBusy)
        ));
    }
    drop((first, first_scope, first_session));
    {
        let _contender_scope = test_session(&contender);
        let accepted = admit_legacy_runtime(&alias).unwrap();
        accepted.ensure_current().unwrap();
    }
    drop(contender);
    contender_fixture.cleanup();
    fixture.cleanup();
}

#[test]
fn ordinary_session_conservatively_retains_previous_runtime_roots() {
    let fixture = Fixture::new();
    let contender_fixture = Fixture::new();
    let old_root = fixture.path.join("old-runtime");
    let new_root = fixture.path.join("new-runtime");
    private_directory(&old_root);
    private_directory(&new_root);
    let session = fixture.session(false);
    let contender = contender_fixture.session(false);
    let scope = test_session(&session);
    let old_guard = admit_legacy_runtime(&old_root).unwrap();
    let new_guard = admit_legacy_runtime(&new_root).unwrap();
    assert!(!Arc::ptr_eq(
        old_guard.runtime.as_ref().unwrap(),
        new_guard.runtime.as_ref().unwrap()
    ));
    drop((old_guard, new_guard));
    assert_eq!(session.ordinary_roots.lock().unwrap().len(), 2);
    for root in [&old_root, &new_root] {
        assert!(matches!(
            probe_runtime(&contender, root),
            Err(Error::ResourceBusy)
        ));
    }
    // Admission-cache coverage only: no root-change settings/DB transaction
    // is invoked here, so this does not prove failed settings remain unwritten.
    drop((scope, session));
    for root in [&old_root, &new_root] {
        drop(probe_runtime(&contender, root).unwrap());
    }
    drop(contender);
    contender_fixture.cleanup();
    fixture.cleanup();
}

struct ManagedTargets {
    profile: PathBuf,
    runtime: PathBuf,
    other: PathBuf,
    session: Arc<GuiProfileSession>,
}

impl ManagedTargets {
    fn new(fixture: &Fixture) -> Self {
        let profile = fixture.path.join("managed-profile");
        let runtime = fixture.path.join("codex-runtime");
        let other = fixture.path.join("other-tool");
        for path in [&profile, &runtime, &other] {
            private_directory(path);
        }
        let owner = ProfileLeaseStartup::acquire(&profile)
            .unwrap()
            .into_managed()
            .unwrap();
        let identity = RuntimeResourceIdentity::existing(&runtime).unwrap();
        let session = GuiProfileSession::managed_with_runtime(owner, identity).unwrap();
        Self {
            profile,
            runtime,
            other,
            session,
        }
    }
}

/// Snapshot names, physical identity, mode, links and bytes without following
/// symlinks. Admission should not create lock files, directories or payloads.
/// Access times are intentionally excluded because reading can update them.
#[derive(Debug, Eq, PartialEq)]
struct TreeEntry {
    relative: PathBuf,
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    content: Vec<u8>,
    symlink_target: Option<PathBuf>,
}

fn snapshot_tree(root: &Path) -> Vec<TreeEntry> {
    fn visit(root: &Path, path: &Path, entries: &mut Vec<TreeEntry>) {
        let meta = fs::symlink_metadata(path).unwrap();
        let symlink_target = meta
            .file_type()
            .is_symlink()
            .then(|| fs::read_link(path).unwrap());
        let content = if meta.is_file() {
            fs::read(path).unwrap()
        } else {
            Vec::new()
        };
        entries.push(TreeEntry {
            relative: path.strip_prefix(root).unwrap().to_path_buf(),
            device: meta.dev(),
            inode: meta.ino(),
            mode: meta.mode(),
            links: meta.nlink(),
            content,
            symlink_target,
        });
        if meta.is_dir() {
            let mut children = fs::read_dir(path)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect::<Vec<_>>();
            children.sort();
            for child in children {
                visit(root, &child, entries);
            }
        }
    }
    let mut entries = Vec::new();
    visit(root, root, &mut entries);
    entries
}

fn assert_cross_target_denied_without_creation(fixture: &Fixture, target: &Path) {
    let before = snapshot_tree(&fixture.path);
    assert!(
        matches!(admit_non_codex_targets(Some("claude_code"), &[target]),
        Err(error) if error == MANAGED_UNSUPPORTED),
        "unexpected target admission: {}",
        target.display()
    );
    assert_eq!(snapshot_tree(&fixture.path), before);
}

#[test]
fn managed_codex_and_legacy_requests_are_rejected_without_creating_missing_roots() {
    let fixture = Fixture::new();
    let targets = ManagedTargets::new(&fixture);
    let scope = test_session(&targets.session);
    let missing = targets.other.join("missing-runtime");
    let before = snapshot_tree(&fixture.path);
    assert!(
        matches!(admit_non_codex_targets(Some("codex"), &[&missing]),
        Err(error) if error == MANAGED_UNSUPPORTED)
    );
    assert!(matches!(admit_non_codex_targets(Some("claude_code"), &[]),
        Err(error) if error == MANAGED_UNSUPPORTED));
    assert!(matches!(admit_legacy_runtime(&missing),
        Err(error) if error == MANAGED_UNSUPPORTED));
    assert_eq!(snapshot_tree(&fixture.path), before);
    assert!(fs::symlink_metadata(&missing).is_err());
    drop((scope, targets));
    fixture.cleanup();
}

#[test]
fn managed_non_codex_disjoint_targets_are_admitted_without_creating_missing_tails() {
    let fixture = Fixture::new();
    let targets = ManagedTargets::new(&fixture);
    let existing = targets.other.join("settings.json");
    let missing = targets
        .other
        .join("not-created")
        .join("nested")
        .join("config.json");
    write_private(&existing, b"{\"synthetic\":true}");
    let scope = test_session(&targets.session);
    let before = snapshot_tree(&fixture.path);
    let accepted = admit_non_codex_targets(Some("claude_code"), &[&existing, &missing]).unwrap();
    accepted.ensure_current().unwrap();
    assert!(Arc::ptr_eq(
        accepted.session.as_ref().unwrap(),
        &targets.session
    ));
    let unspecified_tool = admit_non_codex_targets(None, &[&targets.other]).unwrap();
    unspecified_tool.ensure_current().unwrap();
    assert_eq!(snapshot_tree(&fixture.path), before);
    assert!(!targets.other.join("not-created").exists());
    drop((accepted, unspecified_tool, scope, targets));
    fixture.cleanup();
}

#[test]
fn managed_cross_target_intersections_and_aliases_fail_closed_without_creation() {
    let fixture = Fixture::new();
    let targets = ManagedTargets::new(&fixture);
    let runtime_child = targets.runtime.join("child");
    let profile_child = targets.profile.join("child");
    private_directory(&runtime_child);
    private_directory(&profile_child);
    let runtime_alias = fixture.path.join("codex-alias");
    let profile_alias = fixture.path.join("profile-alias");
    symlink(&targets.runtime, &runtime_alias).unwrap();
    symlink(&targets.profile, &profile_alias).unwrap();
    let scope = test_session(&targets.session);
    let rejected = [
        targets.runtime.clone(),
        targets.profile.clone(),
        fixture.path.clone(),
        runtime_child,
        profile_child,
        runtime_alias.clone(),
        profile_alias.clone(),
        runtime_alias.join("child"),
        profile_alias.join("child"),
        targets
            .runtime
            .join("missing")
            .join("nested")
            .join("payload"),
        targets
            .profile
            .join("missing")
            .join("nested")
            .join("payload"),
        runtime_alias.join("missing").join("payload"),
        profile_alias.join("missing").join("payload"),
    ];
    for target in &rejected {
        assert_cross_target_denied_without_creation(&fixture, target);
    }
    drop((scope, targets));
    fixture.cleanup();
}

#[test]
fn managed_cross_target_hardlinked_and_leaf_symlink_files_are_rejected_without_effects() {
    let fixture = Fixture::new();
    let targets = ManagedTargets::new(&fixture);
    let protected_file = targets.runtime.join("config.toml");
    let unrelated_file = targets.other.join("ordinary.json");
    write_private(&protected_file, b"synthetic = true\n");
    write_private(&unrelated_file, b"{}\n");
    let hardlink = targets.other.join("linked.toml");
    fs::hard_link(&protected_file, &hardlink).unwrap();
    assert_eq!(fs::symlink_metadata(&hardlink).unwrap().nlink(), 2);
    let inward = targets.other.join("points-to-codex");
    let unrelated = targets.other.join("points-to-other");
    let outward = targets.runtime.join("points-outside");
    symlink(&protected_file, &inward).unwrap();
    symlink(&unrelated_file, &unrelated).unwrap();
    symlink(&unrelated_file, &outward).unwrap();
    let scope = test_session(&targets.session);
    for target in [&hardlink, &inward, &unrelated, &outward] {
        assert_cross_target_denied_without_creation(&fixture, target);
    }
    drop((scope, targets));
    fixture.cleanup();
}

#[test]
fn managed_cross_target_without_frozen_runtime_identity_fails_closed() {
    let fixture = Fixture::new();
    let profile = fixture.path.join("profile");
    let other = fixture.path.join("uncreated-other-tool");
    private_directory(&profile);
    let session = session_at(&profile, true);
    let scope = test_session(&session);
    let before = snapshot_tree(&fixture.path);
    assert!(
        matches!(admit_non_codex_targets(Some("claude_code"), &[&other]),
        Err(error) if error == UNAVAILABLE)
    );
    assert_eq!(snapshot_tree(&fixture.path), before);
    drop((scope, session));
    fixture.cleanup();
}

#[test]
fn managed_cross_target_revalidation_rejects_a_new_leaf_symlink_without_effects() {
    let fixture = Fixture::new();
    let targets = ManagedTargets::new(&fixture);
    let target = targets.other.join("not-created-at-admission");
    let scope = test_session(&targets.session);
    let admission = admit_non_codex_targets(Some("claude_code"), &[&target]).unwrap();
    assert!(fs::symlink_metadata(&target).is_err());
    // The test introduces a change after admission. Only revalidation's
    // effects are compared below; no hostile-actor TOCTOU guarantee is implied.
    symlink(&targets.runtime, &target).unwrap();
    let before = snapshot_tree(&fixture.path);
    assert!(matches!(admission.ensure_current(), Err(error) if error == MANAGED_UNSUPPORTED));
    assert_eq!(snapshot_tree(&fixture.path), before);
    drop((admission, scope, targets));
    fixture.cleanup();
}

struct BoundedActor(Option<Child>);

impl Drop for BoundedActor {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let deadline = Instant::now() + WAIT;
            while Instant::now() < deadline {
                if child.try_wait().ok().flatten().is_some() {
                    return;
                }
                thread::yield_now();
            }
            eprintln!("isolated GUI admission actor was not reaped; fixture retained");
        }
    }
}

fn read_bounded(path: &Path) -> String {
    assert!(fs::metadata(path).unwrap().len() <= 16 * 1024);
    let mut text = String::new();
    fs::File::open(path)
        .unwrap()
        .take(16 * 1024)
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn run_isolated_actor(mode: &str) {
    let fixture = Fixture::new();
    let stdout_path = fixture.path.join("actor.stdout");
    let stderr_path = fixture.path.join("actor.stderr");
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&stdout_path)
        .unwrap();
    let error = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&stderr_path)
        .unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            ADMISSION_ACTOR,
            "--nocapture",
            "--test-threads=1",
        ])
        .current_dir(&fixture.path)
        .env_clear()
        .env("AITB_GUI_ADMISSION_FIXTURE", &fixture.path)
        .env("AITB_GUI_ADMISSION_PARENT", fixture.path.parent().unwrap())
        .env("AITB_GUI_ADMISSION_MODE", mode)
        .env("HOME", &fixture.path)
        .env("TMPDIR", &fixture.path)
        .env("XDG_DATA_HOME", &fixture.path)
        .env("XDG_CACHE_HOME", &fixture.path)
        .env("XDG_CONFIG_HOME", &fixture.path)
        .env("AI_TOOLBOX_DATA_DIR", &fixture.path)
        .stdin(Stdio::null())
        .stdout(output)
        .stderr(error)
        .spawn()
        .unwrap();
    let mut actor = BoundedActor(Some(child));
    let deadline = Instant::now() + WAIT;
    let status = loop {
        if let Some(status) = actor.0.as_mut().unwrap().try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "admission actor deadline exceeded; retain fixture"
        );
        thread::yield_now();
    };
    actor.0.take();
    let stdout = read_bounded(&stdout_path);
    let stderr = read_bounded(&stderr_path);
    assert!(
        status.success(),
        "isolated admission actor failed: {stderr}"
    );
    assert!(
        stdout
            .lines()
            .any(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;",)),
        "the exact actor must complete one successful libtest result: {stdout}"
    );
    assert_eq!(
        fs::read(fixture.path.join("actor-complete")).unwrap(),
        MARKER
    );
    if matches!(mode, "usage-managed" | "usage-ordinary") {
        // The real actor has exited and was reaped above. Tauri's mock App
        // retains managed State through its core PathResolver/AppHandle cycle;
        // dropping App alone is not a same-process DB shutdown guarantee.
        // Check actual process-lifetime lease release before deleting its root.
        assert_released(&fixture.path.join("profile"));
    }
    fixture.cleanup();
}

#[test]
fn last_database_clone_retains_ordinary_profile_lease() {
    run_isolated_actor("ordinary");
}

#[test]
fn last_database_clone_retains_managed_profile_lease() {
    run_isolated_actor("managed");
}

#[test]
fn expired_process_registry_fails_closed_without_resetting_once_lock() {
    run_isolated_actor("expired-registry");
}

fn expired_registry_actor(root: &Path) {
    assert!(
        SESSION.get().is_none(),
        "this actor must start in a fresh process"
    );
    let profile = root.join("profile");
    let shadow_profile = root.join("override-profile");
    private_directory(&profile);
    private_directory(&shadow_profile);
    let session = session_at(&profile, false);
    session.install().unwrap();
    let installed = registry_pointer();
    assert_eq!(installed, Some(Arc::as_ptr(&session)));
    let shadow = session_at(&shadow_profile, true);
    {
        let _scope = test_session(&shadow);
        assert!(Arc::ptr_eq(&current_session().unwrap().unwrap(), &shadow));
        assert_eq!(registry_pointer(), installed);
    }
    assert!(Arc::ptr_eq(&current_session().unwrap().unwrap(), &session));
    drop(shadow);
    drop(session);
    assert_eq!(
        registry_pointer(),
        installed,
        "expired installation must not be reset"
    );
    assert!(SESSION.get().unwrap().upgrade().is_none());
    assert!(matches!(current_session(), Err(error) if error == UNAVAILABLE));
    assert!(matches!(begin_codex_write(), Err(error) if error == UNAVAILABLE));
    assert!(matches!(reject_managed_operation(), Err(error) if error == UNAVAILABLE));
    assert_released(&profile);
    assert_released(&shadow_profile);
}

#[test]
#[ignore = "only the bounded synthetic parent may launch this exact actor"]
fn isolated_admission_actor() {
    let root = PathBuf::from(
        std::env::var_os("AITB_GUI_ADMISSION_FIXTURE").expect("explicit synthetic fixture"),
    );
    assert!(root.is_absolute());
    assert_eq!(fs::canonicalize(&root).unwrap(), root);
    let parent = PathBuf::from(
        std::env::var_os("AITB_GUI_ADMISSION_PARENT").expect("explicit private fixture parent"),
    );
    assert!(parent.is_absolute());
    assert_eq!(fs::canonicalize(&parent).unwrap(), parent);
    assert_eq!(root.parent(), Some(parent.as_path()));
    assert_eq!(std::env::temp_dir(), root);
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("ai-toolbox-gui-admission-"));
    assert_eq!(fs::symlink_metadata(&root).unwrap().mode() & 0o7777, 0o700);
    assert_eq!(fs::read(root.join("fixture-marker")).unwrap(), MARKER);
    let mode = std::env::var("AITB_GUI_ADMISSION_MODE").unwrap();
    if mode == "expired-registry" {
        expired_registry_actor(&root);
        write_private(&root.join("actor-complete"), MARKER);
        return;
    }
    if matches!(mode.as_str(), "usage-managed" | "usage-ordinary") {
        official_usage_admission_actor(&root, mode == "usage-managed");
        write_private(&root.join("actor-complete"), MARKER);
        return;
    }
    let managed = match mode.as_str() {
        "ordinary" => false,
        "managed" => true,
        _ => panic!("unknown synthetic admission mode"),
    };
    // A fresh libtest process avoids inheriting another test's global pricing
    // cache. No GUI bootstrap, real HOME lookup or external CLI is involved.
    let cache = root.join("cache");
    private_directory(&cache);
    assert!(ai_toolbox_storage::model_pricing_seed::get_model_pricing_cache_path().is_none());
    ai_toolbox_storage::model_pricing_seed::set_cache_dir(cache.clone());
    assert_eq!(
        ai_toolbox_storage::model_pricing_seed::get_model_pricing_cache_path(),
        Some(cache.join("model_pricing.json"))
    );
    let profile = root.join("profile");
    private_directory(&profile);
    let session = session_at(&profile, managed);
    let weak = Arc::downgrade(&session);
    // Real acquisition happens before initialization of a real synthetic DB.
    let db =
        crate::SqliteDbState::open(profile.join(ai_toolbox_storage::SQLITE_DATABASE_FILE)).unwrap();
    let clone_before_retention = db.clone();
    session.retain_database(&db).unwrap();
    session.retain_database(&clone_before_retention).unwrap();
    let last_clone = db.clone();
    drop(session);
    assert!(
        weak.upgrade().is_none(),
        "DB retains the lease, not the GUI session Arc"
    );
    assert_busy(&profile);
    drop(db);
    assert_busy(&profile);
    drop(clone_before_retention);
    assert_busy(&profile);
    let one = last_clone
        .with_conn(|connection| {
            connection
                .query_row("SELECT 1", [], |row| row.get::<_, i64>(0))
                .map_err(|error| error.to_string())
        })
        .unwrap();
    assert_eq!(
        one, 1,
        "the final clone must still own a live SQLite connection"
    );
    drop(last_clone);
    assert_released(&profile);
    write_private(&root.join("actor-complete"), MARKER);
}

#[test]
fn explicit_worker_session_retains_runtime_without_thread_local_override() {
    let fixture = Fixture::new();
    let runtime = fixture.path.join("worker-runtime");
    private_directory(&runtime);
    let session = fixture.session(false);
    let weak = Arc::downgrade(&session);
    let worker_session = session.clone();
    let worker_runtime = runtime.clone();
    let admission = thread::spawn(move || {
        assert!(TEST_SESSION.with(|slot| slot.borrow().is_none()));
        let admission =
            admit_legacy_runtime_with_session(Some(worker_session), &worker_runtime).unwrap();
        admission.require_runtime(&worker_runtime).unwrap();
        admission
    })
    .join()
    .unwrap();
    drop(session);
    assert!(weak.upgrade().is_some());
    admission.ensure_current().unwrap();
    drop(admission);
    assert!(weak.upgrade().is_none());
    fixture.cleanup();
}

#[test]
fn explicit_managed_worker_rejects_before_missing_runtime_bootstrap() {
    let fixture = Fixture::new();
    let runtime = fixture.path.join("must-remain-absent");
    let session = fixture.session(true);
    let worker_session = session.clone();
    let worker_runtime = runtime.clone();
    thread::spawn(move || {
        let result = admit_legacy_runtime_with_session(Some(worker_session), &worker_runtime);
        assert!(matches!(result, Err(error) if error == MANAGED_UNSUPPORTED));
        assert!(!worker_runtime.exists());
    })
    .join()
    .unwrap();
    assert!(!runtime.exists());
    drop(session);
    fixture.cleanup();
}

#[test]
fn managed_official_usage_rejects_before_persisting_preparation_failure() {
    run_isolated_actor("usage-managed");
}

#[test]
fn ordinary_official_usage_preserves_preparation_failure_contract() {
    run_isolated_actor("usage-ordinary");
}

fn official_usage_admission_actor(root: &Path, managed: bool) {
    use crate::coding::codex::refresh_codex_official_account_limits;
    use crate::db::{
        helpers::{db_get, db_put},
        schema::DbTable,
        SqliteDbState,
    };
    use serde_json::json;
    use tauri::Manager;

    assert!(SESSION.get().is_none());
    let profile = root.join("profile");
    let runtime_root = root.join("synthetic-codex");
    let cache = root.join("pricing-cache");
    for path in [&profile, &runtime_root, &cache] {
        private_directory(path);
    }
    std::env::set_var("CODEX_HOME", &runtime_root);
    assert!(ai_toolbox_storage::model_pricing_seed::get_model_pricing_cache_path().is_none());
    ai_toolbox_storage::model_pricing_seed::set_cache_dir(cache);
    let session = session_at(&profile, managed);
    session.install().unwrap();
    let database =
        SqliteDbState::open(profile.join(ai_toolbox_storage::SQLITE_DATABASE_FILE)).unwrap();
    let app = tauri::test::mock_builder()
        .manage(database)
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let state = app.state::<SqliteDbState>();
    session.retain_database(state.inner()).unwrap();
    let db = state.db();
    db.with_conn(|conn| {
        db_put(conn, DbTable::CodexCommonConfig, "common", &json!({
            "config": "", "root_dir": runtime_root, "updated_at": "synthetic-before"
        }))?;
        db_put(conn, DbTable::CodexProvider, "synthetic-provider", &json!({
            "name": "Synthetic official", "category": "official", "is_applied": false,
            "settings_config": "{}", "created_at": "synthetic", "updated_at": "synthetic"
        }))?;
        db_put(conn, DbTable::CodexOfficialAccount, "synthetic-account", &json!({
            "provider_id": "synthetic-provider", "name": "Synthetic account",
            "auth_snapshot": null, "is_applied": false,
            "credits": {"hasCredits": true, "unlimited": false, "balance": 42.5},
            "last_limits_fetched_at": "synthetic-prior-success", "last_error": "synthetic-prior-error",
            "created_at": "synthetic", "updated_at": "synthetic-before"
        }))?;
        Ok(())
    }).unwrap();
    let snapshot = || {
        db.with_conn(|conn| db_get(conn, DbTable::CodexOfficialAccount, "synthetic-account"))
            .unwrap()
            .unwrap()
    };
    let before = snapshot();
    assert!(before["auth_snapshot"].is_null());
    assert_eq!(before["credits"]["balance"], json!(42.5));
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    // No token or auth.json exists. The current credential preparation rejects
    // the missing/empty persisted snapshot before OAuth or usage requests.
    // Keep the production error contract introduced by credential recovery;
    // this is source-grounded avoidance, not runtime HTTP/DNS monitoring.
    // It is a real production command call, not a copied persistence helper.
    assert_busy(&profile);
    let result = executor.block_on(refresh_codex_official_account_limits(
        state.clone(),
        app.handle().clone(),
        "synthetic-provider".to_owned(),
        "synthetic-account".to_owned(),
    ));
    let after = snapshot();
    assert_busy(&profile);
    if managed {
        // Expected RED on the unmodified entry: preparation failure currently
        // writes credits/error/updated_at before any managed admission check.
        assert_eq!(
            after, before,
            "managed usage request mutated its stored account"
        );
        assert_eq!(result.unwrap_err(), MANAGED_UNSUPPORTED);
    } else {
        assert_eq!(result.unwrap_err(), "Official account snapshot is missing");
        assert!(after["credits"].is_null());
        assert_eq!(
            after["last_limits_fetched_at"],
            before["last_limits_fetched_at"]
        );
        assert_eq!(after["last_error"], "Official account snapshot is missing");
        assert_ne!(after["updated_at"], before["updated_at"]);
    }
    assert!(!runtime_root.join("auth.json").exists());
    assert!(!runtime_root.join("config.toml").exists());
    drop(state);
    drop(app);
    drop(executor);
    drop(session);
    // The bounded parent checks release after this process exits normally.
    // Do not use deprecated unmanage or claim MockRuntime App drop releases DB.
}

// Missing-parent refusal is an admission boundary, not an applied-provider
// or Skills/history recovery-eligibility check. No settings command or DB
// mutation is invoked here.
#[test]
fn ordinary_missing_parent_rejects_before_runtime_lease_retention() {
    let fixture = Fixture::new();
    let session = fixture.session(false);
    let scope = test_session(&session);
    let missing_parent = fixture.path.join("absent-parent");
    let missing_root = missing_parent.join("codex");
    assert!(session.ordinary_roots.lock().unwrap().is_empty());
    let before = snapshot_tree(&fixture.path);

    let result = admit_legacy_runtime(&missing_root);
    assert!(matches!(result, Err(error) if
        error == "The Codex runtime cannot be coordinated; no business write was started"));
    assert!(
        session.ordinary_roots.lock().unwrap().is_empty(),
        "a rejected missing-parent request must not retain a runtime lease"
    );
    assert_eq!(snapshot_tree(&fixture.path), before);
    assert!(matches!(fs::symlink_metadata(&missing_parent),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound));

    // The same live ordinary session can admit a healthy root afterwards:
    // an expired/missing session must not satisfy the negative control.
    let healthy_root = fixture.path.join("healthy-runtime");
    private_directory(&healthy_root);
    let admitted = admit_legacy_runtime(&healthy_root).unwrap();
    admitted.ensure_current().unwrap();
    assert_eq!(session.ordinary_roots.lock().unwrap().len(), 1);
    assert!(!healthy_root.join("auth.json").exists());
    assert!(!healthy_root.join("config.toml").exists());
    drop((admitted, scope, session));
    fixture.cleanup();
}
