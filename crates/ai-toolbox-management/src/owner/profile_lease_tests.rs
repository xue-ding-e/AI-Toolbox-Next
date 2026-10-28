//! Synthetic draft coverage for ordinary/managed startup coordination. These
//! tests do not launch the GUI, certify mount changes or close writer admission.

use super::*;
use ai_toolbox_storage::{SqliteDbState, SQLITE_DATABASE_FILE};
use std::{
    fs::{self, DirBuilder, OpenOptions},
    io::Read,
    os::unix::fs::{symlink, DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const ACTOR: &str = "owner::profile_lease_tests::profile_lease_actor";
const FIXTURE_MARKER: &[u8] = b"synthetic-profile-lease-fixture-v1";

struct Fixture {
    root: Option<tempfile::TempDir>,
    cleanup_safe: Arc<AtomicBool>,
}

impl Fixture {
    fn path(&self) -> &Path { self.root.as_ref().unwrap().path() }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if !self.cleanup_safe.load(Ordering::SeqCst) {
            if let Some(root) = self.root.take() {
                let _ = root.keep();
                eprintln!("retained isolated fixture because its actor was not reaped");
            }
        }
    }
}

fn fixture() -> Fixture {
    let root = tempfile::Builder::new().prefix("profile-lease-")
        .permissions(fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    fs::write(root.path().join("fixture-marker"), FIXTURE_MARKER).unwrap();
    Fixture { root: Some(root), cleanup_safe: Arc::new(AtomicBool::new(true)) }
}

fn directory(parent: &Path, name: &str, mode: u32) -> PathBuf {
    let path = parent.join(name);
    DirBuilder::new().mode(mode).create(&path).unwrap();
    // Explicit fixture permissions, never the process-wide umask.
    fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
    path
}

fn strict_profile(path: &Path) -> ProfileIdentity {
    ProfileIdentity::existing(path).expect(
        "strict fixtures require an isolated TMPDIR on a supported local filesystem",
    )
}

fn write_lock(path: &Path) {
    let file = OpenOptions::new().write(true).create_new(true).mode(0o600)
        .open(path.join(PROFILE_LEASE_FILE)).unwrap();
    file.set_permissions(fs::Permissions::from_mode(0o600)).unwrap();
}

struct Actor {
    child: Option<Child>,
    cleanup_safe: Arc<AtomicBool>,
}

impl Drop for Actor {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                if child.try_wait().ok().flatten().is_some() { return; }
                std::thread::sleep(Duration::from_millis(10));
            }
            // Do not delete an isolated fixture while a child may still use it.
            self.cleanup_safe.store(false, Ordering::SeqCst);
            eprintln!("synthetic actor could not be reaped within cleanup deadline");
        }
    }
}

fn probe(fixture: &Fixture, profile: &Path, role: &str) {
    let actor = start_probe(fixture, profile, None, role);
    finish_probe(fixture, actor, role);
}

fn probe_runtime(fixture: &Fixture, profile: &Path, runtime: &Path, role: &str) {
    let actor = start_probe(fixture, profile, Some(runtime), role);
    finish_probe(fixture, actor, role);
}

fn start_probe(fixture: &Fixture, profile: &Path, runtime: Option<&Path>, role: &str) -> Actor {
    let root = fixture.path();
    let output_path = root.join(format!("{role}.stdout"));
    let output = OpenOptions::new().write(true).create_new(true).mode(0o600)
        .open(&output_path).unwrap();
    let error = OpenOptions::new().write(true).create_new(true).mode(0o600)
        .open(root.join(format!("{role}.stderr"))).unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args(["--ignored", "--exact", ACTOR, "--nocapture", "--test-threads=1"])
        .current_dir(root).env_clear()
        .env("AITB_PROFILE_LEASE_FIXTURE", root)
        .env("AITB_PROFILE_LEASE_TARGET", profile)
        .env("AITB_PROFILE_LEASE_ROLE", role)
        .env("HOME", root).env("TMPDIR", root)
        .env("AI_TOOLBOX_DATA_DIR", profile)
        .stdin(Stdio::null()).stdout(output).stderr(error);
    if let Some(runtime) = runtime { command.env("AITB_RUNTIME_LEASE_TARGET", runtime); }
    let child = command.spawn().unwrap();
    Actor { child: Some(child), cleanup_safe: fixture.cleanup_safe.clone() }
}

fn finish_probe(fixture: &Fixture, mut actor: Actor, role: &str) {
    let root = fixture.path();
    let output_path = root.join(format!("{role}.stdout"));
    let deadline = Instant::now() + Duration::from_secs(8);
    let status = loop {
        if let Some(status) = actor.child.as_mut().unwrap().try_wait().unwrap() { break status; }
        assert!(Instant::now() < deadline, "synthetic actor exceeded its deadline");
        std::thread::sleep(Duration::from_millis(10));
    };
    actor.child.take();
    let mut diagnostic = String::new();
    fs::File::open(root.join(format!("{role}.stderr"))).unwrap()
        .take(16 * 1024).read_to_string(&mut diagnostic).unwrap();
    assert!(status.success(), "synthetic actor failed: {diagnostic}");
    assert!(fs::metadata(&output_path).unwrap().len() <= 16 * 1024,
        "synthetic actor exceeded its diagnostic budget");
    let mut output = String::new();
    fs::File::open(output_path).unwrap().take(16 * 1024).read_to_string(&mut output).unwrap();
    assert!(output.lines().any(|line| line.starts_with(
        "test result: ok. 1 passed; 0 failed; 0 ignored;",
    )), "the actor must report an executed, successful libtest result");
    assert_eq!(fs::read(root.join(format!("complete-{role}"))).unwrap(), FIXTURE_MARKER);
}

fn await_fixture_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while !path.exists() {
        assert!(Instant::now() < deadline, "synthetic rendezvous exceeded its deadline");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn publish_fixture_outcome(root: &Path, role: &str, outcome: &[u8]) {
    // Publish complete rendezvous contents, rather than exposing a created
    // but not yet written status file to the polling parent.
    let staging = root.join(format!("pending-outcome-{role}"));
    fs::write(&staging, outcome).unwrap();
    fs::rename(staging, root.join(format!("outcome-{role}"))).unwrap();
}

#[test]
#[ignore = "only the bounded parent fixtures may launch this actor"]
fn profile_lease_actor() {
    let root = PathBuf::from(std::env::var_os("AITB_PROFILE_LEASE_FIXTURE").expect("isolated fixture"));
    let profile = PathBuf::from(std::env::var_os("AITB_PROFILE_LEASE_TARGET").expect("isolated profile"));
    let role = std::env::var("AITB_PROFILE_LEASE_ROLE").expect("fixture role");
    assert!(root.is_absolute() && profile.is_absolute());
    assert_eq!(fs::read(root.join("fixture-marker")).unwrap(), FIXTURE_MARKER);
    assert!(fs::canonicalize(&profile).unwrap().starts_with(fs::canonicalize(&root).unwrap()));
    let runtime = std::env::var_os("AITB_RUNTIME_LEASE_TARGET").map(PathBuf::from);
    if let Some(runtime) = &runtime {
        assert!(runtime.is_absolute() && runtime.file_name().is_some());
        assert!(fs::canonicalize(runtime.parent().unwrap()).unwrap()
            .starts_with(fs::canonicalize(&root).unwrap()));
        if runtime.exists() {
            assert!(fs::canonicalize(runtime).unwrap().starts_with(fs::canonicalize(&root).unwrap()));
        }
    }
    match role.as_str() {
        "managed-busy" => assert!(matches!(
            ProfileOwner::acquire(strict_profile(&profile)), Err(Error::OwnerBusy),
        )),
        "ordinary-busy" => assert!(matches!(
            ProfileLeaseStartup::acquire(&profile), Err(Error::OwnerBusy),
        )),
        "ordinary-available" => {
            let lease = ProfileLeaseStartup::acquire(&profile).unwrap().into_ordinary();
            lease.ensure_current().unwrap();
            drop(lease);
        }
        "runtime-managed-busy" => {
            let owner = ProfileOwner::acquire(strict_profile(&profile)).unwrap();
            let writer = owner.try_writer().unwrap();
            let resource = RuntimeResourceIdentity::existing(runtime.as_ref().unwrap()).unwrap();
            assert!(matches!(writer.acquire_resources(vec![resource]), Err(Error::ResourceBusy)));
        }
        "runtime-ordinary-busy" | "runtime-ordinary-available" => {
            let lease = ProfileLeaseStartup::acquire(&profile).unwrap().into_ordinary();
            let result = OrdinaryRuntimeLeaseGroup::acquire(&lease, vec![runtime.clone().unwrap()]);
            if role == "runtime-ordinary-busy" {
                assert!(matches!(result, Err(Error::ResourceBusy)));
            } else {
                let resources = result.unwrap();
                resources.require_runtime(runtime.as_ref().unwrap()).unwrap();
            }
        }
        "bootstrap-a" | "bootstrap-b" => {
            let lease = ProfileLeaseStartup::acquire(&profile).unwrap().into_ordinary();
            fs::write(root.join(format!("ready-{role}")), FIXTURE_MARKER).unwrap();
            await_fixture_file(&root.join("start-bootstrap"));
            let runtime = runtime.as_ref().unwrap();
            let outcome = OrdinaryRuntimeLeaseGroup::acquire_for_writer_with_bootstrap(&lease, runtime);
            match outcome {
                Ok(resources) => {
                    resources.require_runtime(runtime).unwrap();
                    // Only the actor with the actual runtime lease performs
                    // the synthetic business effect; the other stays inert.
                    fs::write(runtime.join("synthetic-effect"), role.as_bytes()).unwrap();
                    publish_fixture_outcome(&root, &role, b"held");
                    await_fixture_file(&root.join("release-bootstrap"));
                    resources.ensure_current().unwrap();
                }
                Err(Error::ResourceBusy) => {
                    publish_fixture_outcome(&root, &role, b"busy");
                }
                Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    // An exclusive fixed-lock creation race is fail-closed,
                    // not a retry on another inode or a successful effect.
                    publish_fixture_outcome(&root, &role, b"creation-race");
                }
                Err(error) => panic!("unexpected bootstrap outcome: {error}"),
            }
        }
        _ => panic!("unknown fixture role"),
    }
    fs::write(root.join(format!("complete-{role}")), FIXTURE_MARKER).unwrap();
}

#[test]
fn ordinary_0755_alias_and_chmod_to_safe_still_block_managed_process() {
    let root = fixture();
    let ancestor = directory(root.path(), "ancestor", 0o700);
    let profile = directory(&ancestor, "profile", 0o755);
    strict_profile(&profile);
    let alias = root.path().join("profile-alias");
    symlink(&profile, &alias).unwrap();
    fs::set_permissions(&ancestor, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(matches!(ProfileIdentity::existing(&profile), Err(Error::UnsafeDirectory)));
    let ordinary = ProfileLeaseStartup::acquire(&alias).unwrap().into_ordinary();
    assert_eq!(ordinary.canonical_path(), fs::canonicalize(&profile).unwrap().as_path());
    let inode = fs::metadata(profile.join(PROFILE_LEASE_FILE)).unwrap().ino();
    fs::set_permissions(&ancestor, fs::Permissions::from_mode(0o700)).unwrap();
    ordinary.ensure_current().unwrap();
    probe(&root, &profile, "managed-busy");
    probe(&root, &alias, "ordinary-busy");
    assert_eq!(fs::metadata(profile.join(PROFILE_LEASE_FILE)).unwrap().ino(), inode);
    drop(ordinary);
    probe(&root, &profile, "ordinary-available");
    assert_eq!(fs::metadata(profile.join(PROFILE_LEASE_FILE)).unwrap().ino(), inode);
}

#[test]
fn managed_retention_uses_the_same_allocation_and_clone_gate() {
    let root = fixture();
    let profile = directory(root.path(), "profile", 0o755);
    strict_profile(&profile);
    let startup = ProfileLeaseStartup::acquire(&profile).unwrap();
    // Private access proves the same core before/after consuming the token;
    // production callers cannot clone a token or promote an ordinary lease.
    let allocation = startup.lease.lifetime_owner();
    let owner = startup.into_managed().unwrap();
    assert!(Arc::ptr_eq(&allocation, &owner.lifetime_owner()));
    let clone = owner.clone();
    assert_eq!(owner.epoch(), clone.epoch());
    probe(&root, &profile, "ordinary-busy");
    let writer = owner.try_writer().unwrap();
    assert!(matches!(clone.try_writer(), Err(Error::WriterBusy)));
    drop(writer);
    let db = SqliteDbState::open(profile.join(SQLITE_DATABASE_FILE)).unwrap();
    owner.profile_lease().retain_database(&db).unwrap();
    crate::codex_service::ContextService::attach(&owner, &db).unwrap();
    drop(db);
    drop(clone);
    drop(owner);
    drop(allocation);
    probe(&root, &profile, "ordinary-available");
}

#[test]
fn managed_conversion_preserves_strict_ancestor_and_write_requirements() {
    for wide_ancestor in [true, false] {
        let root = fixture();
        let ancestor = directory(root.path(), "ancestor", 0o700);
        let profile = directory(&ancestor, "profile", 0o755);
        strict_profile(&profile);
        write_lock(&profile);
        let inode = fs::metadata(profile.join(PROFILE_LEASE_FILE)).unwrap().ino();
        if wide_ancestor {
            fs::set_permissions(&ancestor, fs::Permissions::from_mode(0o777)).unwrap();
        } else {
            fs::set_permissions(&profile, fs::Permissions::from_mode(0o555)).unwrap();
        }
        let startup = ProfileLeaseStartup::acquire(&profile).unwrap();
        let outcome = startup.into_managed();
        // Fixture cleanup, not product repair of an existing user directory.
        fs::set_permissions(&ancestor, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&profile, fs::Permissions::from_mode(0o755)).unwrap();
        if wide_ancestor {
            assert!(matches!(outcome, Err(Error::UnsafeDirectory)));
        } else {
            assert!(matches!(outcome, Err(Error::Io(error))
                if error.kind() == std::io::ErrorKind::PermissionDenied));
        }
        assert_eq!(fs::metadata(profile.join(PROFILE_LEASE_FILE)).unwrap().ino(), inode);
        assert!(!profile.join(SQLITE_DATABASE_FILE).exists());
    }
}

#[test]
fn last_database_clone_keeps_ordinary_lease_after_session_drop() {
    let root = fixture();
    let profile = directory(root.path(), "profile", 0o755);
    let lease = ProfileLeaseStartup::acquire(&profile).unwrap().into_ordinary();
    let db = SqliteDbState::open(profile.join(SQLITE_DATABASE_FILE)).unwrap();
    let before = db.clone();
    lease.retain_database(&db).unwrap();
    lease.retain_database(&before).unwrap();
    let after = db.clone();
    drop(lease);
    drop(db);
    drop(before);
    probe(&root, &profile, "ordinary-busy");
    after.with_conn(|connection| {
        connection.execute_batch("CREATE TABLE synthetic_retention(value INTEGER);")
            .map_err(|error| error.to_string())
    }).unwrap();
    drop(after);
    probe(&root, &profile, "ordinary-available");
}

#[test]
fn ordinary_fixed_lock_rejects_abnormal_objects_without_repair() {
    for kind in ["mode", "hardlink", "symlink", "directory", "socket", "fifo"] {
        let root = fixture();
        let profile = directory(root.path(), "profile", 0o755);
        let lock = profile.join(PROFILE_LEASE_FILE);
        let mut socket = None;
        match kind {
            "mode" => { fs::write(&lock, b"unchanged").unwrap(); fs::set_permissions(&lock, fs::Permissions::from_mode(0o644)).unwrap(); }
            "hardlink" => { write_lock(&profile); fs::hard_link(&lock, root.path().join("second-link")).unwrap(); }
            "symlink" => symlink("missing", &lock).unwrap(),
            "directory" => { fs::create_dir(&lock).unwrap(); }
            "socket" => { socket = Some(std::os::unix::net::UnixListener::bind(&lock).unwrap()); }
            "fifo" => {
                use std::os::unix::ffi::OsStrExt;
                let name = std::ffi::CString::new(lock.as_os_str().as_bytes()).unwrap();
                // SAFETY: A NUL-terminated path inside the isolated fixture.
                assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
            }
            _ => unreachable!(),
        }
        let before = fs::symlink_metadata(&lock).unwrap();
        assert!(matches!(ProfileLeaseStartup::acquire(&profile), Err(Error::UnsafeLockFile)));
        let after = fs::symlink_metadata(&lock).unwrap();
        assert_eq!((before.dev(), before.ino(), before.mode(), before.nlink()),
            (after.dev(), after.ino(), after.mode(), after.nlink()));
        if kind == "mode" { assert_eq!(fs::read(&lock).unwrap(), b"unchanged"); }
        assert!(!profile.join(SQLITE_DATABASE_FILE).exists());
        drop(socket);
    }
}

#[test]
fn ordinary_detects_profile_and_lock_replacement_without_rebinding() {
    for replacement_kind in ["profile", "ancestor", "lock"] {
        let root = fixture();
        let ancestor = directory(root.path(), "ancestor", 0o700);
        let profile = directory(&ancestor, "profile", 0o755);
        strict_profile(&profile);
        let lease = ProfileLeaseStartup::acquire(&profile).unwrap().into_ordinary();
        let old_lock = fs::metadata(profile.join(PROFILE_LEASE_FILE)).unwrap().ino();
        match replacement_kind {
            "profile" => {
                fs::rename(&profile, root.path().join("old-profile")).unwrap();
                directory(&ancestor, "profile", 0o755);
            }
            "ancestor" => {
                fs::rename(&ancestor, root.path().join("old-ancestor")).unwrap();
                directory(root.path(), "ancestor", 0o700);
                directory(&ancestor, "profile", 0o755);
            }
            _ => {
                fs::rename(profile.join(PROFILE_LEASE_FILE), profile.join("old-lock")).unwrap();
            }
        }
        write_lock(&profile);
        let replacement = fs::metadata(profile.join(PROFILE_LEASE_FILE)).unwrap().ino();
        assert_ne!(old_lock, replacement);
        assert!(matches!(lease.ensure_current(), Err(Error::IdentityChanged)));
        // A replacement really is another lease domain. This negative control
        // makes it impossible to mistake startup coordination for containment
        // of unguarded ordinary writers following the replaced pathname.
        let successor = ProfileOwner::acquire(strict_profile(&profile)).unwrap();
        successor.ensure_current().unwrap();
        assert!(matches!(lease.ensure_current(), Err(Error::IdentityChanged)));
        drop(successor);
        drop(lease);
        assert_eq!(fs::metadata(profile.join(PROFILE_LEASE_FILE)).unwrap().ino(), replacement);
        // This does NOT prove protection against a rename between an identity
        // check and an ordinary path-based writer's later effect.
    }
}

#[test]
fn ordinary_does_not_apply_managed_filesystem_policy() {
    let root = fixture();
    let profile = directory(root.path(), "profile", 0o755);
    let lease = ProfileLeaseStartup::acquire(&profile).unwrap().into_ordinary();
    lease.ensure_current().unwrap();
    match ProfileIdentity::existing(&profile) {
        Err(Error::UnsupportedFilesystem) => {
            eprintln!("ordinary lease exercised on an actually unsupported managed filesystem");
        }
        Ok(_) => {
            eprintln!("actual fixture filesystem is managed-supported; unknown-FS coverage not claimed");
        }
        Err(error) => panic!("unexpected strict fixture failure: {error}"),
    }
    drop(lease);
}

#[test]
fn ordinary_existing_lock_does_not_prejudge_directory_write_bits() {
    let root = fixture();
    let profile = directory(root.path(), "profile", 0o755);
    write_lock(&profile);
    let inode = fs::metadata(profile.join(PROFILE_LEASE_FILE)).unwrap().ino();
    fs::set_permissions(&profile, fs::Permissions::from_mode(0o555)).unwrap();
    let outcome = ProfileLeaseStartup::acquire(&profile);
    // Restore only this synthetic directory before assertions/cleanup. This
    // is a directory-mode fixture, not a read-only mount or DB-write claim.
    fs::set_permissions(&profile, fs::Permissions::from_mode(0o755)).unwrap();
    let lease = outcome.unwrap().into_ordinary();
    assert_eq!(fs::metadata(profile.join(PROFILE_LEASE_FILE)).unwrap().ino(), inode);
    drop(lease);
}

#[test]
fn ordinary_runtime_alias_and_chmod_contend_with_managed_across_profiles() {
    let root = fixture();
    let profile = directory(root.path(), "ordinary-profile", 0o755);
    let other = directory(root.path(), "other-profile", 0o755);
    let ancestor = directory(root.path(), "runtime-parent", 0o700);
    let runtime = directory(&ancestor, "runtime", 0o755);
    strict_profile(&runtime);
    let alias = root.path().join("runtime-alias");
    symlink(&runtime, &alias).unwrap();
    fs::set_permissions(&ancestor, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(matches!(RuntimeResourceIdentity::existing(&runtime), Err(Error::UnsafeDirectory)));
    let lease = ProfileLeaseStartup::acquire(&profile).unwrap().into_ordinary();
    let resources = OrdinaryRuntimeLeaseGroup::acquire(&lease, vec![alias.clone(), runtime.clone()]).unwrap();
    assert_eq!(resources.directories.len(), 1);
    resources.require_runtime(&alias).unwrap();
    let lock = runtime.join(RESOURCE_LEASE_FILE);
    let inode = fs::metadata(&lock).unwrap().ino();
    fs::set_permissions(&ancestor, fs::Permissions::from_mode(0o700)).unwrap();
    probe_runtime(&root, &other, &runtime, "runtime-managed-busy");
    probe_runtime(&root, &other, &alias, "runtime-ordinary-busy");
    // No management owner is needed for a normal GUI writer. This positive
    // control must still perform its real synthetic effect under the group.
    resources.require_runtime(&runtime).unwrap();
    fs::write(runtime.join("ordinary-effect"), b"ordinary GUI success").unwrap();
    assert_eq!(fs::read(runtime.join("ordinary-effect")).unwrap(), b"ordinary GUI success");
    drop(lease);
    probe(&root, &profile, "ordinary-busy");
    drop(resources);
    probe(&root, &profile, "ordinary-available");
    probe_runtime(&root, &other, &runtime, "runtime-ordinary-available");
    assert_eq!(fs::metadata(lock).unwrap().ino(), inode);
}

#[test]
fn strict_managed_runtime_blocks_an_ordinary_profile_process() {
    let root = fixture();
    let profile = directory(root.path(), "managed-profile", 0o755);
    let other = directory(root.path(), "ordinary-profile", 0o755);
    let runtime = directory(root.path(), "runtime", 0o755);
    let owner = ProfileOwner::acquire(strict_profile(&profile)).unwrap();
    let writer = owner.try_writer().unwrap();
    let resources = writer.acquire_resources(vec![RuntimeResourceIdentity::existing(&runtime).unwrap()]).unwrap();
    let inode = fs::metadata(runtime.join(RESOURCE_LEASE_FILE)).unwrap().ino();
    probe_runtime(&root, &other, &runtime, "runtime-ordinary-busy");
    drop(resources);
    probe_runtime(&root, &other, &runtime, "runtime-ordinary-available");
    assert_eq!(fs::metadata(runtime.join(RESOURCE_LEASE_FILE)).unwrap().ino(), inode);
}

#[test]
fn ordinary_runtime_group_sorts_deduplicates_and_releases_partial_failure() {
    let root = fixture();
    let profile = directory(root.path(), "profile", 0o755);
    let first = directory(root.path(), "first", 0o755);
    let second = directory(root.path(), "second", 0o755);
    let lease = ProfileLeaseStartup::acquire(&profile).unwrap().into_ordinary();
    let group = OrdinaryRuntimeLeaseGroup::acquire(&lease, vec![second, first]).unwrap();
    let keys: Vec<_> = group.directories.iter().map(|directory|
        (directory.key.device, directory.key.inode)).collect();
    assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
    let low = group.directories[0].path.clone();
    let high = group.directories[1].path.clone();
    let inode = fs::metadata(low.join(RESOURCE_LEASE_FILE)).unwrap().ino();
    drop(group);
    let alias = root.path().join("low-alias");
    symlink(&low, &alias).unwrap();
    let held = OrdinaryRuntimeLeaseGroup::acquire(&lease, vec![high.clone()]).unwrap();
    assert!(matches!(OrdinaryRuntimeLeaseGroup::acquire(&lease, vec![high, alias, low.clone()]),
        Err(Error::ResourceBusy)));
    // The first, lower-key lease was acquired before the busy second one;
    // this real reacquisition proves the failed group released its partial set.
    let available = OrdinaryRuntimeLeaseGroup::acquire(&lease, vec![low.clone(), low.clone()]).unwrap();
    assert_eq!(available.directories.len(), 1);
    assert_eq!(fs::metadata(low.join(RESOURCE_LEASE_FILE)).unwrap().ino(), inode);
    assert!(!low.join(PROFILE_LEASE_FILE).exists());
    assert!(matches!(available.require_runtime(root.path()), Err(Error::IdentityChanged)));
    drop(available);
    drop(held);
}

#[test]
fn ordinary_runtime_group_rejects_replacement_and_unsafe_lock_without_repair() {
    for replacement in ["runtime", "lock", "profile"] {
        let root = fixture();
        let profile = directory(root.path(), "profile", 0o755);
        let other = directory(root.path(), "other-profile", 0o755);
        let runtime = directory(root.path(), "runtime", 0o755);
        let lease = ProfileLeaseStartup::acquire(&profile).unwrap().into_ordinary();
        let group = OrdinaryRuntimeLeaseGroup::acquire(&lease, vec![runtime.clone()]).unwrap();
        match replacement {
            "runtime" => {
                fs::rename(&runtime, root.path().join("old-runtime")).unwrap();
                directory(root.path(), "runtime", 0o755);
            }
            "lock" => {
                fs::rename(runtime.join(RESOURCE_LEASE_FILE), runtime.join("old-lock")).unwrap();
                let file = OpenOptions::new().write(true).create_new(true).mode(0o600)
                    .open(runtime.join(RESOURCE_LEASE_FILE)).unwrap();
                file.set_permissions(fs::Permissions::from_mode(0o600)).unwrap();
            }
            _ => {
                fs::rename(&profile, root.path().join("old-profile")).unwrap();
                directory(root.path(), "profile", 0o755);
            }
        }
        assert!(group.ensure_current().is_err());
        assert!(group.require_runtime(&runtime).is_err());
        assert!(!runtime.join("ordinary-effect").exists());
        if replacement != "profile" {
            // A new physical inode really can be acquired. The old group
            // must reject, not rebind; checks are not malicious-race isolation.
            let successor = ProfileLeaseStartup::acquire(&other).unwrap().into_ordinary();
            let successor_group = OrdinaryRuntimeLeaseGroup::acquire(&successor, vec![runtime.clone()]).unwrap();
            successor_group.ensure_current().unwrap();
            assert!(group.ensure_current().is_err());
        }
    }

    let root = fixture();
    let profile = directory(root.path(), "profile", 0o755);
    let runtime = directory(root.path(), "runtime", 0o755);
    let lock = runtime.join(RESOURCE_LEASE_FILE);
    fs::write(&lock, b"do not repair").unwrap();
    fs::set_permissions(&lock, fs::Permissions::from_mode(0o644)).unwrap();
    let before = fs::symlink_metadata(&lock).unwrap();
    let lease = ProfileLeaseStartup::acquire(&profile).unwrap().into_ordinary();
    assert!(matches!(OrdinaryRuntimeLeaseGroup::acquire(&lease, vec![runtime]), Err(Error::UnsafeLockFile)));
    let after = fs::symlink_metadata(&lock).unwrap();
    assert_eq!((before.dev(), before.ino(), before.mode()), (after.dev(), after.ino(), after.mode()));
    assert_eq!(fs::read(lock).unwrap(), b"do not repair");
}

#[test]
fn ordinary_existing_runtime_api_never_bootstraps_and_rejects_relative_paths() {
    let root = fixture();
    let profile = directory(root.path(), "profile", 0o755);
    let runtime = directory(root.path(), "runtime", 0o755);
    let missing = root.path().join("missing");
    let lease = ProfileLeaseStartup::acquire(&profile).unwrap().into_ordinary();
    assert!(matches!(OrdinaryRuntimeLeaseGroup::acquire(&lease, vec![runtime.clone(), missing.clone()]),
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound));
    // All roots are resolved first, so a missing member also leaves an earlier
    // valid root untouched. Acquiring a successful lease still creates a lock,
    // and therefore is NOT a read-only inspect/status API.
    assert!(!runtime.join(RESOURCE_LEASE_FILE).exists());
    assert!(!missing.exists());
    assert!(matches!(OrdinaryRuntimeLeaseGroup::acquire(&lease, vec![PathBuf::from("relative")]),
        Err(Error::RelativePath)));
    assert!(!root.path().join("relative").exists());
}

#[test]
fn runtime_identity_inspection_does_not_create_roots_or_coordination_files() {
    let root = fixture();
    let runtime = directory(root.path(), "runtime", 0o755);
    let identity = RuntimeResourceIdentity::existing(&runtime).expect(
        "strict fixtures require an isolated TMPDIR on a supported local filesystem",
    );
    identity.inspection_directory().unwrap();
    assert_eq!(fs::read_dir(&runtime).unwrap().count(), 0);
    let missing = root.path().join("missing-runtime");
    assert!(matches!(RuntimeResourceIdentity::existing(&missing), Err(Error::InvalidProfile)));
    assert!(!missing.exists());
    assert!(!root.path().join(PROFILE_LEASE_FILE).exists());
    assert!(!root.path().join(RESOURCE_LEASE_FILE).exists());
}

#[test]
fn ordinary_runtime_alias_retargeting_requires_the_selected_physical_root() {
    let root = fixture();
    let profile = directory(root.path(), "profile", 0o755);
    let runtime = directory(root.path(), "runtime", 0o755);
    let other = directory(root.path(), "other-runtime", 0o755);
    let alias = root.path().join("selected-runtime");
    symlink(&runtime, &alias).unwrap();
    let lease = ProfileLeaseStartup::acquire(&profile).unwrap().into_ordinary();
    let group = OrdinaryRuntimeLeaseGroup::acquire(&lease, vec![alias.clone()]).unwrap();
    fs::rename(&alias, root.path().join("old-alias")).unwrap();
    symlink(&other, &alias).unwrap();
    group.ensure_current().unwrap();
    assert!(matches!(group.require_runtime(&alias), Err(Error::IdentityChanged)));
    group.require_runtime(&runtime).unwrap();
    assert!(!other.join(RESOURCE_LEASE_FILE).exists());
}

#[test]
fn explicit_writer_bootstrap_creates_only_one_leaf_and_preserves_existing_objects() {
    let root = fixture();
    let profile = directory(root.path(), "profile", 0o755);
    let runtime = root.path().join("new-runtime");
    let lease = ProfileLeaseStartup::acquire(&profile).unwrap().into_ordinary();
    let group = OrdinaryRuntimeLeaseGroup::acquire_for_writer_with_bootstrap(&lease, &runtime).unwrap();
    group.require_runtime(&runtime).unwrap();
    let names: Vec<_> = fs::read_dir(&runtime).unwrap().map(|entry| entry.unwrap().file_name()).collect();
    assert_eq!(names, vec![std::ffi::OsString::from(RESOURCE_LEASE_FILE)]);
    let before = fs::metadata(&runtime).unwrap();
    assert!(matches!(OrdinaryRuntimeLeaseGroup::acquire_for_writer_with_bootstrap(&lease, &runtime),
        Err(Error::ResourceBusy)));
    let after = fs::metadata(&runtime).unwrap();
    assert_eq!((before.dev(), before.ino(), before.mode()), (after.dev(), after.ino(), after.mode()));
    assert!(!runtime.join("config.toml").exists());
    assert!(!runtime.join("auth.json").exists());
    assert!(!profile.join(SQLITE_DATABASE_FILE).exists());
    let alias = root.path().join("runtime-alias");
    symlink(&runtime, &alias).unwrap();
    let alias_inode = fs::symlink_metadata(&alias).unwrap().ino();
    assert!(matches!(OrdinaryRuntimeLeaseGroup::acquire_for_writer_with_bootstrap(&lease, &alias),
        Err(Error::UnsafeDirectory)));
    assert_eq!(fs::symlink_metadata(&alias).unwrap().ino(), alias_inode);
    let file = root.path().join("not-a-directory");
    fs::write(&file, b"preserve").unwrap();
    assert!(matches!(OrdinaryRuntimeLeaseGroup::acquire_for_writer_with_bootstrap(&lease, &file),
        Err(Error::UnsafeDirectory)));
    assert_eq!(fs::read(file).unwrap(), b"preserve");
    assert!(OrdinaryRuntimeLeaseGroup::acquire_for_writer_with_bootstrap(&lease,
        root.path().join("missing-parent").join("runtime")).is_err());
    assert!(!root.path().join("missing-parent").exists());
}

#[test]
fn concurrent_first_writer_bootstrap_has_only_one_effect_owner() {
    let root = fixture();
    let first = directory(root.path(), "profile-a", 0o755);
    let second = directory(root.path(), "profile-b", 0o755);
    let runtime = root.path().join("shared-new-runtime");
    let actor_a = start_probe(&root, &first, Some(&runtime), "bootstrap-a");
    let actor_b = start_probe(&root, &second, Some(&runtime), "bootstrap-b");
    await_fixture_file(&root.path().join("ready-bootstrap-a"));
    await_fixture_file(&root.path().join("ready-bootstrap-b"));
    fs::write(root.path().join("start-bootstrap"), FIXTURE_MARKER).unwrap();
    await_fixture_file(&root.path().join("outcome-bootstrap-a"));
    await_fixture_file(&root.path().join("outcome-bootstrap-b"));
    let outcomes = ["bootstrap-a", "bootstrap-b"].map(|role|
        fs::read(root.path().join(format!("outcome-{role}"))).unwrap());
    assert_eq!(outcomes.iter().filter(|outcome| outcome.as_slice() == b"held").count(), 1);
    assert_eq!(outcomes.iter().filter(|outcome|
        outcome.as_slice() == b"busy" || outcome.as_slice() == b"creation-race").count(), 1);
    let winner = if outcomes[0] == b"held" { "bootstrap-a" } else { "bootstrap-b" };
    assert_eq!(fs::read(runtime.join("synthetic-effect")).unwrap(), winner.as_bytes());
    let inode = fs::metadata(runtime.join(RESOURCE_LEASE_FILE)).unwrap().ino();
    fs::write(root.path().join("release-bootstrap"), FIXTURE_MARKER).unwrap();
    finish_probe(&root, actor_a, "bootstrap-a");
    finish_probe(&root, actor_b, "bootstrap-b");
    probe_runtime(&root, &first, &runtime, "runtime-ordinary-available");
    assert_eq!(fs::metadata(runtime.join(RESOURCE_LEASE_FILE)).unwrap().ino(), inode);
    assert!(!runtime.join("auth.json").exists());
    assert!(!runtime.join("config.toml").exists());
}
