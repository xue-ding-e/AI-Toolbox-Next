use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::utils::{
    extract_prompt_title_text, extract_text, join_safe_relative, parse_timestamp_to_ms,
    path_basename, read_head_tail_lines, sanitize_path_segment, strip_path_prefix,
    text_contains_query, truncate_summary,
};
use super::{SessionMessage, SessionMeta};

const PROVIDER_ID: &str = "claudecode";

pub fn scan_sessions(root: &Path) -> Vec<SessionMeta> {
    let indexed_sessions = scan_sessions_from_index(root);
    let indexed_paths: std::collections::HashSet<String> = indexed_sessions
        .iter()
        .map(|session| session.source_path.clone())
        .collect();
    let mut jsonl_files = Vec::new();
    collect_jsonl_files(root, &mut jsonl_files);

    let mut sessions = indexed_sessions;
    for path in jsonl_files {
        let resolved_path = path.to_string_lossy().to_string();
        if indexed_paths.contains(&resolved_path) {
            continue;
        }
        if let Some(session) = parse_session(&path) {
            sessions.push(session);
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

        if value.get("isMeta").and_then(Value::as_bool) == Some(true) {
            continue;
        }

        let message = match value.get("message") {
            Some(message) => message,
            None => continue,
        };

        let mut role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();

        if role == "user" {
            if let Some(Value::Array(items)) = message.get("content") {
                let all_tool_results = !items.is_empty()
                    && items.iter().all(|item| {
                        item.get("type").and_then(Value::as_str) == Some("tool_result")
                    });
                if all_tool_results {
                    role = "tool".to_string();
                }
            }
        }

        let content = message.get("content").map(extract_text).unwrap_or_default();
        if content.trim().is_empty() {
            continue;
        }

        let ts = value.get("timestamp").and_then(parse_timestamp_to_ms);
        messages.push(SessionMessage { role, content, ts });
    }

    Ok(messages)
}

pub fn scan_messages_for_query(path: &Path, query_lower: &str) -> Result<bool, String> {
    let file = File::open(path).map_err(|error| format!("Failed to open session file: {error}"))?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = match line {
            Ok(value) => value,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&line) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };

        if value.get("isMeta").and_then(Value::as_bool) == Some(true) {
            continue;
        }

        let Some(message) = value.get("message") else {
            continue;
        };
        let content = message.get("content").map(extract_text).unwrap_or_default();
        if text_contains_query(&content, query_lower) {
            return Ok(true);
        }
    }

    Ok(false)
}

pub fn delete_session(path: &Path) -> Result<(), String> {
    let session_id = infer_session_id_from_filename(path).ok_or_else(|| {
        format!(
            "Failed to infer Claude Code session id from {}",
            path.display()
        )
    })?;
    let project_dir = path.parent().ok_or_else(|| {
        format!(
            "Failed to determine Claude Code project directory for {}",
            path.display()
        )
    })?;

    std::fs::remove_file(path)
        .map_err(|error| format!("Failed to delete session file {}: {error}", path.display()))?;
    remove_session_from_index(project_dir, &session_id)
}

pub fn export_native_snapshot(root: &Path, session_path: &Path) -> Result<Value, String> {
    let session = parse_session(session_path).ok_or_else(|| {
        format!(
            "Failed to parse Claude Code session {}",
            session_path.display()
        )
    })?;
    let relative_session_path = strip_path_prefix(root, session_path).ok_or_else(|| {
        format!(
            "Session path {} is outside Claude Code projects root {}",
            session_path.display(),
            root.display()
        )
    })?;
    let session_file_content = std::fs::read_to_string(session_path).map_err(|error| {
        format!(
            "Failed to read Claude Code session file {}: {error}",
            session_path.display()
        )
    })?;
    let project_relative_dir = session_path
        .parent()
        .and_then(|project_dir| strip_path_prefix(root, project_dir))
        .ok_or_else(|| {
            format!(
                "Failed to determine Claude Code project directory for {}",
                session_path.display()
            )
        })?;

    Ok(serde_json::json!({
        "projectRelativeDir": project_relative_dir,
        "sessionFileName": session_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default(),
        "relativeSessionPath": relative_session_path,
        "sessionFileContent": session_file_content,
        "indexEntry": {
            "sessionId": session.session_id,
            "projectPath": session.project_dir,
            "summary": session.summary,
            "created": session.created_at,
            "modified": session.last_active_at,
            "firstPrompt": session.title,
        }
    }))
}

pub fn import_native_snapshot(
    root: &Path,
    session_id: &str,
    snapshot: &Value,
) -> Result<PathBuf, String> {
    let project_relative_dir = snapshot
        .get("projectRelativeDir")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Claude Code snapshot missing projectRelativeDir".to_string())?;
    let session_file_name = snapshot
        .get("sessionFileName")
        .and_then(Value::as_str)
        .filter(|value| value.ends_with(".jsonl"))
        .map(|value| value.to_string())
        .unwrap_or_else(|| format!("{}.jsonl", sanitize_path_segment(session_id, "session")));
    let session_file_content = snapshot
        .get("sessionFileContent")
        .and_then(Value::as_str)
        .ok_or_else(|| "Claude Code snapshot missing sessionFileContent".to_string())?;
    let index_entry = snapshot
        .get("indexEntry")
        .cloned()
        .ok_or_else(|| "Claude Code snapshot missing indexEntry".to_string())?;

    let project_dir = join_safe_relative(root, project_relative_dir)?;
    std::fs::create_dir_all(&project_dir).map_err(|error| {
        format!(
            "Failed to create Claude Code project directory {}: {error}",
            project_dir.display()
        )
    })?;

    let target_path = project_dir.join(&session_file_name);
    if target_path.exists() {
        return Err(format!(
            "Claude Code session file already exists: {}",
            target_path.display()
        ));
    }

    std::fs::write(&target_path, session_file_content).map_err(|error| {
        format!(
            "Failed to write Claude Code session file {}: {error}",
            target_path.display()
        )
    })?;

    upsert_sessions_index(&project_dir, &target_path, &index_entry)?;
    Ok(target_path)
}

fn scan_sessions_from_index(root: &Path) -> Vec<SessionMeta> {
    if !root.exists() {
        return Vec::new();
    }

    let mut sessions = Vec::new();
    let project_entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    for project_entry in project_entries.flatten() {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }

        let index_path = project_path.join("sessions-index.json");
        let data = match std::fs::read_to_string(&index_path) {
            Ok(data) => data,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&data) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(entries) = value.get("entries").and_then(Value::as_array) else {
            continue;
        };

        for entry in entries {
            let Some(session) = parse_index_entry(entry, &project_path) else {
                continue;
            };
            sessions.push(session);
        }
    }

    sessions
}

fn parse_index_entry(entry: &Value, project_path: &Path) -> Option<SessionMeta> {
    let session_id = entry.get("sessionId").and_then(Value::as_str)?.to_string();
    let full_path = entry
        .get("fullPath")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .unwrap_or_else(|| project_path.join(format!("{session_id}.jsonl")));
    if !full_path.exists() || is_agent_session(&full_path) {
        return None;
    }

    let project_dir = entry
        .get("projectPath")
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .or_else(|| Some(project_path.to_string_lossy().to_string()));
    let created_at = entry.get("created").and_then(parse_timestamp_to_ms);
    let last_active_at = entry
        .get("modified")
        .or_else(|| entry.get("fileMtime"))
        .and_then(parse_timestamp_to_ms)
        .or(created_at);
    let summary = entry
        .get("summary")
        .or_else(|| entry.get("firstPrompt"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(|value| truncate_summary(value, 160));
    let title = summary.clone().or_else(|| {
        project_dir
            .as_deref()
            .and_then(path_basename)
            .map(|value| value.to_string())
    });

    Some(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id: session_id.clone(),
        title,
        summary,
        project_dir,
        created_at,
        last_active_at,
        source_path: full_path.to_string_lossy().to_string(),
        resume_command: Some(format!("claude --resume {session_id}")),
    })
}

fn parse_session(path: &Path) -> Option<SessionMeta> {
    if is_agent_session(path) {
        return None;
    }

    let (head, tail) = read_head_tail_lines(path, 20, 30).ok()?;

    let mut session_id: Option<String> = None;
    let mut project_dir: Option<String> = None;
    let mut created_at: Option<i64> = None;
    let mut slug: Option<String> = None;
    let mut title: Option<String> = None;

    for line in &head {
        let value: Value = match serde_json::from_str(line) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };

        if session_id.is_none() {
            session_id = value
                .get("sessionId")
                .and_then(Value::as_str)
                .map(|value| value.to_string());
        }
        if project_dir.is_none() {
            project_dir = value
                .get("cwd")
                .and_then(Value::as_str)
                .map(|value| value.to_string());
        }
        if slug.is_none() {
            slug = value
                .get("slug")
                .and_then(Value::as_str)
                .and_then(format_slug_title);
        }
        if title.is_none() {
            title = extract_user_prompt_title(&value);
        }
        if created_at.is_none() {
            created_at = value.get("timestamp").and_then(parse_timestamp_to_ms);
        }
    }

    let mut last_active_at: Option<i64> = None;
    let mut summary: Option<String> = None;

    for line in tail.iter().rev() {
        let value: Value = match serde_json::from_str(line) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };

        if last_active_at.is_none() {
            last_active_at = value.get("timestamp").and_then(parse_timestamp_to_ms);
        }
        if summary.is_none() {
            if value.get("isMeta").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            if let Some(message) = value.get("message") {
                let text = message.get("content").map(extract_text).unwrap_or_default();
                if !text.trim().is_empty() {
                    summary = Some(text);
                }
            }
        }

        if last_active_at.is_some() && summary.is_some() {
            break;
        }
    }

    let session_id = session_id.or_else(|| infer_session_id_from_filename(path))?;
    let title = title.or(slug).or_else(|| {
        project_dir
            .as_deref()
            .and_then(path_basename)
            .map(|value| value.to_string())
    });

    Some(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id: session_id.clone(),
        title,
        summary: summary.map(|text| truncate_summary(&text, 160)),
        project_dir,
        created_at,
        last_active_at,
        source_path: path.to_string_lossy().to_string(),
        resume_command: Some(format!("claude --resume {session_id}")),
    })
}

fn is_agent_session(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with("agent-"))
        .unwrap_or(false)
}

fn infer_session_id_from_filename(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.to_string())
}

fn extract_user_prompt_title(value: &Value) -> Option<String> {
    if value.get("type").and_then(Value::as_str) != Some("user") {
        return None;
    }

    let message = value.get("message")?;
    if message.get("role").and_then(Value::as_str) != Some("user") {
        return None;
    }
    if is_tool_result_only_content(message.get("content")) {
        return None;
    }

    let text = message.get("content").map(extract_text).unwrap_or_default();
    normalize_title_text(&text)
}

fn is_tool_result_only_content(content: Option<&Value>) -> bool {
    let Some(Value::Array(items)) = content else {
        return false;
    };

    !items.is_empty()
        && items
            .iter()
            .all(|item| item.get("type").and_then(Value::as_str) == Some("tool_result"))
}

fn normalize_title_text(text: &str) -> Option<String> {
    extract_prompt_title_text(text, 80)
}

fn format_slug_title(slug: &str) -> Option<String> {
    let normalized = slug
        .split('-')
        .filter(|segment| !segment.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if normalized.is_empty() {
        return None;
    }

    Some(normalized)
}

fn upsert_sessions_index(
    project_dir: &Path,
    session_path: &Path,
    index_entry: &Value,
) -> Result<(), String> {
    let index_path = project_dir.join("sessions-index.json");
    let mut root_value = if index_path.exists() {
        let data = std::fs::read_to_string(&index_path).map_err(|error| {
            format!(
                "Failed to read Claude Code sessions index {}: {error}",
                index_path.display()
            )
        })?;
        serde_json::from_str::<Value>(&data).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if !root_value.is_object() {
        root_value = serde_json::json!({});
    }

    let entries = root_value
        .as_object_mut()
        .and_then(|map| {
            map.entry("entries")
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
        })
        .ok_or_else(|| {
            format!(
                "Invalid Claude Code sessions index structure: {}",
                index_path.display()
            )
        })?;

    let mut next_entry = index_entry.clone();
    let next_entry_map = next_entry
        .as_object_mut()
        .ok_or_else(|| "Claude Code snapshot indexEntry must be an object".to_string())?;
    next_entry_map.insert(
        "fullPath".to_string(),
        Value::String(session_path.to_string_lossy().to_string()),
    );

    let session_id = next_entry_map
        .get("sessionId")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Claude Code snapshot indexEntry missing sessionId".to_string())?
        .to_string();

    if let Some(existing_entry) = entries.iter_mut().find(|entry| {
        entry
            .get("sessionId")
            .and_then(Value::as_str)
            .map(|value| value == session_id)
            .unwrap_or(false)
    }) {
        *existing_entry = next_entry;
    } else {
        entries.push(next_entry);
    }

    let serialized = serde_json::to_string_pretty(&root_value).map_err(|error| {
        format!(
            "Failed to serialize Claude Code sessions index {}: {error}",
            index_path.display()
        )
    })?;
    std::fs::write(&index_path, serialized).map_err(|error| {
        format!(
            "Failed to write Claude Code sessions index {}: {error}",
            index_path.display()
        )
    })?;

    Ok(())
}

fn remove_session_from_index(project_dir: &Path, session_id: &str) -> Result<(), String> {
    let index_path = project_dir.join("sessions-index.json");
    if !index_path.exists() {
        return Ok(());
    }

    let data = std::fs::read_to_string(&index_path).map_err(|error| {
        format!(
            "Failed to read Claude Code sessions index {}: {error}",
            index_path.display()
        )
    })?;
    let mut root_value =
        serde_json::from_str::<Value>(&data).unwrap_or_else(|_| serde_json::json!({}));
    let Some(entries) = root_value
        .as_object_mut()
        .and_then(|map| map.get_mut("entries"))
        .and_then(Value::as_array_mut)
    else {
        return Ok(());
    };

    entries.retain(|entry| {
        entry
            .get("sessionId")
            .and_then(Value::as_str)
            .map(|value| value != session_id)
            .unwrap_or(true)
    });

    let serialized = serde_json::to_string_pretty(&root_value).map_err(|error| {
        format!(
            "Failed to serialize Claude Code sessions index {}: {error}",
            index_path.display()
        )
    })?;
    std::fs::write(&index_path, serialized).map_err(|error| {
        format!(
            "Failed to write Claude Code sessions index {}: {error}",
            index_path.display()
        )
    })?;

    Ok(())
}

fn collect_jsonl_files(root: &Path, files: &mut Vec<PathBuf>) {
    if !root.exists() {
        return;
    }

    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_files(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
            files.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::delete_session;

    use std::fs;
    use std::path::{Path, PathBuf};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "ai-toolbox-claude-session-{label}-{}",
                uuid::Uuid::new_v4().simple()
            ));
            fs::create_dir_all(&path).expect("failed to create test directory");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn delete_session_removes_sessions_index_entry() {
        let test_dir = TestDir::new("delete-index-entry");
        let project_dir = test_dir.path().join("project");
        fs::create_dir_all(&project_dir).expect("failed to create project dir");

        let session_path = project_dir.join("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.jsonl");
        fs::write(
            &session_path,
            "{\"sessionId\":\"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee\",\"cwd\":\"/tmp/project\",\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"hello\"},\"timestamp\":\"2026-03-31T10:00:00Z\"}\n",
        )
        .expect("failed to write session file");

        let index_path = project_dir.join("sessions-index.json");
        fs::write(
            &index_path,
            format!(
                "{{\"entries\":[{{\"sessionId\":\"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee\",\"fullPath\":\"{}\"}}]}}",
                session_path.to_string_lossy()
            ),
        )
        .expect("failed to write sessions index");

        delete_session(&session_path).expect("delete_session should succeed");

        let index_content =
            fs::read_to_string(&index_path).expect("failed to read sessions index after delete");
        assert!(
            !index_content.contains("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"),
            "sessions-index should remove deleted session entry"
        );
    }
}
