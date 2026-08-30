use std::fs;
use std::path::{Path, PathBuf};

use ai_toolbox_lib::coding::session_manager::kimi::{
    delete_session, export_native_snapshot, import_native_snapshot, load_messages,
    scan_recent_sessions, scan_sessions,
};
use serde_json::{json, Value};

fn sample_state_json(id: &str) -> String {
    json!({
        "session_id": id,
        "work_dir": "/workspace/demo",
        "title": "Fix the parser",
        "last_prompt": "Fix the parser",
        // Seconds epoch; parse_timestamp must normalize to milliseconds.
        "created_at": 1_756_500_000_i64,
        "updated_at": 1_756_503_600_i64,
    })
    .to_string()
}

fn write_session_dir(root: &Path, id: &str, state_json: &str) -> PathBuf {
    let session_dir = root.join("encoded-cwd").join(id);
    fs::create_dir_all(&session_dir).expect("mkdir session dir");
    fs::write(session_dir.join("state.json"), state_json).expect("write state.json");
    session_dir
}

#[test]
fn scan_sessions_parses_state_json_fields_and_resume_command() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("sessions");
    let session_dir = write_session_dir(&root, "session-1", &sample_state_json("session-1"));
    fs::write(
        session_dir.join("chat_history.jsonl"),
        "{\"role\":\"user\",\"content\":\"hello\"}\n",
    )
    .expect("write history");

    let sessions = scan_sessions(&root);
    assert_eq!(sessions.len(), 1);
    let meta = &sessions[0];
    assert_eq!(meta.provider_id, "kimi");
    assert_eq!(meta.session_id, "session-1");
    assert_eq!(meta.title.as_deref(), Some("Fix the parser"));
    assert_eq!(meta.project_dir.as_deref(), Some("/workspace/demo"));
    assert_eq!(meta.created_at, Some(1_756_500_000_000));
    assert_eq!(meta.last_active_at, Some(1_756_503_600_000));
    assert_eq!(meta.source_path, session_dir.to_string_lossy().to_string());
    assert_eq!(
        meta.resume_command.as_deref(),
        Some("cd /workspace/demo && kimi -S session-1")
    );

    let messages = load_messages(&session_dir).expect("load messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, "user");
    assert!(messages[0].content.contains("hello"));
    assert!(messages[0].id.is_some());
}

#[test]
fn scan_sessions_dedupes_state_and_summary_in_same_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("sessions");
    let session_dir = write_session_dir(&root, "session-1", &sample_state_json("session-1"));
    // summary.json describing the same session must not produce a duplicate.
    fs::write(
        session_dir.join("summary.json"),
        json!({ "session_id": "session-1", "summary": "did things" }).to_string(),
    )
    .expect("write summary.json");

    let sessions = scan_sessions(&root);
    assert_eq!(sessions.len(), 1, "one entry per session directory");
    assert_eq!(sessions[0].session_id, "session-1");

    let recent = scan_recent_sessions(&root, 10);
    assert_eq!(recent.len(), 1);
}

#[test]
fn scan_recent_sessions_respects_limit_and_early_stops() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("sessions");
    for id in ["session-1", "session-2", "session-3"] {
        write_session_dir(&root, id, &sample_state_json(id));
    }

    assert!(scan_recent_sessions(&root, 0).is_empty());

    let recent = scan_recent_sessions(&root, 1);
    assert_eq!(recent.len(), 1, "limit must be honored");

    let all = scan_recent_sessions(&root, 10);
    assert_eq!(all.len(), 3);
}

#[test]
fn load_messages_parses_roles_blocks_and_tool_calls() {
    let temp = tempfile::tempdir().expect("tempdir");
    let session_dir = temp.path().join("session-1");
    fs::create_dir_all(&session_dir).expect("mkdir");
    let history = concat!(
        "{\"role\":\"user\",\"content\":\"list files\"}\n",
        "{\"role\":\"assistant\",\"reasoning_content\":\"thinking hard\",\"content\":\"sure\",",
        "\"tool_calls\":[{\"id\":\"call-1\",\"function\":{\"name\":\"Bash\",\"arguments\":\"{\\\"command\\\":\\\"ls\\\"}\"}}]}\n",
        "{\"role\":\"tool\",\"tool_call_id\":\"call-1\",\"content\":\"a.txt\\nb.txt\"}\n",
        "not-json-line\n",
        "{\"role\":\"system\",\"content\":\"ignored\"}\n"
    );
    fs::write(session_dir.join("chat_history.jsonl"), history).expect("write history");

    let messages = load_messages(&session_dir).expect("load messages");
    assert_eq!(messages.len(), 3, "system and malformed lines are skipped");

    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "list files");

    let assistant = &messages[1];
    assert_eq!(assistant.role, "assistant");
    let kinds: Vec<&str> = assistant.blocks.iter().map(|b| b.kind.as_str()).collect();
    assert!(kinds.contains(&"thinking"), "kinds: {kinds:?}");
    assert!(kinds.contains(&"text"), "kinds: {kinds:?}");
    assert!(
        kinds.contains(&"tool_call") || kinds.contains(&"tool_execution"),
        "kinds: {kinds:?}"
    );

    let tool = &messages[2];
    assert_eq!(tool.role, "tool");
    assert!(tool.content.contains("a.txt"));
}

#[test]
fn native_snapshot_round_trip_preserves_utf8_and_binary_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("sessions");
    let session_dir = write_session_dir(&root, "session-1", &sample_state_json("session-1"));
    fs::write(
        session_dir.join("chat_history.jsonl"),
        "{\"role\":\"user\",\"content\":\"hello\"}\n",
    )
    .expect("write history");
    // Nested companion state must survive too.
    fs::create_dir_all(session_dir.join("subagents/agent-1")).expect("mkdir subagent");
    let binary_bytes: &[u8] = &[0_u8, 159, 146, 150, 255, 0x0a];
    fs::write(
        session_dir.join("subagents/agent-1/state.bin"),
        binary_bytes,
    )
    .expect("write binary state");

    let snapshot = export_native_snapshot(&root, &session_dir).expect("export ok");
    let files = snapshot
        .get("files")
        .and_then(Value::as_array)
        .expect("files array");
    assert_eq!(files.len(), 3, "all files exported, including binary");

    // UTF-8 files keep plain text content.
    let state_entry = files
        .iter()
        .find(|entry| entry.get("path").and_then(Value::as_str) == Some("state.json"))
        .expect("state.json exported");
    assert!(state_entry["content"].is_string());

    // Non-UTF-8 files use an explicit base64 payload instead of failing.
    let binary_entry = files
        .iter()
        .find(|entry| {
            entry.get("path").and_then(Value::as_str) == Some("subagents/agent-1/state.bin")
        })
        .expect("binary file exported");
    assert_eq!(binary_entry["content"]["encoding"], "base64");
    let data = binary_entry["content"]["data"]
        .as_str()
        .expect("base64 data");
    use base64::Engine as _;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(data)
        .expect("valid base64");
    assert_eq!(decoded.as_slice(), binary_bytes);

    // Import restores everything byte-for-byte.
    let target_root = temp.path().join("restored");
    import_native_snapshot(&target_root, "session-1", &snapshot).expect("import ok");
    let restored_dir = target_root.join("session-1");
    assert_eq!(
        fs::read(restored_dir.join("subagents/agent-1/state.bin")).expect("read restored"),
        binary_bytes
    );
    assert_eq!(
        fs::read_to_string(restored_dir.join("state.json")).expect("read restored state"),
        sample_state_json("session-1")
    );
}

#[test]
fn delete_session_rejects_paths_outside_sessions_root_and_keeps_idempotent() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("sessions");
    let session_dir = write_session_dir(&root, "session-1", &sample_state_json("session-1"));

    // Outside the sessions root: refused.
    let outside = temp.path().join("elsewhere/session-1");
    fs::create_dir_all(&outside).expect("mkdir outside");
    let error = delete_session(&root, &outside).expect_err("outside path must be refused");
    assert!(error.contains("outside sessions root"), "error: {error}");
    assert!(outside.exists(), "outside path must not be touched");

    // Missing path inside the root: idempotent success.
    let missing = root.join("encoded-cwd/missing");
    delete_session(&root, &missing).expect("missing path is a no-op");

    // Real session inside the root: deleted.
    delete_session(&root, &session_dir).expect("delete ok");
    assert!(!session_dir.exists());
}
