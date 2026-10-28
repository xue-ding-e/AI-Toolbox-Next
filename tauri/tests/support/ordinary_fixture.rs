//! Production-library positive fixtures run in separate processes. Each child
//! installs one real ordinary owner before constructing any test DB or runtime.
//! No production registry reset, cfg(test) override, or absent-owner bypass.
use ai_toolbox_lib::{initialize_ordinary_profile, OrdinaryProfileOwnership};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const CASE: &str = "AITB_ORDINARY_FIXTURE_CASE";
const ROOT: &str = "AITB_ORDINARY_FIXTURE_ROOT";
const NONCE: &str = "AITB_ORDINARY_FIXTURE_NONCE";
const LIMIT: u64 = 64 * 1024;
const WAIT: Duration = Duration::from_secs(30);

struct BoundedChild(Option<Child>);
impl Drop for BoundedChild {
    fn drop(&mut self) {
        let Some(mut child) = self.0.take() else { return; };
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {},
            Err(error) => eprintln!("ordinary fixture status error: {error}"),
        }
        if let Err(error) = child.kill() {
            eprintln!("ordinary fixture kill error: {error}");
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if matches!(child.try_wait(), Ok(Some(_))) { return; }
            thread::sleep(Duration::from_millis(10));
        }
        eprintln!("ordinary fixture child exit unconfirmed; retain owned fixture");
    }
}

fn bounded_log(path: &Path) -> String {
    assert!(fs::metadata(path).unwrap().len() <= LIMIT, "fixture log limit");
    let mut text = String::new();
    File::open(path).unwrap().take(LIMIT).read_to_string(&mut text).unwrap();
    text
}

pub(crate) fn enter(full_name: &str) -> Option<OrdinaryProfileOwnership> {
    // module_path includes the integration crate; libtest's exact name does not.
    let exact = full_name.split_once("::").expect("integration module path").1;
    if let Some(requested) = std::env::var_os(CASE) {
        assert_eq!(requested, exact);
        let root = std::path::PathBuf::from(std::env::var_os(ROOT).expect("owned fixture root"));
        assert!(root.is_absolute());
        assert_eq!(root.canonicalize().unwrap(), root);
        assert!(root.file_name().unwrap().to_string_lossy().starts_with("aitb-ordinary-integration-"));
        let meta = fs::symlink_metadata(&root).unwrap();
        assert!(meta.is_dir() && !meta.file_type().is_symlink());
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(meta.permissions().mode() & 0o777, 0o700);
        }
        let nonce = std::env::var(NONCE).expect("owned nonce");
        assert!(uuid::Uuid::parse_str(&nonce).is_ok());
        let marker = root.join("fixture-owner");
        assert!(fs::symlink_metadata(&marker).unwrap().is_file());
        assert_eq!(fs::read_to_string(marker).unwrap(), format!("{nonce}\n{exact}\n"));
        assert_eq!(std::env::current_dir().unwrap(), root);
        OpenOptions::new().write(true).create_new(true).open(root.join("child-entered")).unwrap();
        let profile = root.join("profile");
        assert!(fs::read_dir(&profile).unwrap().next().is_none());
        return Some(initialize_ordinary_profile(&profile).expect("real ordinary ownership"));
    }
    assert!(std::env::var_os(ROOT).is_none() && std::env::var_os(NONCE).is_none(), "partial fixture environment");
    let temp = tempfile::Builder::new().prefix("aitb-ordinary-integration-").tempdir().unwrap();
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    }
    // Retain on failure or unconfirmed child exit. Do not drop TempDir while a
    // child may still be using it. The outer isolated supervisor owns residues.
    let root = temp.keep().canonicalize().unwrap();
    let nonce = uuid::Uuid::new_v4().to_string();
    fs::write(root.join("fixture-owner"), format!("{nonce}\n{exact}\n")).unwrap();
    fs::create_dir(root.join("profile")).unwrap();
    fs::create_dir(root.join("tmp")).unwrap();
    let stdout_path = root.join("stdout.log");
    let stderr_path = root.join("stderr.log");
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", exact, "--nocapture", "--test-threads=1"])
        .env_clear().env(CASE, exact).env(ROOT, &root).env(NONCE, nonce)
        .env("HOME", &root).env("USERPROFILE", &root)
        .env("APPDATA", &root).env("LOCALAPPDATA", &root)
        .env("TMPDIR", root.join("tmp"))
        .env("TEMP", root.join("tmp")).env("TMP", root.join("tmp"))
        .env("XDG_CONFIG_HOME", &root).env("XDG_DATA_HOME", &root)
        .env("XDG_CACHE_HOME", &root).env("AI_TOOLBOX_DATA_DIR", root.join("profile"))
        .current_dir(&root).stdin(Stdio::null())
        .stdout(File::create(&stdout_path).unwrap()).stderr(File::create(&stderr_path).unwrap());
    // Windows loader/system APIs require these non-sensitive installation
    // locations. No PATH, user config, credentials, or arbitrary env inheritance.
    #[cfg(windows)]
    for name in ["SystemRoot", "WINDIR"] {
        if let Some(value) = std::env::var_os(name) { command.env(name, value); }
    }
    let child = command.spawn().unwrap();
    let mut child = BoundedChild(Some(child));
    let deadline = Instant::now() + WAIT;
    let status = loop {
        if let Some(status) = child.0.as_mut().unwrap().try_wait().unwrap() { break status; }
        assert!(Instant::now() < deadline, "ordinary fixture timed out: {}", root.display());
        assert!(fs::metadata(&stdout_path).unwrap().len() <= LIMIT && fs::metadata(&stderr_path).unwrap().len() <= LIMIT,
            "ordinary fixture output exceeded bound: {}", root.display());
        thread::sleep(Duration::from_millis(10));
    };
    child.0.take(); // try_wait has reaped this exact child; no daemon/setsid actors.
    let stdout = bounded_log(&stdout_path);
    let stderr = bounded_log(&stderr_path);
    assert!(status.success(), "ordinary fixture failed at {}: {stdout}\n{stderr}", root.display());
    assert!(stdout.lines().any(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;")), "exact child case did not pass: {stdout}");
    assert!(root.join("child-entered").is_file());
    eprintln!("ordinary integration child: {exact}; 1 passed; 0 failed; 0 ignored; reaped");
    fs::remove_dir_all(root).unwrap();
    None
}
