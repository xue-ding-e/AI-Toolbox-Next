use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use serde_json::Value;
use walkdir::WalkDir;

use super::message_blocks::{
    message_from_blocks, text_block, thinking_block, tool_call_block, tool_result_block,
};
use super::utils::collect_recent_files_by_modified;
use super::{assign_missing_message_ids, SessionMessage, SessionMeta};

const PROVIDER_ID: &str = "kimi";

fn is_session_meta_file(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name == "state.json" || name == "summary.json")
}

pub fn scan_sessions(root: &Path) -> Vec<SessionMeta> {
    // Full scan must keep the previous global ordering: newest first.
    let mut sessions = scan_recent_sessions(root, usize::MAX);
    sessions.sort_by_key(|meta| std::cmp::Reverse(meta.last_active_at));
    sessions
}

pub fn scan_recent_sessions(root: &Path, limit: usize) -> Vec<SessionMeta> {
    if limit == 0 || !root.is_dir() {
        return Vec::new();
    }

    // state.json and summary.json in the same session directory describe the
    // same session; over-fetch candidate files, then keep one entry per
    // directory and stop as soon as `limit` sessions parsed.
    let candidate_limit = limit.saturating_mul(3).max(limit);
    let files = collect_recent_files_by_modified(root, candidate_limit, is_session_meta_file);
    let mut seen_dirs = HashSet::new();
    let mut sessions = Vec::new();
    for path in files {
        let Ok(meta) = parse_session_meta(&path) else {
            continue;
        };
        if seen_dirs.insert(meta.source_path.clone()) {
            sessions.push(meta);
            if sessions.len() >= limit {
                break;
            }
        }
    }
    sessions
}

fn parse_session_meta(path: &Path) -> Result<SessionMeta, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let session_dir = path
        .parent()
        .ok_or_else(|| "Missing Kimi session directory".to_string())?;

    let session_id = value
        .get("session_id")
        .or_else(|| value.get("id"))
        .or_else(|| value.pointer("/info/id"))
        .and_then(Value::as_str)
        .or_else(|| session_dir.file_name().and_then(|name| name.to_str()))
        .unwrap_or_default()
        .to_string();

    let project_dir = value
        .get("work_dir")
        .or_else(|| value.get("cwd"))
        .or_else(|| value.pointer("/info/cwd"))
        .and_then(Value::as_str)
        .map(str::to_string);

    let title = value
        .get("title")
        .or_else(|| value.get("generated_title"))
        .or_else(|| value.get("last_prompt"))
        .and_then(Value::as_str)
        .map(str::to_string);

    let summary = value
        .get("summary")
        .or_else(|| value.get("session_summary"))
        .and_then(Value::as_str)
        .map(str::to_string);

    let created_at = parse_timestamp(value.get("created_at"));
    let last_active_at = parse_timestamp(
        value
            .get("updated_at")
            .or_else(|| value.get("last_active_at")),
    )
    .or(created_at);

    Ok(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id: session_id.clone(),
        title,
        summary,
        project_dir: project_dir.clone(),
        created_at,
        last_active_at,
        source_path: session_dir.to_string_lossy().to_string(),
        resume_command: Some(super::utils::build_resume_command(
            project_dir.as_deref(),
            // `kimi -S <sessionId>` per docs/plan-kimi-code-cli.md §8.1
            // (「会话恢复命令：`kimi -S <sessionId>` 或 `kimi -c`」). The id comes
            // from on-disk session metadata, so it must be shell-quoted.
            &format!("kimi -S {}", super::utils::quote_session_arg(&session_id)),
        )),
        runtime_source: None,
        runtime_distro: None,
    })
}

fn parse_timestamp(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    super::utils::parse_timestamp_to_ms(value).or_else(|| {
        value
            .as_str()
            .and_then(|raw| raw.parse::<i64>().ok())
            .map(|num| {
                if num < 1_000_000_000_000 {
                    num * 1000
                } else {
                    num
                }
            })
    })
}

pub fn load_messages(session_dir: &Path) -> Result<Vec<SessionMessage>, String> {
    let jsonl_candidates = ["chat_history.jsonl", "messages.jsonl", "history.jsonl"];
    let jsonl_path = jsonl_candidates
        .iter()
        .map(|name| session_dir.join(name))
        .find(|p| p.is_file());

    if let Some(path) = jsonl_path {
        let file =
            fs::File::open(&path).map_err(|e| format!("Failed to open {}: {e}", path.display()))?;
        let mut messages = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|e| e.to_string())?;
            let Ok(value) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let role = value
                .get("role")
                .or_else(|| value.get("type"))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();

            if role == "system" {
                continue;
            }

            let mut blocks = Vec::new();
            if let Some(reasoning) = value.get("reasoning_content").and_then(Value::as_str) {
                if !reasoning.trim().is_empty() {
                    blocks.push(thinking_block(reasoning));
                }
            }

            // Tool outputs are emitted as a tool_result block below; adding a
            // text block for the same content would render it twice.
            let is_tool_output = role == "tool";
            let mut array_tool_output: Vec<String> = Vec::new();
            if let Some(content) = value.get("content").and_then(Value::as_str) {
                if !is_tool_output && !content.trim().is_empty() {
                    blocks.push(text_block(content));
                }
            } else if let Some(content_arr) = value.get("content").and_then(Value::as_array) {
                for item in content_arr {
                    if let Some(text) = item.get("text").and_then(Value::as_str) {
                        if is_tool_output {
                            array_tool_output.push(text.to_string());
                        } else if !text.trim().is_empty() {
                            blocks.push(text_block(text));
                        }
                    }
                }
            }

            if let Some(tool_calls) = value.get("tool_calls").and_then(Value::as_array) {
                for call in tool_calls {
                    let id = call
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let name = call
                        .pointer("/function/name")
                        .or_else(|| call.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or("tool")
                        .to_string();
                    let args = call
                        .pointer("/function/arguments")
                        .or_else(|| call.get("arguments"))
                        .and_then(Value::as_str)
                        .unwrap_or("{}");
                    let input = serde_json::from_str(args)
                        .unwrap_or_else(|_| Value::String(args.to_string()));
                    blocks.push(tool_call_block(Some(id), name, Some(input)));
                }
            }

            if role == "tool" {
                let call_id = value
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let output = value
                    .get("content")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| array_tool_output.join("\n"));
                blocks.push(tool_result_block(
                    Some(call_id),
                    None,
                    Some(Value::String(output)),
                    None,
                ));
            }

            if !blocks.is_empty() {
                messages.push(message_from_blocks(
                    role,
                    parse_timestamp(value.get("created_at").or_else(|| value.get("timestamp"))),
                    blocks,
                ));
            }
        }
        assign_missing_message_ids(&mut messages, "kimi");
        return Ok(messages);
    }

    Ok(Vec::new())
}

pub fn delete_session(sessions_root: &Path, session_path: &Path) -> Result<(), String> {
    // source_path values come from scan results, but stale or forged paths must
    // not escape the sessions root.
    if !session_path.starts_with(sessions_root) {
        return Err(format!(
            "Kimi session path {} is outside sessions root {}",
            session_path.display(),
            sessions_root.display()
        ));
    }
    if !session_path.exists() {
        return Ok(());
    }
    if session_path.is_dir() {
        fs::remove_dir_all(session_path).map_err(|e| {
            format!(
                "Failed to delete Kimi session directory {}: {e}",
                session_path.display()
            )
        })?;
    } else {
        fs::remove_file(session_path).map_err(|e| {
            format!(
                "Failed to delete Kimi session file {}: {e}",
                session_path.display()
            )
        })?;
    }
    Ok(())
}

pub fn scan_messages_for_query(session_dir: &Path, query_lower: &str) -> Result<bool, String> {
    let jsonl_candidates = ["chat_history.jsonl", "messages.jsonl", "history.jsonl"];
    let jsonl_path = jsonl_candidates
        .iter()
        .map(|name| session_dir.join(name))
        .find(|p| p.is_file());

    let Some(path) = jsonl_path else {
        return Ok(false);
    };

    let file =
        fs::File::open(&path).map_err(|e| format!("Failed to open {}: {e}", path.display()))?;
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.to_lowercase().contains(query_lower) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn export_native_snapshot(sessions_root: &Path, session_path: &Path) -> Result<Value, String> {
    if !session_path.exists() {
        return Err(format!(
            "Kimi session path does not exist: {}",
            session_path.display()
        ));
    }

    let mut snapshot = serde_json::Map::new();
    snapshot.insert(
        "sessionPath".to_string(),
        Value::String(session_path.to_string_lossy().to_string()),
    );
    snapshot.insert(
        "sessionsRoot".to_string(),
        Value::String(sessions_root.to_string_lossy().to_string()),
    );

    // Copy every file in the session directory. Depth is unbounded so nested
    // companion state (e.g. subagents) survives; UTF-8 files keep their text,
    // non-UTF-8 files use an explicit base64 payload — never fail or skip.
    if session_path.is_dir() {
        let mut files = Vec::new();
        for entry in WalkDir::new(session_path)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.file_type().is_file() {
                // Snapshot paths are cross-platform JSON fields; always use
                // forward slashes (WalkDir yields backslashes on Windows).
                let rel_path = entry
                    .path()
                    .strip_prefix(session_path)
                    .unwrap_or(entry.path())
                    .to_string_lossy()
                    .replace('\\', "/");
                let bytes = fs::read(entry.path())
                    .map_err(|e| format!("Failed to read {}: {e}", entry.path().display()))?;
                let content = match String::from_utf8(bytes.clone()) {
                    Ok(text) => Value::String(text),
                    Err(_) => serde_json::json!({
                        "encoding": "base64",
                        "data": BASE64_STANDARD.encode(bytes),
                    }),
                };
                files.push(serde_json::json!({
                    "path": rel_path,
                    "content": content,
                }));
            }
        }
        snapshot.insert("files".to_string(), Value::Array(files));
    }

    Ok(Value::Object(snapshot))
}

pub fn import_native_snapshot(
    sessions_root: &Path,
    session_id: &str,
    snapshot: &Value,
) -> Result<(), String> {
    // Snapshots are user-imported JSON; never trust their session id either.
    let relative = super::utils::safe_relative_snapshot_path(session_id, "Kimi")?;
    super::utils::reject_snapshot_symlink_components(sessions_root, &relative, "Kimi")?;
    let session_dir = sessions_root.join(&relative);
    // Refuse to silently merge over an existing local session with the same id.
    if fs::symlink_metadata(&session_dir).is_ok() {
        return Err(format!("Kimi session {session_id} already exists"));
    }
    fs::create_dir_all(&session_dir).map_err(|e| {
        format!(
            "Failed to create Kimi session directory {}: {e}",
            session_dir.display()
        )
    })?;

    if let Some(files) = snapshot.get("files").and_then(Value::as_array) {
        for file_entry in files {
            let Some(rel_path) = file_entry.get("path").and_then(Value::as_str) else {
                continue;
            };
            let bytes = decode_snapshot_file_content(file_entry.get("content"))?;
            let Some(bytes) = bytes else {
                continue;
            };
            // Snapshots are user-imported JSON; never trust their paths.
            let relative_file = super::utils::safe_relative_snapshot_path(rel_path, "Kimi")?;
            super::utils::reject_snapshot_symlink_components(&session_dir, &relative_file, "Kimi")?;
            let file_path = session_dir.join(&relative_file);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directory {}: {e}", parent.display()))?;
            }
            fs::write(&file_path, bytes)
                .map_err(|e| format!("Failed to write {}: {e}", file_path.display()))?;
        }
    }

    Ok(())
}

/// Snapshot files carry either plain UTF-8 text or an explicit base64 payload
/// (`{"encoding": "base64", "data": ...}`) for non-UTF-8 files. `None` means
/// the entry has no recognizable content and should be skipped.
fn decode_snapshot_file_content(content: Option<&Value>) -> Result<Option<Vec<u8>>, String> {
    match content {
        Some(Value::String(text)) => Ok(Some(text.clone().into_bytes())),
        Some(Value::Object(map)) => {
            if map.get("encoding").and_then(Value::as_str) != Some("base64") {
                return Ok(None);
            }
            let data = map
                .get("data")
                .and_then(Value::as_str)
                .ok_or_else(|| "Base64 snapshot file is missing its data field".to_string())?;
            let bytes = BASE64_STANDARD
                .decode(data)
                .map_err(|e| format!("Failed to decode base64 snapshot file: {e}"))?;
            Ok(Some(bytes))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_messages_tool_output_renders_once() {
        let temp = tempfile::tempdir().expect("tempdir");
        let history = temp.path().join("chat_history.jsonl");
        fs::write(
            &history,
            "{\"role\":\"assistant\",\"tool_calls\":[{\"id\":\"call-1\",\"function\":{\"name\":\"Bash\",\"arguments\":\"{}\"}}]}\n\
             {\"role\":\"tool\",\"tool_call_id\":\"call-1\",\"content\":\"a.txt\\nb.txt\"}\n",
        )
        .expect("write history");

        let messages = load_messages(temp.path()).expect("load messages");
        let tool_message = messages
            .iter()
            .find(|message| message.role == "tool")
            .expect("tool message");
        let text_blocks = tool_message
            .blocks
            .iter()
            .filter(|block| block.kind == "text")
            .count();
        let result_blocks = tool_message
            .blocks
            .iter()
            .filter(|block| block.kind == "tool_result")
            .count();
        assert_eq!(text_blocks, 0, "tool output must not become a text block");
        assert_eq!(result_blocks, 1);
    }

    #[test]
    fn import_native_snapshot_rejects_unsafe_session_ids() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("sessions");
        for malicious in ["../escaped", "a/../../escaped", "/absolute", ""] {
            let snapshot = serde_json::json!({ "files": [] });
            let result = import_native_snapshot(&root, malicious, &snapshot);
            assert!(
                result.is_err(),
                "must reject unsafe session id: {malicious}"
            );
            assert!(!temp.path().join("escaped").exists());
            assert!(!temp.path().join("absolute").exists());
        }
    }

    #[test]
    fn import_native_snapshot_rejects_unsafe_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("sessions");
        for malicious in ["../escaped.txt", "a/../../escaped.txt", "/absolute.txt"] {
            let snapshot = serde_json::json!({
                "files": [{ "path": malicious, "content": "pwned" }]
            });
            let result = import_native_snapshot(&root, "session-1", &snapshot);
            assert!(result.is_err(), "must reject unsafe path: {malicious}");
            assert!(!temp.path().join("escaped.txt").exists());
            assert!(!temp.path().join("absolute.txt").exists());
        }
    }

    #[test]
    fn import_native_snapshot_accepts_nested_relative_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("sessions");
        let snapshot = serde_json::json!({
            "files": [
                { "path": "state.json", "content": "{\"id\":\"session-1\"}" },
                { "path": "subagents/agent-1/state.json", "content": "{}" }
            ]
        });
        import_native_snapshot(&root, "session-1", &snapshot).expect("import ok");
        assert!(root.join("session-1/state.json").is_file());
        assert!(root
            .join("session-1/subagents/agent-1/state.json")
            .is_file());
    }

    #[test]
    fn import_native_snapshot_refuses_existing_session() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("sessions");
        let snapshot = serde_json::json!({ "files": [] });
        import_native_snapshot(&root, "session-1", &snapshot).expect("first import ok");
        let error = import_native_snapshot(&root, "session-1", &snapshot)
            .expect_err("second import must be refused");
        assert!(error.contains("already exists"), "error: {error}");
    }

    #[cfg(unix)]
    #[test]
    fn import_native_snapshot_rejects_symlink_escape() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("sessions");
        fs::create_dir_all(&root).expect("create root");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&outside).expect("create outside");
        std::os::unix::fs::symlink(&outside, root.join("link")).expect("create symlink");

        // Session id routed through an existing symlink component must fail.
        let snapshot = serde_json::json!({
            "files": [{ "path": "state.json", "content": "{}" }]
        });
        let error = import_native_snapshot(&root, "link/evil", &snapshot)
            .expect_err("symlinked session dir must be rejected");
        assert!(error.contains("symlink"), "error: {error}");
        assert!(!outside.join("evil").exists());

        // The sessions root itself being a symlink is refused as well.
        let linked_root = temp.path().join("linked-root");
        std::os::unix::fs::symlink(&outside, &linked_root).expect("create root symlink");
        let error = import_native_snapshot(&linked_root, "session-x", &snapshot)
            .expect_err("symlinked sessions root must be rejected");
        assert!(error.contains("symlink"), "error: {error}");
    }
}
