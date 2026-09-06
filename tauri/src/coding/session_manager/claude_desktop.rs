//! Claude Desktop 3P session scanning.
//!
//! Claude Desktop (3P / cowork local-agent mode) persists sessions under
//! `<Claude-3p>/local-agent-mode-sessions/<project-hex>/<spaceId>/`:
//! - `local_<uuid>.json` — per-session metadata (`title`, `cwd`,
//!   `model`, `cliSessionId`, `createdAt`/`lastActivityAt` as epoch ms, ...).
//! - `local_<uuid>/.claude/projects/<cwd-encoded>/*.jsonl` — the transcript in
//!   the standard Claude Code transcript format, named after `cliSessionId`.
//!
//! This module reads the metadata to build `SessionMeta` (richer than deriving
//! from the transcript) and reuses `claude_code::load_messages` for detail, so
//! the Claude Desktop sessions surface in the browser session manager.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::claude_code;
use super::utils::parse_timestamp_to_ms;
use super::{SessionMessage, SessionMeta};

const PROVIDER_ID: &str = "claudedesktop";
const OUTPUTS_SUFFIX: &str = "outputs";

/// Scan all Claude Desktop 3P sessions under `sessions_root`
/// (e.g. `<Claude-3p>/local-agent-mode-sessions`), newest first.
pub fn scan_sessions(sessions_root: &Path) -> Vec<SessionMeta> {
    let mut meta_jsons = Vec::new();
    collect_meta_jsons(sessions_root, &mut meta_jsons);

    let mut sessions = Vec::new();
    for path in meta_jsons {
        if let Some(session) = session_meta_from_json(&path) {
            sessions.push(session);
        }
    }
    sessions.sort_by(|left, right| {
        right
            .last_active_at
            .unwrap_or(0)
            .cmp(&left.last_active_at.unwrap_or(0))
            .then_with(|| {
                right
                    .created_at
                    .unwrap_or(0)
                    .cmp(&left.created_at.unwrap_or(0))
            })
    });
    sessions
}

/// Scan recent Claude Desktop sessions, capped at `limit` (newest first).
pub fn scan_recent_sessions(sessions_root: &Path, limit: usize) -> Vec<SessionMeta> {
    if limit == 0 {
        return Vec::new();
    }
    scan_sessions(sessions_root)
        .into_iter()
        .take(limit)
        .collect()
}

/// Load the transcript for a Claude Desktop session. The `.jsonl` transcript
/// uses the Claude Code format, so the reader is reused unchanged.
pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    claude_code::load_messages(path)
}

fn collect_meta_jsons(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_meta_jsons(&path, out);
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("local_") && name.ends_with(".json"))
        {
            out.push(path);
        }
    }
}

fn session_meta_from_json(meta_path: &Path) -> Option<SessionMeta> {
    let content = fs::read_to_string(meta_path).ok()?;
    let value: Value = serde_json::from_str(&content).ok()?;

    let cli_session_id = value.get("cliSessionId").and_then(Value::as_str);
    let local_session_id = value
        .get("sessionId")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let fallback_id = meta_path
        .file_stem()
        .and_then(|name| name.to_str())
        .map(str::to_string);
    let session_id = cli_session_id
        .or(local_session_id)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or(fallback_id)?;

    let transcript_path = resolve_transcript(meta_path, cli_session_id)?;

    let title = value
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let initial_message = value
        .get("initialMessage")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let project_dir = value
        .get("resolvedFolderKinds")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("display"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            value
                .get("userSelectedFolders")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .or_else(|| claude_desktop_cwd(value.get("cwd")));

    Some(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id,
        summary: initial_message.clone(),
        title: title.or(initial_message),
        project_dir,
        created_at: value.get("createdAt").and_then(parse_timestamp_to_ms),
        last_active_at: value.get("lastActivityAt").and_then(parse_timestamp_to_ms),
        source_path: transcript_path.to_string_lossy().to_string(),
        resume_command: None,
        runtime_source: None,
        runtime_distro: None,
    })
}

/// The session's working directory derived from its metadata `cwd`, which
/// points at `<session dir>/outputs`; strip that suffix.
fn claude_desktop_cwd(cwd: Option<&Value>) -> Option<String> {
    let cwd = cwd
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let path = Path::new(cwd);
    let is_outputs_dir = path.file_name().and_then(|name| name.to_str()) == Some(OUTPUTS_SUFFIX);
    let dir = if is_outputs_dir {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    Some(dir.to_string_lossy().to_string())
}

/// Resolve the transcript `.jsonl` for a session: prefer the file named
/// `<cliSessionId>.jsonl` under `local_<uuid>/.claude/projects/**/`; fall back
/// to the sole jsonl if unambiguous; otherwise skip.
fn resolve_transcript(meta_path: &Path, cli_session_id: Option<&str>) -> Option<PathBuf> {
    let session_dir = meta_path.with_extension("").clone();
    let projects_root = session_dir.join(".claude").join("projects");
    let mut jsonls = Vec::new();
    collect_jsonls(&projects_root, &mut jsonls);
    if jsonls.is_empty() {
        return None;
    }
    if let Some(cli_session_id) = cli_session_id {
        let expected = format!("{cli_session_id}.jsonl");
        if let Some(path) = jsonls
            .iter()
            .find(|path| path.file_name().and_then(|name| name.to_str()) == Some(expected.as_str()))
        {
            return Some(path.clone());
        }
    }
    if jsonls.len() == 1 {
        return Some(jsonls.remove(0));
    }
    None
}

fn collect_jsonls(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonls(&path, out);
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext == "jsonl")
        {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn setup_session(root: &Path, uuid: &str, cli_session_id: &str) {
        let space = root.join("abc123").join("00000000");
        fs::create_dir_all(&space).expect("create space dir");
        let meta = space.join(format!("local_{uuid}.json"));
        fs::write(
            &meta,
            json!({
                "sessionId": format!("local_{uuid}"),
                "cliSessionId": cli_session_id,
                "title": "Build fixture",
                "initialMessage": "initial prompt",
                "model": "claude-opus-5",
                "cwd": space.join(format!("local_{uuid}")).join("outputs").to_string_lossy(),
                "resolvedFolderKinds": [{"display": "D:\\Projects\\app", "kind": "local"}],
                "createdAt": 1786723202667_i64,
                "lastActivityAt": 1786756920221_i64,
            })
            .to_string(),
        )
        .expect("write meta");
        let cwd_dir = space
            .join(format!("local_{uuid}"))
            .join(".claude")
            .join("projects")
            .join("encoded-cwd");
        fs::create_dir_all(&cwd_dir).expect("create transcript dir");
        // Main transcript named after the cli session id.
        fs::write(
            cwd_dir.join(format!("{cli_session_id}.jsonl")),
            "{\"sessionId\":\"s1\",\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"hi\"}]},\"timestamp\":\"2026-08-15T00:00:00Z\"}\n",
        )
        .expect("write transcript");
    }

    #[test]
    fn scan_sessions_reads_desktop_metadata_and_transcript() {
        let dir = tempfile::tempdir().expect("tempdir");
        setup_session(
            dir.path(),
            "local_a4a41914",
            "e711a4d4-28ae-4788-b1cf-aec1d6631e80",
        );

        let sessions = scan_sessions(dir.path());
        assert_eq!(sessions.len(), 1);

        let session = &sessions[0];
        assert_eq!(session.provider_id, "claudedesktop");
        assert_eq!(session.session_id, "e711a4d4-28ae-4788-b1cf-aec1d6631e80");
        assert_eq!(session.title.as_deref(), Some("Build fixture"));
        assert_eq!(session.project_dir.as_deref(), Some("D:\\Projects\\app"));
        assert_eq!(session.created_at, Some(1786723202667));
        assert_eq!(session.last_active_at, Some(1786756920221));
        assert!(session
            .source_path
            .ends_with("e711a4d4-28ae-4788-b1cf-aec1d6631e80.jsonl"));
    }

    #[test]
    fn scan_skips_session_without_readable_transcript() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Metadata only, no sibling transcript dir.
        let space = dir.path().join("proj").join("space");
        fs::create_dir_all(&space).expect("create space");
        fs::write(
            space.join("local_orphan.json"),
            json!({
                "cliSessionId": "cli-orphan",
                "title": "Orphan",
                "createdAt": 1786723202667_i64,
            })
            .to_string(),
        )
        .expect("write meta");

        assert!(scan_sessions(dir.path()).is_empty());
    }

    #[test]
    fn load_messages_uses_claude_code_reader() {
        let dir = tempfile::tempdir().expect("tempdir");
        setup_session(
            dir.path(),
            "local_a4a41914",
            "e711a4d4-28ae-4788-b1cf-aec1d6631e80",
        );
        let sessions = scan_sessions(dir.path());
        let source = Path::new(&sessions[0].source_path);
        let messages = load_messages(source).expect("load transcript");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert!(messages[0].content.contains("hi"));
    }
}
