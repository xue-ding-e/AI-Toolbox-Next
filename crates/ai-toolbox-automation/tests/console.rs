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
fn discovery_limits_business_and_mcp_support_to_shared_path_service() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&args(&["--format", "json", "capabilities"]));
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    let expected = concat!(
        r#"{"schema_version":1,"operation":"automation.capabilities","request_id":null,"ok":true,"data":{"stage":"foundation","business_coverage":"partial","mcp_supported":true,"mcp_protocol_version":"2025-11-25","mcp_tools":["backup_normalize_path"],"operations":["#,
        r#"{"operation_id":"automation.help","command":"--help","supported":true,"side_effects":false,"input":"none"},"#,
        r#"{"operation_id":"automation.version","command":"--version","supported":true,"side_effects":false,"input":"none"},"#,
        r#"{"operation_id":"automation.capabilities","command":"capabilities","supported":true,"side_effects":false,"input":"none"},"#,
        r#"{"operation_id":"codex.status","command":"codex status","supported":false,"side_effects":false,"input":"none"},"#,
        r#"{"operation_id":"backup.normalize_path","command":"backup normalize-path --path <path>","supported":true,"side_effects":false,"input":"explicit_path_max_32768_bytes"},"#,
    );
    let expected = format!("{expected}{{\"operation_id\":\"daemon.status\",\"command\":\"daemon status\",\"supported\":{},\"side_effects\":false,\"input\":\"explicit_absolute_profile_and_private_endpoint_dir\",\"status_scope\":\"profile_lease_only\",\"daemon_lifecycle_complete\":false,\"platforms\":[\"linux\"]}}]}},\"warnings\":[],\"error\":null}}\n", cfg!(target_os = "linux"));
    // Preserve the pre-existing operation contracts exactly. Context additions
    // explicitly distinguish the local client from unchecked owner readiness.
    let mut expected: serde_json::Value = serde_json::from_str(&expected).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let operations = value["data"]["operations"].as_array().unwrap();
    assert_eq!(operations.len(), 9);
    for (operation_id, command, side_effects) in [
        ("codex.context.inspect", "codex context inspect", false),
        ("codex.context.set", "codex context set", false),
        ("codex.context.request_status", "codex context request-status", false),
    ] {
        expected["data"]["operations"].as_array_mut().unwrap().push(serde_json::json!({
            "operation_id": operation_id, "command": command,
            "supported": false, "side_effects": side_effects,
            "input": "explicit_owner_paths_and_closed_context_arguments",
            "local_client_supported": cfg!(target_os = "linux") && operation_id != "codex.context.set",
            "owner_readiness": "not_checked", "owner_write_enablement": "not_established",
            "platforms": ["linux"]
        }));
    }
    assert_eq!(value, expected);
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

#[test]
fn context_structural_errors_are_redacted_and_never_initialize_state() {
    let sandbox = Sandbox::new();
    let valid_set = vec!["codex", "context", "set", "--profile", "/missing-profile", "--endpoint-dir", "/missing-endpoint",
        "--request-id", "request_1", "--expected-provider-id", "provider_1", "--expected-revision",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "--target", "common", "--model-context-window", "1000000"];
    for (option, value) in [
        ("--target", "secret-target-never-echo"),
        ("--model-context-window", "1.5"), ("--model-context-window", "-1"),
        ("--model-context-window", "+1"), ("--model-context-window", "1e6"),
        ("--model-context-window", "18446744073709551616"),
        ("--profile", "relative"), ("--endpoint-dir", "relative"),
    ] {
        let mut command = valid_set.clone();
        let index = command.iter().position(|argument| *argument == option).unwrap();
        command[index + 1] = value;
        let mut command = args(&command);
        command.extend(args(&["--format", "json"]));
        let output = sandbox.run(&command);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stderr.is_empty());
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["error"]["code"], "INVALID_INPUT");
        assert!(value["request_id"].is_null());
        assert!(!String::from_utf8(output.stdout).unwrap().contains("secret-target"));
    }
    for option in ["--profile", "--endpoint-dir", "--request-id", "--expected-provider-id", "--expected-revision", "--target", "--model-context-window"] {
        let mut command = args(&valid_set);
        command.extend(args(&[option, "duplicate-secret-never-echo", "--format", "json"]));
        let output = sandbox.run(&command);
        assert_eq!(output.status.code(), Some(2));
        assert!(!String::from_utf8(output.stdout).unwrap().contains("duplicate-secret"));
        let index = valid_set.iter().position(|argument| *argument == option).unwrap();
        let mut missing = valid_set.clone();
        missing.drain(index..index + 2);
        let mut command = args(&missing);
        command.extend(args(&["--format", "json"]));
        assert_eq!(sandbox.run(&command).status.code(), Some(2));
    }
    for command in [
        vec!["codex", "context", "inspect", "--request-id", "extra"],
        vec!["codex", "context", "request-status", "--request-id", "request_1", "--target", "common"],
        vec!["daemon", "status", "--request-id", "extra"],
        vec!["--help", "--request-id", "extra"],
        vec!["capabilities", "--target", "common"],
    ] {
        let mut command = args(&command);
        command.extend(args(&["--profile", "/missing", "--endpoint-dir", "/missing", "--format", "json"]));
        assert_eq!(sandbox.run(&command).status.code(), Some(2));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn context_semantics_use_shared_validation_before_path_inspection() {
    let sandbox = Sandbox::new();
    let base = args(&["--format", "json", "codex", "context", "set", "--profile", "/missing", "--endpoint-dir", "/missing",
        "--request-id", "request_1", "--expected-provider-id", "provider_1", "--expected-revision",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "--target", "common", "--model-context-window", "1000000", "--model-auto-compact-token-limit", "900000"]);
    let reserved = sandbox.run(&base);
    assert_eq!(reserved.status.code(), Some(3));
    let reserved: serde_json::Value = serde_json::from_slice(&reserved.stdout).unwrap();
    assert_eq!(reserved["error"]["code"], "NOT_SUPPORTED");
    assert_eq!(reserved["error"]["stage"], "dispatch");
    assert!(reserved["data"].is_null());
    for (option, value) in [
        ("--request-id", ""), ("--request-id", "secret/id"), ("--request-id", "中文"),
        ("--expected-provider-id", ""), ("--expected-provider-id", "private/provider"),
        ("--expected-revision", "short"),
        ("--expected-revision", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
        ("--expected-revision", "gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg"),
        ("--model-context-window", "0"), ("--model-context-window", "9223372036854775808"),
        ("--model-auto-compact-token-limit", "1000001"),
    ] {
        let mut command = base.clone();
        let index = command.iter().position(|argument| argument == option).unwrap();
        command[index + 1] = value.into();
        let output = sandbox.run(&command);
        assert_eq!(output.status.code(), Some(2), "{option}");
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["error"]["stage"], "parse");
        assert!(value["operation"].is_null());
        assert!(value["request_id"].is_null());
    }
    for invalid_id in ["".to_owned(), "secret/id".to_owned(), "x".repeat(129)] {
        let mut command = args(&["--format", "json", "codex", "context", "request-status", "--profile", "/missing", "--endpoint-dir", "/missing", "--request-id"]);
        command.push(invalid_id.into());
        assert_eq!(sandbox.run(&command).status.code(), Some(2));
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
fn context_commands_are_unsupported_without_path_inspection() {
    let sandbox = Sandbox::new();
    for (command, extra) in [
        ("inspect", vec![]),
        ("request-status", vec!["--request-id", "request_1"]),
        ("set", vec!["--request-id", "request_1", "--expected-provider-id", "provider_1", "--expected-revision",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "--target", "common", "--model-context-window", "1000000"]),
    ] {
        let mut command = args(&["--format", "json", "codex", "context", command, "--profile"]);
        command.push(sandbox.root.join("missing-profile").into_os_string());
        command.push("--endpoint-dir".into());
        command.push(sandbox.root.join("missing-endpoint").into_os_string());
        command.extend(args(&extra));
        let output = sandbox.run(&command);
        assert_eq!(output.status.code(), Some(3));
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["error"]["code"], "NOT_SUPPORTED");
        assert_eq!(value["error"]["stage"], "dispatch");
    }
}
