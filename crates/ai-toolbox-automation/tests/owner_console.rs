//! Actual console processes, real OwnerServer, and synthetic fault peers only.
//! No installed GUI, user profile, real credentials, or business writes are used.
#![cfg(target_os = "linux")]

use std::cell::RefCell;
use std::ffi::{CString, OsString};
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ai_toolbox_management::owner::{
    OwnerPermit, ProfileIdentity, ProfileOwner, RuntimeDirectory, PROFILE_LEASE_FILE,
};
use ai_toolbox_management::transport::{
    endpoint_basename, OwnerOperation, OwnerServer, IO_TIMEOUT, MAX_FRAME_BYTES, PROTOCOL_MAJOR,
};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
// Connect/hello and status each have a five-second protocol budget. Do not use
// the discovery harness's five-second total as this command's observation limit.
const CHILD_LIMIT: Duration = Duration::from_secs(13);
const CLEANUP_LIMIT: Duration = Duration::from_secs(2);
const OUTPUT_LIMIT: u64 = 64 * 1024;

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

struct BoundedChild {
    child: Child,
    reaped: bool,
    cleanup: Arc<CleanupState>,
}

struct CleanupState {
    active_children: AtomicUsize,
    safe_to_remove: AtomicBool,
}

impl BoundedChild {
    fn mark_reaped(&mut self) {
        if !self.reaped {
            self.reaped = true;
            self.cleanup.active_children.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl Drop for BoundedChild {
    fn drop(&mut self) {
        if self.reaped {
            return;
        }
        let _ = self.child.kill();
        let deadline = Instant::now() + CLEANUP_LIMIT;
        loop {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                self.mark_reaped();
                return;
            }
            if Instant::now() >= deadline {
                // A live or unobserved child may still use the lease and DB.
                // Retain the entire fixture, including its output files.
                self.cleanup.safe_to_remove.store(false, Ordering::SeqCst);
                eprintln!("console child cleanup deadline exceeded; reap is unconfirmed, so its fixture must be retained");
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

struct Fixture {
    root: PathBuf,
    outputs: RefCell<Vec<PathBuf>>,
    cleanup: Arc<CleanupState>,
    retain: bool,
}

impl Fixture {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ac-{:x}-{nonce:x}-{:x}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        DirBuilder::new().mode(0o700).create(&root).unwrap();
        assert_eq!(
            fs::metadata(&root).unwrap().permissions().mode() & 0o7777,
            0o700
        );
        let fixture = Self {
            root,
            outputs: RefCell::new(Vec::new()),
            cleanup: Arc::new(CleanupState {
                active_children: AtomicUsize::new(0),
                safe_to_remove: AtomicBool::new(true),
            }),
            retain: false,
        };
        for name in ["p", "r", "home"] {
            DirBuilder::new()
                .mode(0o700)
                .create(fixture.path(name))
                .unwrap();
        }
        // Deliberately not a SQLite database. A hidden initializer cannot pass
        // unnoticed; the inotify watch also detects open/read attempts, not just writes.
        fs::write(fixture.path("p/ai-toolbox.db"), b"synthetic-not-a-database").unwrap();
        RuntimeDirectory::existing(&fixture.root).expect("Use an isolated TMPDIR on a management-admitted local filesystem; never fall back to a user profile");
        assert!(
            fixture.socket().as_os_str().as_bytes().len() < 108,
            "Use a shorter isolated TMPDIR for Unix socket fixtures"
        );
        fixture
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn profile(&self) -> ProfileIdentity {
        ProfileIdentity::existing(self.path("p")).unwrap()
    }

    fn owner(&self) -> OwnerPermit {
        ProfileOwner::acquire(self.profile()).unwrap()
    }

    fn socket(&self) -> PathBuf {
        self.path("r").join(endpoint_basename(self.profile().key()))
    }

    fn status_args(&self, format: &str) -> Vec<OsString> {
        vec![
            "--format".into(),
            format.into(),
            "daemon".into(),
            "status".into(),
            "--profile".into(),
            self.path("p").into_os_string(),
            "--endpoint-dir".into(),
            self.path("r").into_os_string(),
        ]
    }

    async fn run(&self, args: &[OsString]) -> Output {
        let lease = self.path(&format!("p/{PROFILE_LEASE_FILE}"));
        let lease_signature = || match fs::metadata(&lease) {
            Ok(meta) => Some((
                meta.dev(),
                meta.ino(),
                meta.len(),
                meta.mode(),
                meta.mtime(),
                meta.mtime_nsec(),
                meta.ctime(),
                meta.ctime_nsec(),
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => panic!("could not inspect fixture lease: {error}"),
        };
        let original_lease = lease_signature();
        let index = self.outputs.borrow().len();
        let stdout = self.path(&format!("run-{index}.stdout"));
        let stderr = self.path(&format!("run-{index}.stderr"));
        let out = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&stdout)
            .unwrap();
        self.outputs.borrow_mut().push(stdout.clone());
        let err = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&stderr)
            .unwrap();
        self.outputs.borrow_mut().push(stderr.clone());
        let mut command = Command::new(env!("CARGO_BIN_EXE_ai-toolbox-cli"));
        command
            .args(args)
            .env_clear()
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err);
        for name in ["HOME", "USERPROFILE"] {
            command.env(name, self.path("home"));
        }
        // Use hostile-but-synthetic environment defaults. Explicit arguments
        // must win, and discovery must not inspect or bootstrap any of these.
        command.env("AI_TOOLBOX_DATA_DIR", self.path("p"));
        for name in [
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_CACHE_HOME",
            "XDG_RUNTIME_DIR",
            "AI_TOOLBOX_CACHE_DIR",
            "APPDATA",
            "LOCALAPPDATA",
            "CODEX_HOME",
            "CLAUDE_CONFIG_DIR",
            "TMPDIR",
            "TMP",
            "TEMP",
        ] {
            command.env(name, self.path(name));
        }
        let child = command.spawn().unwrap();
        self.cleanup.active_children.fetch_add(1, Ordering::SeqCst);
        let mut child = BoundedChild {
            child,
            reaped: false,
            cleanup: self.cleanup.clone(),
        };
        let deadline = Instant::now() + CHILD_LIMIT;
        let status = loop {
            // Check while the child runs, not only after it exits. Overflow of
            // the combined output budget immediately invokes bounded kill/reap.
            let output_bytes = fs::metadata(&stdout)
                .unwrap()
                .len()
                .saturating_add(fs::metadata(&stderr).unwrap().len());
            assert!(
                output_bytes <= OUTPUT_LIMIT,
                "console combined output budget exceeded"
            );
            match child.child.try_wait() {
                Ok(Some(status)) => {
                    child.mark_reaped();
                    break status;
                }
                Ok(None) => {}
                Err(error) => panic!("could not observe console process: {error}"),
            }
            assert!(
                Instant::now() < deadline,
                "owner console exceeded its 13-second observation deadline"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        assert!(
            fs::metadata(&stdout)
                .unwrap()
                .len()
                .saturating_add(fs::metadata(&stderr).unwrap().len())
                <= OUTPUT_LIMIT,
            "console combined output budget exceeded at exit"
        );
        assert_eq!(
            lease_signature(),
            original_lease,
            "console created or changed the lease inode"
        );
        // Regular files avoid unbounded pipe drains after child termination.
        let read_output = |path: &Path| {
            let mut bytes = Vec::new();
            File::open(path)
                .unwrap()
                .take(OUTPUT_LIMIT + 1)
                .read_to_end(&mut bytes)
                .unwrap();
            assert!(
                bytes.len() <= OUTPUT_LIMIT as usize,
                "console output budget exceeded"
            );
            bytes
        };
        self.assert_no_implicit_state();
        Output {
            status,
            stdout: read_output(&stdout),
            stderr: read_output(&stderr),
        }
    }

    fn assert_no_implicit_state(&self) {
        let mut expected = vec![OsString::from("p"), "r".into(), "home".into()];
        expected.extend(
            self.outputs
                .borrow()
                .iter()
                .map(|path| path.file_name().unwrap().to_owned()),
        );
        expected.sort();
        let mut actual: Vec<_> = fs::read_dir(&self.root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        actual.sort();
        assert_eq!(actual, expected, "console created an implicit state path");
        assert_eq!(fs::read_dir(self.path("home")).unwrap().count(), 0);
        for entry in fs::read_dir(self.path("p")).unwrap() {
            let name = entry.unwrap().file_name();
            assert!(
                name == "ai-toolbox.db" || name == PROFILE_LEASE_FILE,
                "unexpected profile artifact"
            );
        }
        for entry in fs::read_dir(self.path("r")).unwrap() {
            assert_eq!(
                entry.unwrap().path(),
                self.socket(),
                "unexpected endpoint artifact"
            );
        }
    }

    fn cleanup_is_safe(&self) -> bool {
        self.cleanup.safe_to_remove.load(Ordering::SeqCst)
            && self.cleanup.active_children.load(Ordering::SeqCst) == 0
    }

    fn remove_owned_artifacts(&mut self) -> std::io::Result<()> {
        // This is not recursive cleanup: an unexpected path causes directory
        // removal to fail and remains available for diagnosis.
        for path in self.outputs.get_mut().iter() {
            fs::remove_file(path)?;
        }
        fs::remove_file(self.path("p/ai-toolbox.db"))?;
        match fs::remove_file(self.path(&format!("p/{PROFILE_LEASE_FILE}"))) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        for name in ["p", "r", "home"] {
            fs::remove_dir(self.path(name))?;
        }
        fs::remove_dir(&self.root)
    }

    fn finish(mut self) {
        // All success paths explicitly verify cleanup. Never downgrade a cleanup
        // failure to a passing test or retry partial cleanup during Drop.
        self.retain = true;
        assert!(
            self.cleanup_is_safe(),
            "fixture retained because child exit/reap was not confirmed"
        );
        self.remove_owned_artifacts()
            .expect("fixture cleanup failed; remaining owned artifacts retained");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // During failure unwinding, no cleanup is permitted until every spawned
        // child was confirmed reaped. Unknown outcomes retain the whole root.
        if self.retain || !self.cleanup_is_safe() {
            return;
        }
        if let Err(error) = self.remove_owned_artifacts() {
            eprintln!("fixture cleanup failed; remaining owned artifacts retained: {error}");
        }
    }
}

struct DatabaseWatch(OwnedFd);

impl DatabaseWatch {
    fn new(fixture: &Fixture) -> Self {
        // SAFETY: inotify_init1 has no pointer arguments; successful FD ownership
        // transfers immediately to OwnedFd, including add-watch failure paths.
        let raw = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        assert!(raw >= 0);
        let descriptor = unsafe { OwnedFd::from_raw_fd(raw) };
        let path = CString::new(fixture.path("p/ai-toolbox.db").as_os_str().as_bytes()).unwrap();
        // SAFETY: the CString and live FD remain valid for the syscall.
        let watch = unsafe {
            libc::inotify_add_watch(
                descriptor.as_raw_fd(),
                path.as_ptr(),
                libc::IN_OPEN
                    | libc::IN_ACCESS
                    | libc::IN_MODIFY
                    | libc::IN_ATTRIB
                    | libc::IN_DELETE_SELF
                    | libc::IN_MOVE_SELF,
            )
        };
        assert!(watch >= 0);
        Self(descriptor)
    }

    fn assert_untouched(&self) {
        let mut events = [0u8; 4096];
        // SAFETY: events is writable for its full length and the FD is live.
        let count =
            unsafe { libc::read(self.0.as_raw_fd(), events.as_mut_ptr().cast(), events.len()) };
        assert_eq!(count, -1, "the synthetic DB was opened, read, or modified");
        assert_eq!(
            std::io::Error::last_os_error().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}

fn json_result(output: &Output, exit: i32, error: Option<&str>) -> Value {
    assert_eq!(output.status.code(), Some(exit));
    assert!(output.stderr.is_empty());
    assert_eq!(
        std::str::from_utf8(&output.stdout).unwrap().lines().count(),
        1
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert!(value["request_id"].is_null());
    assert_eq!(value["ok"], exit == 0);
    match error {
        Some(code) => {
            assert_eq!(value["error"]["code"], code);
            assert!(value["data"].is_null());
        }
        None => assert!(value["error"].is_null()),
    }
    value
}

#[test]
fn actual_console_reads_real_owner_without_touching_database_or_lease() {
    let fixture = Fixture::new();
    let lease = fixture.path(&format!("p/{PROFILE_LEASE_FILE}"));
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&lease)
        .unwrap();
    let before = fs::metadata(&lease).unwrap();
    let watch = DatabaseWatch::new(&fixture);
    runtime().block_on(async {
        let server = Arc::new(OwnerServer::bind(fixture.owner(), RuntimeDirectory::existing(fixture.path("r")).unwrap()).unwrap());
        for format in ["json", "text"] {
            let serving = server.clone();
            let task = tokio::spawn(async move {
                let request = serving.accept().await.unwrap();
                assert_eq!(request.request().operation, OwnerOperation::OwnerStatus);
                assert_eq!(request.request().request_id, 1);
                request.respond_status().await.unwrap();
            });
            let output = fixture.run(&fixture.status_args(format)).await;
            tokio::time::timeout(CHILD_LIMIT, task).await.unwrap().unwrap();
            if format == "json" {
                let value = json_result(&output, 0, None);
                assert_eq!(value["operation"], "daemon.status");
                assert_eq!(value["data"], json!({ "profile_lease_held": true }));
            } else {
                assert_eq!(output.status.code(), Some(0));
                assert!(output.stderr.is_empty());
                assert_eq!(std::str::from_utf8(&output.stdout).unwrap(), "profile_lease_held: true\nThis does not establish database readiness or complete writer admission.\n");
            }
            watch.assert_untouched();
        }
        assert!(matches!(ProfileOwner::acquire(fixture.profile()), Err(ai_toolbox_management::Error::OwnerBusy)));
        let after = fs::metadata(&lease).unwrap();
        assert_eq!((before.dev(), before.ino(), before.len(), before.mode(), before.mtime(), before.mtime_nsec(), before.ctime(), before.ctime_nsec()),
            (after.dev(), after.ino(), after.len(), after.mode(), after.mtime(), after.mtime_nsec(), after.ctime(), after.ctime_nsec()));
    });
    assert!(!fixture.socket().exists());
    assert!(
        lease.exists(),
        "only the owner socket may be removed on shutdown"
    );
    fixture.finish();
}

#[test]
fn discovery_and_parse_errors_never_open_the_synthetic_database() {
    let fixture = Fixture::new();
    let watch = DatabaseWatch::new(&fixture);
    runtime().block_on(async {
        for command in ["--help", "-h", "--version", "-V", "capabilities"] {
            let output = fixture
                .run(&["--format".into(), "json".into(), command.into()])
                .await;
            json_result(&output, 0, None);
            watch.assert_untouched();
        }
        let mut invalid = fixture.status_args("json");
        invalid.extend([
            "--secret=never-echo-token".into(),
            "--profile".into(),
            fixture.path("private-token").into_os_string(),
        ]);
        let output = fixture.run(&invalid).await;
        json_result(&output, 2, Some("INVALID_INPUT"));
        let text = std::str::from_utf8(&output.stdout).unwrap();
        assert!(!text.contains("private-token") && !text.contains("never-echo-token"));
        watch.assert_untouched();
        assert!(!fixture.path(&format!("p/{PROFILE_LEASE_FILE}")).exists());
        assert_eq!(fs::read_dir(fixture.path("r")).unwrap().count(), 0);
    });
    fixture.finish();
}

#[test]
fn missing_paths_and_owner_unavailability_never_bootstrap_or_fall_back() {
    let fixture = Fixture::new();
    let watch = DatabaseWatch::new(&fixture);
    runtime().block_on(async {
        let mut missing_profile = fixture.status_args("json");
        missing_profile[5] = fixture.path("missing-profile/private-token").into_os_string();
        let output = fixture.run(&missing_profile).await;
        json_result(&output, 4, Some("INVALID_PROFILE"));
        assert!(!std::str::from_utf8(&output.stdout).unwrap().contains("private-token"));
        assert!(!fixture.path("missing-profile").exists());

        let mut missing_endpoint = fixture.status_args("json");
        missing_endpoint[7] = fixture.path("missing-endpoint").into_os_string();
        json_result(&fixture.run(&missing_endpoint).await, 4, Some("INVALID_ENDPOINT"));
        assert!(!fixture.path("missing-endpoint").exists());

        json_result(&fixture.run(&fixture.status_args("json")).await, 5, Some("OWNER_UNAVAILABLE"));
        let text = fixture.run(&fixture.status_args("text")).await;
        assert_eq!(text.status.code(), Some(5));
        assert!(text.stdout.is_empty());
        assert_eq!(std::str::from_utf8(&text.stderr).unwrap(), "OWNER_UNAVAILABLE [connect]: The existing owner endpoint is unavailable; no owner was started.\n");
        watch.assert_untouched();
        assert!(!fixture.path(&format!("p/{PROFILE_LEASE_FILE}")).exists());
        assert_eq!(fs::read_dir(fixture.path("r")).unwrap().count(), 0);
    });
    fixture.finish();
}

#[test]
fn insecure_endpoint_directory_fails_without_repairing_permissions() {
    let fixture = Fixture::new();
    let watch = DatabaseWatch::new(&fixture);
    fs::set_permissions(fixture.path("r"), fs::Permissions::from_mode(0o755)).unwrap();
    runtime().block_on(async {
        json_result(
            &fixture.run(&fixture.status_args("json")).await,
            4,
            Some("INVALID_ENDPOINT"),
        );
    });
    assert_eq!(
        fs::metadata(fixture.path("r")).unwrap().mode() & 0o7777,
        0o755
    );
    watch.assert_untouched();
    fixture.finish();
}

async fn read_frame(stream: &mut UnixStream) -> Value {
    let length = stream.read_u32().await.unwrap() as usize;
    assert!(length > 0 && length <= MAX_FRAME_BYTES);
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn write_frame(stream: &mut UnixStream, value: &Value) {
    let bytes = serde_json::to_vec(value).unwrap();
    stream.write_u32(bytes.len() as u32).await.unwrap();
    stream.write_all(&bytes).await.unwrap();
}

#[test]
fn incompatible_protocol_profile_and_correlation_are_rejected_without_raw_echo() {
    let fixture = Fixture::new();
    let watch = DatabaseWatch::new(&fixture);
    runtime().block_on(async {
        let _owner = fixture.owner();
        for fault in ["protocol", "profile", "correlation", "untrusted-field"] {
            let listener = UnixListener::bind(fixture.socket()).unwrap();
            let task = tokio::spawn(async move {
                tokio::time::timeout(CHILD_LIMIT, async {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    let hello = read_frame(&mut stream).await;
                    let epoch = [0u8; 16];
                    let mut ack = json!({ "protocol_major": PROTOCOL_MAJOR, "request_id": hello["request_id"],
                        "profile": hello["profile"], "owner_epoch": epoch });
                    match fault {
                        "protocol" => ack["protocol_major"] = json!(PROTOCOL_MAJOR + 1),
                        "profile" => ack["profile"] = json!({ "device": u64::MAX, "inode": u64::MAX }),
                        "correlation" => ack["request_id"] = json!(2),
                        _ => ack["untrusted"] = json!("never-echo-token"),
                    }
                    write_frame(&mut stream, &ack).await;
                }).await.unwrap();
            });
            let output = fixture.run(&fixture.status_args("json")).await;
            tokio::time::timeout(CHILD_LIMIT, task).await.unwrap().unwrap();
            let code = if fault == "profile" { "OWNER_IDENTITY_REJECTED" } else { "OWNER_PROTOCOL_REJECTED" };
            json_result(&output, 6, Some(code));
            assert!(!std::str::from_utf8(&output.stdout).unwrap().contains("never-echo-token"));
            watch.assert_untouched();
            fs::remove_file(fixture.socket()).unwrap();
        }
    });
    fixture.finish();
}

#[test]
fn stalled_owner_is_bounded_without_starting_a_replacement() {
    let fixture = Fixture::new();
    let watch = DatabaseWatch::new(&fixture);
    runtime().block_on(async {
        let _owner = fixture.owner();
        let listener = UnixListener::bind(fixture.socket()).unwrap();
        let task = tokio::spawn(async move {
            tokio::time::timeout(CHILD_LIMIT, async {
                let (mut stream, _) = listener.accept().await.unwrap();
                let _hello = read_frame(&mut stream).await;
                tokio::time::sleep(IO_TIMEOUT + Duration::from_secs(2)).await;
                drop(stream);
            })
            .await
            .unwrap();
        });
        json_result(
            &fixture.run(&fixture.status_args("json")).await,
            7,
            Some("OWNER_TIMEOUT"),
        );
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        watch.assert_untouched();
        fs::remove_file(fixture.socket()).unwrap();
    });
    fixture.finish();
}
