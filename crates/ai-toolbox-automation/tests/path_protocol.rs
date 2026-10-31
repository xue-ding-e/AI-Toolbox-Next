//! Actual Linux console transports, synthetic path environment only.
//! No desktop, real profile, target file, or owner is used by these fixtures.
#![cfg(target_os = "linux")]

use serde_json::{json, Value};
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ai-path-protocol-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }

    fn run(&self, args: &[&str], input: &str) -> (ExitStatus, Vec<Value>) {
        self.run_with_home(args, input, self.0.join("home"))
    }

    fn run_with_home(&self, args: &[&str], input: &str, home: PathBuf) -> (ExitStatus, Vec<Value>) {
        let stdin = self.0.join("stdin");
        let stdout = self.0.join("stdout");
        let stderr = self.0.join("stderr");
        fs::write(&stdin, input).unwrap();
        // Regular files avoid blocking on output-pipe capacity. EOF is finite.
        let mut child = Command::new(env!("CARGO_BIN_EXE_ai-toolbox-cli"))
            .args(args)
            .env_clear()
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("APPDATA", self.0.join("config"))
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("LOCALAPPDATA", self.0.join("local"))
            .env("AI_TOOLBOX_DATA_DIR", self.0.join("data"))
            .env("AI_TOOLBOX_CACHE_DIR", self.0.join("cache"))
            .current_dir(&self.0)
            .stdin(Stdio::from(File::open(&stdin).unwrap()))
            .stdout(Stdio::from(File::create(&stdout).unwrap()))
            .stderr(Stdio::from(File::create(&stderr).unwrap()))
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                outcome => {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("path console timed out or observation failed: {outcome:?}");
                }
            }
        };
        assert_eq!(fs::metadata(&stderr).unwrap().len(), 0);
        assert!(fs::metadata(&stdout).unwrap().len() < 256 * 1024);
        let output = fs::read_to_string(&stdout).unwrap();
        let responses = output
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let mut names: Vec<_> = fs::read_dir(&self.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec!["stderr", "stdin", "stdout"],
            "console created application state"
        );
        (status, responses)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Delete only harness-owned files; retain unexpected state for diagnosis.
        for name in ["stdin", "stdout", "stderr"] {
            let _ = fs::remove_file(self.0.join(name));
        }
        let _ = fs::remove_dir(&self.0);
    }
}

fn session(path: &str) -> String {
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"fixture","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
            "name":"backup_normalize_path","arguments":{"path":path}}}),
    ];
    requests
        .iter()
        .map(|request| format!("{request}\n"))
        .collect()
}

#[test]
fn actual_cli_and_mcp_share_host_expansion_and_portable_results() {
    let fixture = Fixture::new();
    for (input, expected) in [
        ("$HOME/missing/file", "~/missing/file"),
        ("${APPDATA}/missing/file", "%APPDATA%/missing/file"),
        ("relative/../中文", "relative/../中文"),
        ("$UNKNOWN/file", "$UNKNOWN/file"),
    ] {
        let (cli_status, cli) = fixture.run(
            &[
                "--format",
                "json",
                "backup",
                "normalize-path",
                "--path",
                input,
            ],
            "",
        );
        assert!(cli_status.success());
        assert_eq!(cli.len(), 1);
        assert_eq!(cli[0]["ok"], true);
        assert_eq!(cli[0]["data"]["path"], expected);
        let (mcp_status, mcp) = fixture.run(&["mcp", "serve"], &session(input));
        assert!(mcp_status.success());
        assert_eq!(mcp.len(), 2, "notifications must not receive a response");
        assert_eq!(mcp[1]["id"], 2);
        assert_eq!(mcp[1]["result"]["isError"], false);
        assert_eq!(mcp[1]["result"]["structuredContent"], cli[0]["data"]);
    }
}

#[test]
fn oversized_adapter_input_fails_without_target_state_or_successful_path() {
    let fixture = Fixture::new();
    let input = "x".repeat(ai_toolbox_automation::MAX_PATH_BYTES + 1);
    let (status, cli) = fixture.run(
        &[
            "--format",
            "json",
            "backup",
            "normalize-path",
            "--path",
            &input,
        ],
        "",
    );
    assert_eq!(status.code(), Some(2));
    assert_eq!(cli[0]["ok"], false);
    assert_eq!(cli[0]["data"], Value::Null);
    let (status, mcp) = fixture.run(&["mcp", "serve"], &session(&input));
    assert!(
        status.success(),
        "a protocol error does not terminate the server"
    );
    assert_eq!(mcp[1]["error"]["code"], -32602);
    assert_eq!(mcp[1]["result"], Value::Null);
}

#[test]
fn decoded_unicode_and_backslash_boundaries_agree_despite_larger_json_frames() {
    let fixture = Fixture::new();
    for input in [
        "é".repeat(ai_toolbox_automation::MAX_PATH_BYTES / 2),
        "\\".repeat(ai_toolbox_automation::MAX_PATH_BYTES),
    ] {
        assert_eq!(input.len(), ai_toolbox_automation::MAX_PATH_BYTES);
        let frame = session(&input).replace('é', "\\u00e9");
        assert!(frame.len() > ai_toolbox_automation::MAX_PATH_BYTES);
        assert!(frame.len() < ai_toolbox_automation::mcp::MAX_MESSAGE_BYTES);
        let (status, cli) = fixture.run(
            &[
                "--format",
                "json",
                "backup",
                "normalize-path",
                "--path",
                &input,
            ],
            "",
        );
        assert!(status.success());
        let (status, mcp) = fixture.run(&["mcp", "serve"], &frame);
        assert!(status.success());
        assert_eq!(mcp[1]["result"]["isError"], false);
        assert_eq!(mcp[1]["result"]["structuredContent"], cli[0]["data"]);
    }
    let too_large = "é".repeat(ai_toolbox_automation::MAX_PATH_BYTES / 2 + 1);
    let (status, cli) = fixture.run(
        &[
            "--format",
            "json",
            "backup",
            "normalize-path",
            "--path",
            &too_large,
        ],
        "",
    );
    assert_eq!(status.code(), Some(2));
    assert_eq!(cli[0]["ok"], false);
    let (_, mcp) = fixture.run(
        &["mcp", "serve"],
        &session(&too_large).replace('é', "\\u00e9"),
    );
    assert_eq!(mcp[1]["error"]["code"], -32602);
}

#[test]
fn actual_service_expansion_rejection_is_not_reported_as_a_path() {
    let fixture = Fixture::new();
    let home = fixture.0.join("h".repeat(512));
    let input = "$HOME/".repeat(5000);
    assert!(input.len() < ai_toolbox_automation::MAX_PATH_BYTES);
    let (status, cli) = fixture.run_with_home(
        &[
            "--format",
            "json",
            "backup",
            "normalize-path",
            "--path",
            &input,
        ],
        "",
        home.clone(),
    );
    assert_eq!(status.code(), Some(2));
    assert_eq!(cli[0]["ok"], false);
    assert_eq!(cli[0]["data"], Value::Null);
    assert_eq!(cli[0]["error"]["stage"], "dispatch");
    let (status, mcp) = fixture.run_with_home(&["mcp", "serve"], &session(&input), home);
    assert!(status.success());
    assert_eq!(mcp[1]["result"]["isError"], true);
    assert_eq!(mcp[1]["result"]["structuredContent"], Value::Null);
    assert_eq!(
        mcp[1]["result"]["content"][0]["text"],
        cli[0]["error"]["message"]
    );
}

#[test]
fn nul_is_rejected_by_cli_parser_and_actual_mcp_before_path_service() {
    // OS argv cannot carry NUL. Exercise the real parser in-process instead
    // of pretending a spawned CLI received an impossible native argument.
    let args: Vec<_> = [
        "--format",
        "json",
        "backup",
        "normalize-path",
        "--path",
        "fixture\0path",
    ]
    .iter()
    .map(std::ffi::OsString::from)
    .collect();
    let response = ai_toolbox_automation::cli::run(&args);
    assert_eq!(response.envelope.exit_code(), 2);
    assert!(response.envelope.result.is_err());
    let fixture = Fixture::new();
    let (status, mcp) = fixture.run(&["mcp", "serve"], &session("fixture\0path"));
    assert!(status.success());
    assert_eq!(mcp[1]["error"]["code"], -32602);
    assert_eq!(mcp[1]["result"], Value::Null);
}

#[test]
fn actual_gateway_cli_and_mcp_query_share_results_without_initialization() {
    let fixture = Fixture::new();
    let (status, cli) = fixture.run(&["--format", "json", "gateway", "supported-cli-keys"], "");
    assert!(status.success());
    assert_eq!(cli.len(), 1);
    assert_eq!(cli[0]["ok"], true);
    assert_eq!(cli[0]["operation"], "gateway.supported_cli_keys");
    assert_eq!(cli[0]["data"], json!({"cli_keys":["claude", "claude_desktop", "codex", "grok", "kimi", "gemini"]}));
    let mut requests = session("unused").lines().take(2).map(|line| format!("{line}\n")).collect::<String>();
    for (id, arguments) in [(2, json!({})), (3, json!({"path":"fixture"})), (4, Value::Null)] {
        requests.push_str(&format!("{}\n", json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{
            "name":"gateway_supported_cli_keys","arguments":arguments}})));
    }
    requests.push_str(&format!("{}\n", json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{
        "name":"gateway_supported_cli_keys"}})));
    let (status, mcp) = fixture.run(&["mcp", "serve"], &requests);
    assert!(status.success());
    assert_eq!(mcp.len(), 5);
    for index in [1, 4] {
        assert_eq!(mcp[index]["result"]["structuredContent"], cli[0]["data"]);
        assert_eq!(mcp[index]["result"]["isError"], false);
        assert_eq!(serde_json::from_str::<Value>(mcp[index]["result"]["content"][0]["text"].as_str().unwrap()).unwrap(), cli[0]["data"]);
    }
    for index in [2, 3] {
        assert_eq!(mcp[index]["error"]["code"], -32602);
        assert_eq!(mcp[index]["result"], Value::Null);
    }
    for extra in [&["--path", "fixture"][..], &["extra"][..], &["--profile", "fixture"][..], &["--unknown"][..]] {
        let mut args = vec!["--format", "json", "gateway", "supported-cli-keys"];
        args.extend_from_slice(extra);
        let (status, result) = fixture.run(&args, "");
        assert_eq!(status.code(), Some(2));
        assert_eq!(result[0]["ok"], false);
        assert_eq!(result[0]["data"], Value::Null);
    }
}
