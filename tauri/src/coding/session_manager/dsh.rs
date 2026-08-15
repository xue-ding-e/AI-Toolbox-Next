//! Dsh session scanning/loading.
//!
//! dsh persists every session as a versioned JSONL artifact under its sessions
//! root (`<home>/sessions`):
//!   `<home>/sessions/<project-key>/<encoded-session-id>/session.jsonl[.zstd]`
//!
//! The first line is a `{type:"session", version, id, createdAt, cwd...}`
//! header; every following line is a `StorageRecord` — either a full
//! `SessionEvent` (`{type, seq, time, data, surfaceOp?}`) or a packed chunk
//! row (`text-chunks` / `reasoning-chunks` / `tool-call-chunks`) that only
//! carries streaming deltas. zstd frames are the default encoding, so both
//! `.jsonl.zstd` and plain `.jsonl` artifacts are covered. All read paths
//! degrade silently to an empty list when the data is unreachable.
//!
//! Limitations: dsh titles live in a separate projection store, so the list
//! title falls back to the first user message; last-active is approximated by
//! the artifact's file mtime, and the tail timestamp is not read for zstd
//! artifacts (a bounded head read keeps the scan cheap). Compaction `replace`
//! surface ops are surfaced as-is, which may repeat long-ago turns in loaded
//! transcripts.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::message_blocks::text_message;
use super::utils::{extract_text, parse_timestamp_to_ms, text_contains_query, truncate_summary};
use super::{assign_missing_message_ids, SessionMessage, SessionMeta};

const PROVIDER_ID: &str = "dsh";
const TITLE_MAX_CHARS: usize = 80;
// Bound the scan pass to the header plus a handful of early events; titles and
// creation metadata never sit deeper than a session's opening events.
const HEAD_LINES: usize = 40;

fn is_session_artifact_name(name: &str) -> bool {
    name == "session.jsonl" || name == "session.jsonl.zstd"
}

fn is_session_artifact(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(is_session_artifact_name)
        .unwrap_or(false)
}

/// Open a session artifact for buffered text reading, decompressing zstd when
/// the file is a `.zstd` artifact.
fn open_session_reader(path: &Path) -> std::io::Result<Box<dyn BufRead>> {
    let file = File::open(path)?;
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
    if name.ends_with(".zstd") {
        let decoder = zstd::stream::read::Decoder::new(file)?;
        Ok(Box::new(BufReader::new(decoder)))
    } else {
        Ok(Box::new(BufReader::new(file)))
    }
}

/// Read up to `max_lines` text lines from an artifact (header plus early events).
fn read_head_lines(path: &Path, max_lines: usize) -> std::io::Result<Vec<String>> {
    let mut reader = open_session_reader(path)?;
    let mut lines = Vec::new();
    let mut line = String::new();
    while lines.len() < max_lines {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            break;
        }
        lines.push(line.clone());
    }
    Ok(lines)
}

/// Recursively collect every session artifact under `root`.
fn collect_session_artifacts(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if is_session_artifact(&path) {
                files.push(path);
            }
        }
    }
    files
}

fn file_modified_ms(path: &Path) -> Option<i64> {
    let modified = path.metadata().ok()?.modified().ok()?;
    Some(modified.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64)
}

/// Extract a surface message (role + text + ms timestamp) from a `SessionEvent`,
/// tolerating both `data.message`-wrapped and `data`-as-message shapes.
fn extract_surface_message(event: &Value) -> Option<(String, String, Option<i64>)> {
    let event_type = event.get("type").and_then(Value::as_str)?;
    if !matches!(
        event_type,
        "user/message" | "assistant/message" | "tool/result"
    ) {
        return None;
    }
    let data = event.get("data")?;
    let message = data
        .get("message")
        .filter(|message| message.is_object())
        .unwrap_or(data);
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| match event_type {
            "user/message" => "user",
            "assistant/message" => "assistant",
            _ => "tool",
        }.to_string());
    let text = message
        .get("content")
        .map(extract_text)
        .unwrap_or_default();
    if text.trim().is_empty() {
        return None;
    }
    let ts = event.get("time").and_then(parse_timestamp_to_ms);
    Some((role, text, ts))
}

/// Build the `SessionMeta` for one artifact from its header line plus early
/// events and file mtime.
fn parse_session_artifact(path: &Path) -> Option<SessionMeta> {
    let head = read_head_lines(path, HEAD_LINES).ok()?;

    let mut session_id: Option<String> = None;
    let mut created_at: Option<i64> = None;
    let mut cwd: Option<String> = None;
    let mut first_user: Option<String> = None;

    for raw_line in &head {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line).ok()?;
        let event_type = value.get("type").and_then(Value::as_str).unwrap_or("");
        if event_type == "session" {
            if session_id.is_none() {
                session_id = value.get("id").and_then(Value::as_str).map(str::to_string);
            }
            if created_at.is_none() {
                created_at = value.get("createdAt").and_then(parse_timestamp_to_ms);
            }
            if cwd.is_none() {
                cwd = value
                    .get("cwd")
                    .and_then(Value::as_str)
                    .filter(|text| !text.trim().is_empty())
                    .map(str::to_string);
            }
        }
        if first_user.is_none() {
            if let Some((role, text, _)) = extract_surface_message(&value) {
                if role == "user" && !text.trim().is_empty() {
                    first_user = Some(truncate_summary(&text, TITLE_MAX_CHARS).to_string());
                }
            }
        }
    }

    let session_id = session_id?;
    let created_at = created_at.unwrap_or_else(|| file_modified_ms(path).unwrap_or(0));
    let last_active_at = file_modified_ms(path)
        .filter(|ts| *ts >= created_at)
        .or(Some(created_at));

    Some(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id,
        title: first_user.clone(),
        summary: first_user,
        project_dir: cwd,
        created_at: Some(created_at),
        last_active_at,
        source_path: path.to_string_lossy().to_string(),
        resume_command: None,
        runtime_source: None,
        runtime_distro: None,
    })
}

/// Scan every dsh session artifact under `root` into `SessionMeta` entries.
pub fn scan_sessions(root: &Path) -> Vec<SessionMeta> {
    collect_session_artifacts(root)
        .into_iter()
        .filter_map(|path| parse_session_artifact(&path))
        .collect()
}

/// Scan recent sessions, newest first. Reuses the full scan and truncates.
pub fn scan_recent_sessions(root: &Path, limit: usize) -> Vec<SessionMeta> {
    if limit == 0 {
        return Vec::new();
    }

    let mut sessions = scan_sessions(root);
    sessions.sort_by(|left, right| {
        let left_ts = left.last_active_at.or(left.created_at).unwrap_or(0);
        let right_ts = right.last_active_at.or(right.created_at).unwrap_or(0);
        right_ts.cmp(&left_ts)
    });
    sessions.truncate(limit);
    sessions
}

/// Load the surface messages from a dsh session artifact.
pub fn load_messages(source: &str) -> Result<Vec<SessionMessage>, String> {
    let path = Path::new(source);
    let mut reader = open_session_reader(path)
        .map_err(|error| format!("Failed to open dsh session file {source}: {error}"))?;

    let mut messages = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|error| format!("Failed to read dsh session file {source}: {error}"))?;
        if read == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if let Some((role, text, ts)) = extract_surface_message(&value) {
            messages.push(text_message(role, text, ts));
        }
    }

    assign_missing_message_ids(&mut messages, PROVIDER_ID);
    Ok(messages)
}

/// Test whether a session artifact's text content contains the given query.
pub fn scan_messages_for_query(source: &str, query_lower: &str) -> Result<bool, String> {
    let path = Path::new(source);
    let mut reader = open_session_reader(path)
        .map_err(|error| format!("Failed to open dsh session file {source}: {error}"))?;

    let mut line = String::new();
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|error| format!("Failed to read dsh session file {source}: {error}"))?;
        if read == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let text = extract_surface_message(&value)
            .map(|(_, text, _)| text)
            .unwrap_or_default();
        if text_contains_query(&text, query_lower) {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Delete a dsh session by removing its owning artifact directory, guarded to
/// the artifact name and a location under `root`.
pub fn delete_session(root: &Path, source: &str) -> Result<(), String> {
    let path = Path::new(source);
    let artifact_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Invalid dsh session path".to_string())?;
    if !is_session_artifact_name(artifact_name) {
        return Err("Not a dsh session artifact".to_string());
    }
    let session_dir = path
        .parent()
        .ok_or_else(|| "Invalid dsh session directory".to_string())?;
    if !session_dir.starts_with(root) {
        return Err("dsh session directory is outside the sessions root".to_string());
    }
    std::fs::remove_dir_all(session_dir)
        .map_err(|error| format!("Failed to delete dsh session {}: {error}", session_dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zstd_bytes(payload: &[u8]) -> Vec<u8> {
        zstd::stream::encode_all(std::io::Cursor::new(payload), 0).expect("zstd encode")
    }

    fn write_artifact(dir: &Path, session_id: &str, compressed: bool, events: &str) {
        let session_dir = dir.join(session_id);
        std::fs::create_dir_all(&session_dir).expect("create session dir");
        let path = if compressed {
            session_dir.join("session.jsonl.zstd")
        } else {
            session_dir.join("session.jsonl")
        };
        let payload = if compressed {
            zstd_bytes(events.as_bytes())
        } else {
            events.as_bytes().to_vec()
        };
        std::fs::write(&path, payload).expect("write artifact");
    }

    fn sample_events(created: i64) -> String {
        format!(
            r#"{{"type":"session","id":"s1","createdAt":{created},"cwd":"/home/user/proj","delegationDepth":0}}
{{"type":"request/header","seq":0,"time":1,"data":{{"header":{{"config":{{"model":"x"}}}},"reason":"initial"}}}}
{{"type":"user/message","seq":1,"time":5,"data":{{"role":"user","content":[{{"type":"text","text":"Hello there"}}],"source":{{"kind":"user"}}}},"surfaceOp":"append"}}
{{"type":"assistant/message","seq":2,"time":9,"data":{{"content":[{{"type":"text","text":"Hi back"}}]}},"surfaceOp":"append"}}
"#
        )
    }

    #[test]
    fn scan_parses_zstd_artifact() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_artifact(dir.path(), "proj-a", true, &sample_events(1_700_000_000_000));
        let sessions = scan_sessions(dir.path());
        assert_eq!(sessions.len(), 1);
        let meta = &sessions[0];
        assert_eq!(meta.session_id, "s1");
        assert_eq!(meta.project_dir.as_deref(), Some("/home/user/proj"));
        assert_eq!(meta.created_at.unwrap(), 1_700_000_000_000);
        assert_eq!(meta.title.as_deref(), Some("Hello there"));
    }

    #[test]
    fn load_messages_extracts_surface_messages() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_artifact(dir.path(), "proj-a", true, &sample_events(1_700_000_000_000));
        let path = collect_session_artifacts(dir.path())[0].clone();
        let messages = load_messages(&path.to_string_lossy()).expect("load");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Hello there");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "Hi back");
        assert!(messages[0].id.is_some());
    }

    #[test]
    fn scan_messages_query_matches() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_artifact(dir.path(), "proj-a", false, &sample_events(1_700_000_000_000));
        let path = collect_session_artifacts(dir.path())[0].clone();
        let source = path.to_string_lossy();
        assert!(scan_messages_for_query(&source, "hi back").expect("scan"));
        assert!(!scan_messages_for_query(&source, "zzz").expect("scan"));
    }

    #[test]
    fn delete_removes_session_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_artifact(dir.path(), "proj-a", true, &sample_events(1_700_000_000_000));
        let path = collect_session_artifacts(dir.path())[0].clone();
        delete_session(dir.path(), &path.to_string_lossy()).expect("delete");
        assert!(!path.parent().unwrap().exists());
        assert!(scan_sessions(dir.path()).is_empty());
    }
}