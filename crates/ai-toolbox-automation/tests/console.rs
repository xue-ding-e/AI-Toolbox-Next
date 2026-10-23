//! Real process tests: never use the operator's home or application data.
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ai-toolbox-console-中文 space-{}-{nonce}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("home")).unwrap();
        Self { root }
    }

    fn run(&self, args: &[OsString]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ai-toolbox-cli"));
        command
            .args(args)
            .env_clear()
            .current_dir(&self.root)
            .stdin(Stdio::null());
        for name in ["HOME", "USERPROFILE"] {
            command.env(name, self.root.join("home"));
        }
        for name in [
            "AI_TOOLBOX_DATA_DIR",
            "AI_TOOLBOX_CACHE_DIR",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_CACHE_HOME",
            "APPDATA",
            "LOCALAPPDATA",
            "CODEX_HOME",
            "CLAUDE_CONFIG_DIR",
            "TMP",
            "TEMP",
            "TMPDIR",
        ] {
            command.env(name, self.root.join(name));
        }
        // Windows loader may need these; never inherit PATH, auth, proxy, or GUI environment.
        #[cfg(windows)]
        for name in ["SYSTEMROOT", "WINDIR"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {}
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("failed to observe console child: {error}");
                }
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("console child exceeded five-second deadline");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // This foundation emits short output and never spawns descendants.
        // This is not a general process-tree watchdog: inherited pipe handles
        // or output larger than pipe capacity need a different harness.
        let output = child.wait_with_output().unwrap();
        assert_eq!(
            fs::read_dir(self.root.join("home")).unwrap().count(),
            0,
            "child wrote to HOME"
        );
        let entries: Vec<_> = fs::read_dir(&self.root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(
            entries,
            vec![OsString::from("home")],
            "child created state files/directories"
        );
        output
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        // Only remove empty fixture directories. Unexpected child-created data
        // is retained for diagnosis rather than recursively deleting it.
        let _ = fs::remove_dir(self.root.join("home"));
        let _ = fs::remove_dir(&self.root);
    }
}

fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn help_and_version_have_no_application_initialization() {
    let sandbox = Sandbox::new();
    for argument in ["--help", "-h", "--version", "-V"] {
        let output = sandbox.run(&args(&[argument]));
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("foundation"));
        assert!(!text.contains('\u{1b}'));
    }
    let version = sandbox.run(&args(&["--format", "json", "--version"]));
    assert_eq!(String::from_utf8(version.stdout).unwrap(), format!("{{\"schema_version\":1,\"operation\":\"automation.version\",\"request_id\":null,\"ok\":true,\"data\":{{\"name\":\"ai-toolbox-cli\",\"version\":\"{}\",\"stage\":\"foundation\"}},\"warnings\":[],\"error\":null}}\n", env!("CARGO_PKG_VERSION")));
    assert!(version.stderr.is_empty());
}

#[test]
fn malformed_and_unknown_arguments_have_stable_redacted_json_errors() {
    let sandbox = Sandbox::new();
    let invalid_cases = [
        vec![],
        vec!["--format"],
        vec!["--format", "yaml"],
        vec!["--unknown"],
        vec!["invoke", "get_settings"],
        vec!["--help", "extra"],
        vec!["--version", "--help"],
        vec!["capabilities", "extra"],
        vec!["codex"],
        vec!["codex", "status", "--repair"],
        vec!["daemon", "status"],
        vec!["daemon", "status", "--profile", "relative", "--endpoint-dir", "/missing"],
        vec!["daemon", "status", "--profile", "/missing", "--endpoint-dir", "relative"],
        vec!["daemon", "status", "--profile", "/missing", "--profile", "/missing", "--endpoint-dir", "/missing"],
        vec!["daemon", "status", "--profile", "/missing", "--endpoint-dir", "/missing", "--endpoint-dir", "/missing"],
        vec!["daemon", "status", "--profile", "/missing", "--endpoint-dir", "/missing", "--repair"],
        vec!["--help", "--profile", "/never-inspected"],
        vec!["secret=never-echo-我\n"],
        vec!["--format", "text", "capabilities"],
    ];
    let expected = "{\"schema_version\":1,\"operation\":null,\"request_id\":null,\"ok\":false,\"data\":null,\"warnings\":[],\"error\":{\"code\":\"INVALID_INPUT\",\"stage\":\"parse\",\"retryable\":false,\"message\":\"Invalid arguments. Use --help for the supported syntax.\"}}\n";
    for invalid in invalid_cases {
        let mut command = args(&["--format", "json"]);
        command.extend(args(&invalid));
        for _ in 0..2 {
            let output = sandbox.run(&command);
            assert_eq!(output.status.code(), Some(2), "{invalid:?}");
            assert!(output.stderr.is_empty());
            assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
        }
    }
}

#[test]
fn text_errors_use_stderr_and_no_arguments_are_not_success() {
    let sandbox = Sandbox::new();
    for command in [vec![], vec!["not-a-command"], vec!["--format"]] {
        let output = sandbox.run(&args(&command));
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "INVALID_INPUT [parse]: Invalid arguments. Use --help for the supported syntax.\n"
        );
    }
}

#[test]
fn reserved_business_operation_fails_explicitly_without_state_changes() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&args(&["codex", "status", "--format", "json"]));
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stderr.is_empty());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "{\"schema_version\":1,\"operation\":\"codex.status\",\"request_id\":null,\"ok\":false,\"data\":null,\"warnings\":[],\"error\":{\"code\":\"NOT_SUPPORTED\",\"stage\":\"dispatch\",\"retryable\":false,\"message\":\"Codex business services are not connected in this foundation.\"}}\n");
}

#[test]
fn discovery_does_not_claim_business_or_mcp_support() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&args(&["--format", "json", "capabilities"]));
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    let expected = concat!(
        r#"{"schema_version":1,"operation":"automation.capabilities","request_id":null,"ok":true,"data":{"stage":"foundation","business_coverage":"none","mcp_supported":false,"operations":["#,
        r#"{"operation_id":"automation.help","command":"--help","supported":true,"side_effects":false,"input":"none"},"#,
        r#"{"operation_id":"automation.version","command":"--version","supported":true,"side_effects":false,"input":"none"},"#,
        r#"{"operation_id":"automation.capabilities","command":"capabilities","supported":true,"side_effects":false,"input":"none"},"#,
        r#"{"operation_id":"codex.status","command":"codex status","supported":false,"side_effects":false,"input":"none"},"#,
    );
    let expected = format!("{expected}{{\"operation_id\":\"daemon.status\",\"command\":\"daemon status\",\"supported\":{},\"side_effects\":false,\"input\":\"explicit_absolute_profile_and_private_endpoint_dir\",\"status_scope\":\"profile_lease_only\",\"daemon_lifecycle_complete\":false,\"platforms\":[\"linux\"]}}]}},\"warnings\":[],\"error\":null}}\n", cfg!(target_os = "linux"));
    assert_eq!(text, expected);
}

#[cfg(not(target_os = "linux"))]
#[test]
fn owner_status_is_explicitly_unsupported_without_path_inspection() {
    let sandbox = Sandbox::new();
    let mut command = args(&["--format", "json", "daemon", "status", "--profile"]);
    command.push(sandbox.root.join("missing-profile").into_os_string());
    command.push("--endpoint-dir".into());
    command.push(sandbox.root.join("missing-endpoint").into_os_string());
    let output = sandbox.run(&command);
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["operation"], "daemon.status");
    assert_eq!(value["error"]["code"], "NOT_SUPPORTED");
}

#[cfg(unix)]
#[test]
fn non_unicode_arguments_are_rejected_without_panic_or_lossy_echo() {
    use std::os::unix::ffi::OsStringExt;
    let sandbox = Sandbox::new();
    let mut command = args(&["--format", "json"]);
    command.push(OsString::from_vec(vec![0xff]));
    let output = sandbox.run(&command);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("\"code\":\"INVALID_INPUT\""));
}
