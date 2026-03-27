use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde_json::Value;

use super::utils::{
    extract_text, parse_timestamp_to_ms, path_basename, read_head_tail_lines, truncate_summary,
};
use super::{SessionMessage, SessionMeta};

const PROVIDER_ID: &str = "openclaw";

pub fn scan_sessions(agents_root: &Path) -> Vec<SessionMeta> {
    if !agents_root.exists() {
        return Vec::new();
    }

    let mut sessions = Vec::new();
    let agent_entries = match std::fs::read_dir(agents_root) {
        Ok(entries) => entries,
        Err(_) => return sessions,
    };

    for agent_entry in agent_entries.flatten() {
        let agent_path = agent_entry.path();
        if !agent_path.is_dir() {
            continue;
        }

        let sessions_dir = agent_path.join("sessions");
        if !sessions_dir.is_dir() {
            continue;
        }

        let session_entries = match std::fs::read_dir(&sessions_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in session_entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                continue;
            }
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name == "sessions.json")
                .unwrap_or(false)
            {
                continue;
            }

            if let Some(session) = parse_session(&path) {
                sessions.push(session);
            }
        }
    }

    sessions
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let file = File::open(path).map_err(|error| format!("Failed to open session file: {error}"))?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(value) => value,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&line) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };

        if value.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }

        let message = match value.get("message") {
            Some(message) => message,
            None => continue,
        };

        let raw_role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let role = match raw_role {
            "toolResult" => "tool".to_string(),
            other => other.to_string(),
        };

        let content = message.get("content").map(extract_text).unwrap_or_default();
        if content.trim().is_empty() {
            continue;
        }

        let ts = value.get("timestamp").and_then(parse_timestamp_to_ms);
        messages.push(SessionMessage { role, content, ts });
    }

    Ok(messages)
}

fn parse_session(path: &Path) -> Option<SessionMeta> {
    let (head, tail) = read_head_tail_lines(path, 10, 30).ok()?;

    let mut session_id: Option<String> = None;
    let mut cwd: Option<String> = None;
    let mut created_at: Option<i64> = None;
    let mut summary: Option<String> = None;

    for line in &head {
        let value: Value = match serde_json::from_str(line) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };

        if created_at.is_none() {
            created_at = value.get("timestamp").and_then(parse_timestamp_to_ms);
        }

        let event_type = value.get("type").and_then(Value::as_str).unwrap_or("");
        if event_type == "session" {
            if session_id.is_none() {
                session_id = value
                    .get("id")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string());
            }
            if cwd.is_none() {
                cwd = value
                    .get("cwd")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string());
            }
            if let Some(timestamp) = value.get("timestamp").and_then(parse_timestamp_to_ms) {
                created_at.get_or_insert(timestamp);
            }
            continue;
        }

        if event_type == "message" && summary.is_none() {
            if let Some(message) = value.get("message") {
                let text = message.get("content").map(extract_text).unwrap_or_default();
                if !text.trim().is_empty() {
                    summary = Some(text);
                }
            }
        }
    }

    let mut last_active_at: Option<i64> = None;
    for line in tail.iter().rev() {
        let value: Value = match serde_json::from_str(line) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };
        if let Some(timestamp) = value.get("timestamp").and_then(parse_timestamp_to_ms) {
            last_active_at = Some(timestamp);
            break;
        }
    }

    let session_id = session_id.or_else(|| {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .map(|value| value.to_string())
    })?;

    let title = cwd
        .as_deref()
        .and_then(path_basename)
        .map(|value| value.to_string());

    Some(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id,
        title,
        summary: summary.map(|text| truncate_summary(&text, 160)),
        project_dir: cwd,
        created_at,
        last_active_at,
        source_path: path.to_string_lossy().to_string(),
        resume_command: None,
    })
}
