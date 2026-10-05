//! ZCode session discovery.
//!
//! ZCode keeps sessions in two independent stores under its data root:
//!
//! - `v2/sessions/<workspaceId>/<taskId>.json` — desktop transcripts. Each file
//!   holds `{ meta, messages }` with plain `{ role, content, timestamp }` rows.
//! - `cli/db/db.sqlite` — the CLI's own store. Its schema is the OpenCode v1
//!   layout (`session`/`message`/`part`), so the read/write helpers are reused
//!   from [`super::open_code`] rather than reimplemented. Note the CLI database
//!   has no `session_v2` table: ZCode is always on OpenCode's *v1* code path.
//!
//! ZCode also maintains `v2/tasks-index.sqlite`, whose `tasks` table carries the
//! display titles for the desktop store. It is not required for listing — the
//! JSON files carry their own `meta.title` — so it is deliberately unused.

use std::path::Path;

use serde_json::Value;

use super::{SessionMessage, SessionMeta};

/// Reads one desktop transcript file.
///
/// The desktop format stores plain strings for `content`, so each entry maps to
/// a single text message.
pub fn load_desktop_messages(source_path: &str) -> Result<Vec<SessionMessage>, String> {
    let text = std::fs::read_to_string(source_path)
        .map_err(|error| format!("Failed to read ZCode session file: {error}"))?;
    let parsed: Value = serde_json::from_str(&text)
        .map_err(|error| format!("Failed to parse ZCode session file: {error}"))?;

    let messages = parsed
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(messages
        .into_iter()
        .enumerate()
        .map(|(index, message)| {
            let role = message
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("assistant")
                .to_string();
            let content = message
                .get("content")
                .map(render_content)
                .unwrap_or_default();
            SessionMessage {
                role,
                content,
                ts: message.get("timestamp").and_then(Value::as_i64),
                id: Some(index.to_string()),
                parent_id: None,
                message_type: None,
                blocks: Vec::new(),
                model: None,
                usage: None,
                duration_ms: None,
                cost_usd: None,
                is_sidechain: None,
                metadata: None,
            }
        })
        .collect())
}

/// Flattens a desktop `content` value into display text.
///
/// Observed files store a bare string, but a block array is tolerated so a
/// future richer format degrades to readable text instead of nothing.
fn render_content(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| {
                block
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| block.as_str().map(str::to_string))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Scans the desktop store, newest first.
pub fn scan_desktop_sessions(desktop_sessions_root: &Path) -> Vec<SessionMeta> {
    let mut sessions = Vec::new();
    let Ok(workspaces) = std::fs::read_dir(desktop_sessions_root) else {
        return sessions;
    };

    for workspace in workspaces.flatten() {
        let workspace_path = workspace.path();
        if !workspace_path.is_dir() {
            continue;
        }
        let Ok(files) = std::fs::read_dir(&workspace_path) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            if let Some(session) = read_desktop_meta(&path) {
                sessions.push(session);
            }
        }
    }

    sessions.sort_by(|left, right| {
        let left_ts = left.last_active_at.or(left.created_at).unwrap_or(0);
        let right_ts = right.last_active_at.or(right.created_at).unwrap_or(0);
        right_ts.cmp(&left_ts)
    });
    sessions
}

/// Builds a [`SessionMeta`] from one desktop transcript.
///
/// Returns `None` for a file that is not a readable transcript, so one damaged
/// file cannot hide the rest of the list.
fn read_desktop_meta(path: &Path) -> Option<SessionMeta> {
    let text = std::fs::read_to_string(path).ok()?;
    let parsed: Value = serde_json::from_str(&text).ok()?;
    let meta = parsed.get("meta")?;

    let session_id = meta
        .get("taskId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            path.file_stem()
                .and_then(|value| value.to_str())
                .map(str::to_string)
        })?;

    let title = meta
        .get("title")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty());

    Some(SessionMeta {
        provider_id: "zcode".to_string(),
        session_id,
        title,
        summary: None,
        project_dir: meta
            .get("workspacePath")
            .and_then(Value::as_str)
            .map(str::to_string),
        created_at: meta.get("createdAt").and_then(Value::as_i64),
        last_active_at: meta.get("updatedAt").and_then(Value::as_i64),
        source_path: path.display().to_string(),
        resume_command: None,
        runtime_source: None,
        runtime_distro: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_transcript(
        root: &Path,
        workspace: &str,
        file_name: &str,
        meta: &str,
        messages: &str,
    ) {
        let dir = root.join(workspace);
        fs::create_dir_all(&dir).expect("create workspace dir");
        fs::write(
            dir.join(file_name),
            format!("{{\"meta\":{meta},\"messages\":{messages}}}"),
        )
        .expect("write transcript");
    }

    #[test]
    fn desktop_transcript_maps_meta_and_messages() {
        let dir = tempdir().expect("tempdir");
        write_transcript(
            dir.path(),
            "87e8f76dae83",
            "task-1.json",
            r#"{"taskId":"task-1","title":"Fix the parser","workspacePath":"D:\\repo","createdAt":1000,"updatedAt":2000}"#,
            r#"[{"role":"user","content":"hello","timestamp":1000},{"role":"assistant","content":"hi","timestamp":1100}]"#,
        );

        let sessions = scan_desktop_sessions(dir.path());
        assert_eq!(sessions.len(), 1);
        let session = &sessions[0];
        assert_eq!(session.session_id, "task-1");
        assert_eq!(session.title.as_deref(), Some("Fix the parser"));
        assert_eq!(session.project_dir.as_deref(), Some("D:\\repo"));
        assert_eq!(session.created_at, Some(1000));
        assert_eq!(session.last_active_at, Some(2000));
        assert_eq!(session.provider_id, "zcode");

        let messages = load_desktop_messages(&session.source_path).expect("load");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "hello");
        assert_eq!(messages[0].ts, Some(1000));
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "hi");
    }

    #[test]
    fn desktop_scan_sorts_newest_first_and_skips_damaged_files() {
        let dir = tempdir().expect("tempdir");
        write_transcript(
            dir.path(),
            "ws-a",
            "older.json",
            r#"{"taskId":"older","updatedAt":100}"#,
            "[]",
        );
        write_transcript(
            dir.path(),
            "ws-a",
            "newer.json",
            r#"{"taskId":"newer","updatedAt":900}"#,
            "[]",
        );
        fs::write(dir.path().join("ws-a").join("broken.json"), "{not json").expect("write broken");

        let sessions = scan_desktop_sessions(dir.path());
        let ids: Vec<&str> = sessions
            .iter()
            .map(|session| session.session_id.as_str())
            .collect();
        assert_eq!(ids, vec!["newer", "older"]);
    }

    #[test]
    fn desktop_title_falls_back_to_none_when_blank() {
        let dir = tempdir().expect("tempdir");
        write_transcript(
            dir.path(),
            "ws-a",
            "task.json",
            r#"{"taskId":"task","title":"   "}"#,
            "[]",
        );

        let sessions = scan_desktop_sessions(dir.path());
        assert_eq!(sessions[0].title, None);
    }

    #[test]
    fn desktop_session_id_falls_back_to_file_stem() {
        let dir = tempdir().expect("tempdir");
        write_transcript(dir.path(), "ws-a", "task-9.json", r#"{}"#, "[]");

        let sessions = scan_desktop_sessions(dir.path());
        assert_eq!(sessions[0].session_id, "task-9");
    }

    #[test]
    fn missing_desktop_root_yields_no_sessions() {
        let dir = tempdir().expect("tempdir");
        let missing = dir.path().join("does-not-exist");
        assert!(scan_desktop_sessions(&missing).is_empty());
    }

    #[test]
    fn block_array_content_is_flattened() {
        let dir = tempdir().expect("tempdir");
        write_transcript(
            dir.path(),
            "ws-a",
            "task.json",
            r#"{"taskId":"task"}"#,
            r#"[{"role":"assistant","content":[{"type":"text","text":"first"},{"type":"text","text":"second"}]}]"#,
        );

        let messages = load_desktop_messages(
            &dir.path().join("ws-a").join("task.json").display().to_string(),
        )
        .expect("load");
        assert_eq!(messages[0].content, "first\nsecond");
    }
}
