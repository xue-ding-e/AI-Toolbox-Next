//! Linux-only synthetic acceptance drafts. These exercise this test executable,
//! NOT the installed GUI/CLI or a different operating-system user's session.
//! Each parent explicitly launches the ignored actor and verifies its normal
//! completion or the single deliberate abrupt-exit marker; the actor being ignored in the normal test listing is not coverage.

#![cfg(target_os = "linux")]

use std::ffi::CString;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::future::Future;
use std::io::{self, Read, Write};
use std::os::unix::fs::{symlink, DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::UnixStream as StdUnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ai_toolbox_management::owner::{
    ProfileIdentity, ProfileOwner, RuntimeDirectory, RuntimeResourceIdentity,
    PROFILE_LEASE_FILE, RESOURCE_LEASE_FILE,
};
use ai_toolbox_management::transport::{
    endpoint_basename, OwnerClient, OwnerOperation, OwnerRequest, OwnerServer,
    MAX_FRAME_BYTES, PROTOCOL_MAJOR,
};
use ai_toolbox_management::Error;
use serde_json::{json, Value};

static SERIAL: Mutex<()> = Mutex::new(());
const MARKER: &[u8] = b"ai-toolbox-owner-synthetic-v1";
const ABRUPT_MARKER: &[u8] = b"synthetic-owner-abrupt-exit-v1";
const ABRUPT_EXIT: i32 = 73;
const WORK_LIMIT: Duration = Duration::from_secs(13);
const CHILD_LIMIT: Duration = Duration::from_secs(8);
const CLEANUP_LIMIT: Duration = Duration::from_secs(2);
const RAW_PHASE_LIMIT: Duration = Duration::from_secs(2);
const DIAGNOSTIC_LIMIT: u64 = 16 * 1024;

struct Actor {
    child: Child,
    id: String,
    deadline: Instant,
    observed: bool,
}

struct Fixture {
    actors: Vec<Actor>,
    root: Option<tempfile::TempDir>,
    path: PathBuf,
    deadline: Instant,
    _serial: MutexGuard<'static, ()>,
}

impl Fixture {
    fn new() -> Self {
        let serial = SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = tempfile::Builder::new().prefix("ao-")
            .permissions(fs::Permissions::from_mode(0o700)).tempdir().unwrap();
        let root_mode = fs::metadata(root.path()).unwrap().permissions().mode() & 0o7777;
        assert_eq!(root_mode, 0o700, "synthetic runtime root must start private");
        eprintln!("synthetic runtime root mode: {root_mode:o}");
        let path = root.path().to_path_buf();
        // No global set_var/umask/chmod/chown/ACL changes. Child overrides below
        // isolate its temporary, data and home locations under this owned root.
        for name in ["tmp", "home", "xdg", "cache", "p", "p-a", "p-b", "r"] {
            DirBuilder::new().mode(0o700).create(path.join(name)).unwrap();
        }
        fs::write(path.join("fixture-marker"), MARKER).unwrap();
        RuntimeDirectory::existing(&path).unwrap();
        let socket = path.join("r").join(endpoint_basename(ProfileIdentity::existing(path.join("p")).unwrap().key()));
        assert!(socket.as_os_str().len() < 108, "Choose a shorter isolated TMPDIR for this fixture; do not fall back to a real profile");
        Self { actors: Vec::new(), root: Some(root), path, deadline: Instant::now() + WORK_LIMIT, _serial: serial }
    }

    fn path(&self, name: &str) -> PathBuf { self.path.join(name) }

    fn directory(&self, name: &str) -> PathBuf {
        let path = self.path(name);
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        path
    }

    fn spawn(&mut self, id: &str, role: &str, argument: &str) -> usize {
        assert!(self.actors.len() < 3, "per-test actor budget exceeded");
        assert!(Instant::now() < self.deadline);
        let stdout = OpenOptions::new().create_new(true).write(true).mode(0o600).open(self.path(&format!("{id}.stdout"))).unwrap();
        let stderr = OpenOptions::new().create_new(true).write(true).mode(0o600).open(self.path(&format!("{id}.stderr"))).unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "fixture_process", "--nocapture", "--test-threads=1"])
            .current_dir(&self.path)
            .env_clear()
            .env("AITB_FIXTURE_ROOT", &self.path)
            .env("AITB_FIXTURE_ID", id)
            .env("AITB_FIXTURE_ROLE", role)
            .env("AITB_FIXTURE_ARGUMENT", argument)
            .env("HOME", self.path("home"))
            .env("TMPDIR", self.path("tmp"))
            .env("XDG_CONFIG_HOME", self.path("xdg"))
            .env("XDG_CACHE_HOME", self.path("cache"))
            .env("XDG_RUNTIME_DIR", self.path("r"))
            .env("AI_TOOLBOX_DATA_DIR", self.path("p"))
            .env("RUST_BACKTRACE", "0")
            .stdin(Stdio::null()).stdout(stdout).stderr(stderr)
            .spawn().unwrap();
        self.actors.push(Actor { child, id: id.to_owned(), deadline: (Instant::now() + CHILD_LIMIT).min(self.deadline), observed: false });
        self.actors.len() - 1
    }

    fn diagnostics(&self, index: usize) -> String {
        let mut output = String::new();
        for extension in ["stdout", "stderr"] {
            if let Ok(file) = File::open(self.path(&format!("{}.{}", self.actors[index].id, extension))) {
                let mut bytes = Vec::new();
                let _ = file.take(DIAGNOSTIC_LIMIT).read_to_end(&mut bytes);
                output.push_str(&String::from_utf8_lossy(&bytes));
            }
        }
        output
    }

    fn ready(&mut self, index: usize) {
        let marker = self.path(&format!("ready-{}", self.actors[index].id));
        loop {
            if marker.is_file() { return; }
            if self.actors[index].child.try_wait().unwrap().is_some() {
                panic!("actor exited before readiness: {}", self.diagnostics(index));
            }
            assert!(Instant::now() < self.actors[index].deadline, "actor readiness deadline exceeded");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn stop(&self, index: usize) { fs::write(self.path(&format!("stop-{}", self.actors[index].id)), b"stop").unwrap(); }

    fn wait(&mut self, index: usize) {
        loop {
            if let Some(status) = self.actors[index].child.try_wait().unwrap() {
                assert!(status.success(), "actor failed: {}", self.diagnostics(index));
                let mut output = Vec::new();
                File::open(self.path(&format!("{}.stdout", self.actors[index].id))).unwrap()
                    .take(DIAGNOSTIC_LIMIT + 1).read_to_end(&mut output).unwrap();
                assert!(output.len() <= DIAGNOSTIC_LIMIT as usize, "actor output budget exceeded");
                let output = std::str::from_utf8(&output).unwrap();
                assert!(output.lines().any(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in ")),
                    "actor must report exactly one executed test and no ignored tests");
                assert!(self.path(&format!("complete-{}", self.actors[index].id)).is_file(), "ignored actor did not actually execute");
                self.actors[index].observed = true;
                eprintln!("verified actor execution: {}", self.actors[index].id);
                return;
            }
            assert!(Instant::now() < self.actors[index].deadline, "actor completion deadline exceeded");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    // Only the named stale-owner fixture deliberately exits before libtest/Drop.
    // Do not count this as a child test pass; require a distinct exit and marker.
    fn wait_abrupt_owner(&mut self, index: usize) {
        assert_eq!(self.actors[index].id, "stale");
        loop {
            if let Some(status) = self.actors[index].child.try_wait().unwrap() {
                assert_eq!(status.code(), Some(ABRUPT_EXIT), "unexpected abrupt actor exit");
                assert_eq!(fs::read(self.path("abrupt-stale")).unwrap(), ABRUPT_MARKER);
                assert!(!self.path("complete-stale").exists(), "abrupt actor must not claim normal completion");
                let mut output = Vec::new();
                File::open(self.path("stale.stdout")).unwrap()
                    .take(DIAGNOSTIC_LIMIT + 1).read_to_end(&mut output).unwrap();
                assert!(output.len() <= DIAGNOSTIC_LIMIT as usize);
                let output = std::str::from_utf8(&output).unwrap();
                assert!(!output.contains("test result:"), "abrupt actor unexpectedly returned through libtest");
                self.actors[index].observed = true;
                eprintln!("verified abrupt actor execution: stale");
                return;
            }
            assert!(Instant::now() < self.actors[index].deadline, "abrupt actor completion deadline exceeded");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn run<F: Future>(&self, future: F) -> F::Output {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            tokio::time::timeout_at(tokio::time::Instant::from_std(self.deadline), future).await.expect("fixture I/O work deadline exceeded")
        })
    }

    fn raw_connect(&self) -> StdUnixStream {
        let profile = ProfileIdentity::existing(self.path("p")).unwrap();
        let path = self.path("r").join(endpoint_basename(profile.key()));
        let stream = self.run(async {
            tokio::time::timeout(RAW_PHASE_LIMIT, tokio::net::UnixStream::connect(path)).await.unwrap().unwrap().into_std().unwrap()
        });
        stream.set_nonblocking(true).unwrap();
        stream
    }

    fn write_raw(&self, stream: &mut StdUnixStream, mut bytes: &[u8]) {
        let deadline = (Instant::now() + RAW_PHASE_LIMIT).min(self.deadline);
        while !bytes.is_empty() {
            match stream.write(bytes) {
                Ok(0) => panic!("fixture stream stopped accepting bytes"),
                Ok(count) => bytes = &bytes[count..],
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {},
                Err(error) => panic!("fixture write failed: {error}"),
            }
            assert!(Instant::now() < deadline, "fixture write deadline exceeded");
            if !bytes.is_empty() { std::thread::sleep(Duration::from_millis(5)); }
        }
    }

    fn send(&self, stream: &mut StdUnixStream, value: &Value) {
        let bytes = serde_json::to_vec(value).unwrap();
        self.write_raw(stream, &(bytes.len() as u32).to_be_bytes());
        self.write_raw(stream, &bytes);
    }

    fn read_exact(&self, stream: &mut StdUnixStream, bytes: &mut [u8], deadline: Instant) {
        let mut read = 0;
        while read < bytes.len() {
            match stream.read(&mut bytes[read..]) {
                Ok(0) => panic!("fixture stream closed during a frame"),
                Ok(count) => read += count,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {},
                Err(error) => panic!("fixture read failed: {error}"),
            }
            assert!(Instant::now() < deadline, "fixture read deadline exceeded");
            if read < bytes.len() { std::thread::sleep(Duration::from_millis(5)); }
        }
    }

    fn receive(&self, stream: &mut StdUnixStream) -> Value {
        let deadline = (Instant::now() + RAW_PHASE_LIMIT).min(self.deadline);
        let mut header = [0; 4];
        self.read_exact(stream, &mut header, deadline);
        let length = u32::from_be_bytes(header) as usize;
        assert!((1..=MAX_FRAME_BYTES).contains(&length));
        let mut bytes = vec![0; length];
        self.read_exact(stream, &mut bytes, deadline);
        serde_json::from_slice(&bytes).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let mut failed_cleanup = false;
        let unobserved = self.actors.iter().any(|actor| !actor.observed);
        for actor in &mut self.actors {
            if actor.child.try_wait().ok().flatten().is_none() { let _ = actor.child.kill(); }
        }
        let deadline = Instant::now() + CLEANUP_LIMIT;
        for actor in &mut self.actors {
            loop {
                match actor.child.try_wait() {
                    Ok(Some(_)) => break,
                    _ if Instant::now() >= deadline => { failed_cleanup = true; break; }
                    _ => std::thread::sleep(Duration::from_millis(10)),
                }
            }
        }
        // No unbounded wait()/join()/pipe draining. Actors never spawn children.
        // Preserve diagnostics and unexpected state rather than hiding failures.
        if std::thread::panicking() || failed_cleanup || unobserved {
            if let Some(root) = self.root.take() {
                eprintln!("retained synthetic fixture: {}", root.keep().display());
            }
        }
        if !std::thread::panicking() {
            assert!(!failed_cleanup && !unobserved, "actor completion/cleanup was not established");
        }
    }
}

fn create_file(path: &Path, mode: u32) {
    OpenOptions::new().create_new(true).write(true).mode(mode).open(path).unwrap();
}

#[test]
fn missing_profile_client_does_not_create_state() {
    let mut fixture = Fixture::new();
    let child = fixture.spawn("missing", "missing", "");
    fixture.wait(child);
    assert!(!fixture.path("missing-profile").exists());
    assert!(!fixture.path("missing-runtime").exists());
    assert!(!fixture.path("relative-profile").exists());
    assert!(!fixture.path("relative-runtime").exists());
    assert!(!fixture.path("relative-resource").exists());
    assert!(fs::read_dir(fixture.path("p")).unwrap().next().is_none());
}

#[test]
fn aliases_share_owner_and_writer_retains_the_lease() {
    let mut fixture = Fixture::new();
    symlink(fixture.path("p"), fixture.path("alias")).unwrap();
    let profile = ProfileIdentity::existing(fixture.path("p")).unwrap();
    let alias = ProfileIdentity::existing(fixture.path("alias")).unwrap();
    assert_eq!(profile.key(), alias.key());
    let child = fixture.spawn("owner", "hold-owner", "");
    fixture.ready(child);
    assert!(matches!(ProfileOwner::acquire(alias.clone()), Err(Error::OwnerBusy)));
    let inode = fs::metadata(fixture.path("p").join(PROFILE_LEASE_FILE)).unwrap().ino();
    fixture.stop(child);
    fixture.wait(child);
    let owner = ProfileOwner::acquire(alias).unwrap();
    let writer = owner.try_writer().unwrap();
    assert!(matches!(owner.try_writer(), Err(Error::WriterBusy)));
    drop(owner);
    assert!(matches!(ProfileOwner::acquire(profile.clone()), Err(Error::OwnerBusy)));
    writer.ensure_current().unwrap();
    drop(writer);
    let _next = ProfileOwner::acquire(profile).unwrap();
    assert_eq!(inode, fs::metadata(fixture.path("p").join(PROFILE_LEASE_FILE)).unwrap().ino());
}

#[test]
fn abnormal_lock_objects_fail_without_replacement() {
    let fixture = Fixture::new();
    for kind in ["symlink", "hardlink", "directory", "readonly", "fifo"] {
        let root = fixture.directory(kind);
        let lock = root.join(PROFILE_LEASE_FILE);
        let target = root.join("target");
        match kind {
            "symlink" => { create_file(&target, 0o600); symlink(&target, &lock).unwrap(); }
            "hardlink" => { create_file(&target, 0o600); fs::hard_link(&target, &lock).unwrap(); }
            "directory" => { DirBuilder::new().mode(0o700).create(&lock).unwrap(); }
            "readonly" => create_file(&lock, 0o400),
            "fifo" => {
                use std::os::unix::ffi::OsStrExt;
                let path = CString::new(lock.as_os_str().as_bytes()).unwrap();
                // SAFETY: An owned synthetic absolute path, NUL terminated.
                assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
            }
            _ => unreachable!(),
        }
        let before = fs::symlink_metadata(&lock).unwrap();
        assert!(matches!(ProfileOwner::acquire(ProfileIdentity::existing(root).unwrap()), Err(Error::UnsafeLockFile)));
        let after = fs::symlink_metadata(&lock).unwrap();
        assert_eq!((before.dev(), before.ino(), before.mode(), before.nlink()), (after.dev(), after.ino(), after.mode(), after.nlink()));
    }
    let root = fixture.directory("replaced-inode");
    let owner = ProfileOwner::acquire(ProfileIdentity::existing(&root).unwrap()).unwrap();
    fs::rename(root.join(PROFILE_LEASE_FILE), root.join("retained-old-inode")).unwrap();
    create_file(&root.join(PROFILE_LEASE_FILE), 0o600);
    assert!(matches!(owner.ensure_current(), Err(Error::IdentityChanged)));
}

#[test]
fn runtime_directory_never_repairs_existing_mode_or_follows_leaf_symlink() {
    let fixture = Fixture::new();
    let readonly = fixture.path("readonly-runtime");
    DirBuilder::new().mode(0o500).create(&readonly).unwrap();
    let mode = fs::metadata(&readonly).unwrap().mode();
    assert!(RuntimeDirectory::existing(&readonly).is_err());
    assert!(RuntimeDirectory::create_for_owner(&readonly).is_err());
    assert_eq!(mode, fs::metadata(&readonly).unwrap().mode());
    symlink(fixture.path("r"), fixture.path("runtime-alias")).unwrap();
    assert!(RuntimeDirectory::existing(fixture.path("runtime-alias")).is_err());
}

#[test]
fn profiles_and_aliases_contend_for_one_runtime_inode() {
    let mut fixture = Fixture::new();
    let resource = fixture.directory("resource-a");
    symlink(&resource, fixture.path("resource-alias")).unwrap();
    let child = fixture.spawn("resource", "hold-resource", "resource-a");
    fixture.ready(child);
    let owner = ProfileOwner::acquire(ProfileIdentity::existing(fixture.path("p-b")).unwrap()).unwrap();
    let writer = owner.try_writer().unwrap();
    let alias = RuntimeResourceIdentity::existing(fixture.path("resource-alias")).unwrap();
    assert!(matches!(writer.acquire_resources(vec![alias.clone()]), Err(Error::ResourceBusy)));
    let inode = fs::metadata(resource.join(RESOURCE_LEASE_FILE)).unwrap().ino();
    fixture.stop(child);
    fixture.wait(child);
    let permit = writer.acquire_resources(vec![alias.clone()]).unwrap();
    permit.require_resource(&alias).unwrap();
    assert_eq!(inode, fs::metadata(resource.join(RESOURCE_LEASE_FILE)).unwrap().ino());
    let readonly = fixture.path("readonly-resource");
    DirBuilder::new().mode(0o500).create(&readonly).unwrap();
    let readonly_identity = RuntimeResourceIdentity::existing(&readonly).unwrap();
    assert!(matches!(writer.acquire_resources(vec![readonly_identity]), Err(Error::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied));
    assert!(!readonly.join(RESOURCE_LEASE_FILE).exists());
}

#[test]
fn runtime_order_is_physical_and_failed_acquisition_releases_its_prefix() {
    let mut fixture = Fixture::new();
    fixture.directory("resource-a");
    fixture.directory("resource-b");
    let mut resources = ["resource-a", "resource-b"].map(|name| (name, RuntimeResourceIdentity::existing(fixture.path(name)).unwrap()));
    resources.sort_by_key(|(_, resource)| (resource.key().device, resource.key().inode));
    let child = fixture.spawn("high", "hold-resource", resources[1].0);
    fixture.ready(child);
    let owner = ProfileOwner::acquire(ProfileIdentity::existing(fixture.path("p-b")).unwrap()).unwrap();
    let writer = owner.try_writer().unwrap();
    assert!(!resources[0].1.canonical_path().join(RESOURCE_LEASE_FILE).exists());
    assert!(matches!(writer.acquire_resources(vec![resources[1].1.clone(), resources[0].1.clone(), resources[0].1.clone()]), Err(Error::ResourceBusy)));
    // This proves it attempted the lower identity before the busy higher one,
    // rather than merely failing at the first item in the caller's order.
    assert!(resources[0].1.canonical_path().join(RESOURCE_LEASE_FILE).exists());
    let _released_prefix = writer.acquire_resources(vec![resources[0].1.clone()]).unwrap();
    fixture.stop(child);
    fixture.wait(child);
}

#[test]
fn owner_status_round_trip_is_a_real_local_process_exchange() {
    let mut fixture = Fixture::new();
    let child = fixture.spawn("status", "serve", "status");
    fixture.ready(child);
    let profile = ProfileIdentity::existing(fixture.path("p")).unwrap();
    let runtime = RuntimeDirectory::existing(fixture.path("r")).unwrap();
    let status = fixture.run(async { OwnerClient::connect(&profile, &runtime, 7).await.unwrap().owner_status().await.unwrap() });
    assert!(status.profile_lease_held);
    fixture.wait(child);
    assert!(!runtime.path().join(endpoint_basename(profile.key())).exists());
    assert!(profile.canonical_path().join(PROFILE_LEASE_FILE).is_file());
}

#[test]
fn peer_rejects_version_epoch_and_profile_mismatch() {
    let mut fixture = Fixture::new();
    let profile = ProfileIdentity::existing(fixture.path("p")).unwrap();
    for case in ["version", "epoch", "profile"] {
        let child = fixture.spawn(case, "serve", case);
        fixture.ready(child);
        let mut stream = fixture.raw_connect();
        fixture.send(&mut stream, &json!({"protocol_major": if case == "version" { 0 } else { PROTOCOL_MAJOR }, "request_id": 1, "profile": profile.key()}));
        if case != "version" {
            let ack = fixture.receive(&mut stream);
            let mut request = OwnerRequest { protocol_major: PROTOCOL_MAJOR, request_id: 1, profile: profile.key(), owner_epoch: serde_json::from_value(ack["owner_epoch"].clone()).unwrap(), operation: OwnerOperation::OwnerStatus };
            if case == "epoch" { request.owner_epoch[0] ^= 1; } else { request.profile.inode ^= 1; }
            fixture.send(&mut stream, &serde_json::to_value(request).unwrap());
        }
        fixture.wait(child);
    }
}

#[test]
fn peer_rejects_oversized_empty_and_truncated_frames() {
    let mut fixture = Fixture::new();
    for case in ["oversize", "zero", "truncated"] {
        let child = fixture.spawn(case, "serve", case);
        fixture.ready(child);
        let mut stream = fixture.raw_connect();
        let length = match case { "oversize" => MAX_FRAME_BYTES as u32 + 1, "zero" => 0, _ => 99 };
        fixture.write_raw(&mut stream, &length.to_be_bytes());
        if case == "truncated" {
            fixture.write_raw(&mut stream, b"{");
            stream.shutdown(std::net::Shutdown::Write).unwrap();
        }
        fixture.wait(child);
    }
}

#[test]
fn stalled_partial_frame_has_a_total_deadline() {
    let mut fixture = Fixture::new();
    let start = Instant::now();
    let child = fixture.spawn("slow", "serve", "slow");
    fixture.ready(child);
    let mut stream = fixture.raw_connect();
    fixture.write_raw(&mut stream, &[0]);
    fixture.wait(child);
    assert!(start.elapsed() >= Duration::from_secs(4));
    drop(stream);
}

#[test]
fn crashed_owner_leaves_an_explicit_stale_endpoint_conflict() {
    let mut fixture = Fixture::new();
    let child = fixture.spawn("stale", "serve", "stale");
    fixture.wait_abrupt_owner(child);
    let profile = ProfileIdentity::existing(fixture.path("p")).unwrap();
    let runtime = RuntimeDirectory::existing(fixture.path("r")).unwrap();
    let socket = runtime.path().join(endpoint_basename(profile.key()));
    let inode = fs::symlink_metadata(&socket).unwrap().ino();
    let owner = ProfileOwner::acquire(profile).unwrap();
    let result = fixture.run(async { OwnerServer::bind(owner, runtime) });
    assert!(matches!(result, Err(Error::EndpointConflict)));
    assert_eq!(inode, fs::symlink_metadata(socket).unwrap().ino());
}

#[test]
#[ignore = "Only an explicit parent --ignored --exact fixture_process launch is an execution"]
fn fixture_process() {
    let root = PathBuf::from(std::env::var_os("AITB_FIXTURE_ROOT").expect("missing synthetic root"));
    RuntimeDirectory::existing(&root).unwrap();
    assert_eq!(fs::read(root.join("fixture-marker")).unwrap(), MARKER);
    assert_eq!(std::env::var_os("TMPDIR").unwrap(), root.join("tmp").into_os_string());
    assert_eq!(std::env::var_os("HOME").unwrap(), root.join("home").into_os_string());
    let id = std::env::var("AITB_FIXTURE_ID").unwrap();
    assert!(!id.is_empty() && id.len() < 32 && id.bytes().all(|byte| byte.is_ascii_lowercase() || byte == b'-'));
    let role = std::env::var("AITB_FIXTURE_ROLE").unwrap();
    let argument = std::env::var("AITB_FIXTURE_ARGUMENT").unwrap();
    let ready = || fs::write(root.join(format!("ready-{id}")), b"ready").unwrap();
    let complete = || fs::write(root.join(format!("complete-{id}")), b"complete").unwrap();
    let wait_stop = || {
        let deadline = Instant::now() + Duration::from_secs(6);
        while !root.join(format!("stop-{id}")).is_file() {
            assert!(Instant::now() < deadline, "hold actor stop deadline exceeded");
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    match role.as_str() {
        "missing" => {
            assert!(matches!(ProfileIdentity::existing(root.join("missing-profile")), Err(Error::InvalidProfile)));
            assert!(RuntimeDirectory::existing(root.join("missing-runtime")).is_err());
            assert!(matches!(ProfileIdentity::existing("relative-profile"), Err(Error::RelativePath)));
            assert!(matches!(RuntimeDirectory::existing("relative-runtime"), Err(Error::RelativePath)));
            assert!(matches!(RuntimeDirectory::create_for_owner("relative-runtime"), Err(Error::RelativePath)));
            assert!(matches!(RuntimeResourceIdentity::existing("relative-resource"), Err(Error::RelativePath)));
            assert!(!root.join("missing-profile").exists() && !root.join("missing-runtime").exists());
        }
        "hold-owner" => {
            let _owner = ProfileOwner::acquire(ProfileIdentity::existing(root.join("p")).unwrap()).unwrap();
            ready();
            wait_stop();
        }
        "hold-resource" => {
            assert!(matches!(argument.as_str(), "resource-a" | "resource-b"));
            let owner = ProfileOwner::acquire(ProfileIdentity::existing(root.join("p-a")).unwrap()).unwrap();
            let writer = owner.try_writer().unwrap();
            let resource = RuntimeResourceIdentity::existing(root.join(&argument)).unwrap();
            let _permit = writer.acquire_resources(vec![resource]).unwrap();
            ready();
            wait_stop();
        }
        "serve" => {
            let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            runtime.block_on(async {
                let owner = ProfileOwner::acquire(ProfileIdentity::existing(root.join("p")).unwrap()).unwrap();
                let server = OwnerServer::bind(owner, RuntimeDirectory::existing(root.join("r")).unwrap()).unwrap();
                ready();
                if argument == "stale" {
                    assert_eq!(id, "stale");
                    fs::write(root.join("abrupt-stale"), ABRUPT_MARKER).unwrap();
                    // Model abrupt owner loss without running Rust Drop. OS
                    // closes its handles; only the synthetic socket remains.
                    std::process::exit(ABRUPT_EXIT);
                }
                let accepted = server.accept().await;
                match argument.as_str() {
                    "status" => {
                        let pending = accepted.unwrap();
                        drop(server);
                        let identity = ProfileIdentity::existing(root.join("p")).unwrap();
                        assert!(matches!(ProfileOwner::acquire(identity.clone()), Err(Error::OwnerBusy)));
                        pending.respond_status().await.unwrap();
                        let _after_request = ProfileOwner::acquire(identity).unwrap();
                    }
                    "version" => assert!(matches!(accepted, Err(Error::ProtocolMismatch))),
                    "epoch" | "profile" => assert!(matches!(accepted, Err(Error::StaleOwner))),
                    "oversize" => assert!(matches!(accepted, Err(Error::FrameTooLarge))),
                    "zero" => assert!(matches!(accepted, Err(Error::InvalidMessage))),
                    "truncated" => assert!(matches!(accepted, Err(Error::Io(error)) if error.kind() == io::ErrorKind::UnexpectedEof)),
                    "slow" => assert!(matches!(accepted, Err(Error::TimedOut))),
                    _ => panic!("unrecognized synthetic serve case"),
                }
            });
        }
        _ => panic!("unrecognized synthetic actor role"),
    }
    complete();
}
