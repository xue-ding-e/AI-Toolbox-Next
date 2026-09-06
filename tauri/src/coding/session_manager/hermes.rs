//! Hermes session scanning/loading.
//!
//! Hermes persists sessions in two places under its config root:
//!   - `<root>/state.db`        : SQLite database with a `sessions` table.
//!   - `<root>/sessions/*.jsonl`: JSONL transcript files.
//!
//! SQLite is the primary source; JSONL transcripts supplement sessions whose
//! IDs are missing from the database. All SQLite reads are read-only and every
//! I/O path degrades silently to an empty list when the data is unreachable.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde_json::Value;

use super::message_blocks::text_message;
use super::utils::{
    extract_text, parse_timestamp_to_ms, read_head_tail_lines, text_contains_query,
    truncate_summary,
};
use super::{assign_missing_message_ids, SessionMessage, SessionMeta};

const PROVIDER_ID: &str = "hermes";
const TITLE_MAX_CHARS: usize = 80;

/// Scan sessions from both the SQLite database and the JSONL transcript files,
/// with SQLite taking precedence on ID conflicts.
pub fn scan_sessions(root: &Path) -> Vec<SessionMeta> {
    let sqlite_sessions = scan_sessions_sqlite(root);
    let jsonl_sessions = scan_sessions_jsonl(root);

    if sqlite_sessions.is_empty() {
        return jsonl_sessions;
    }
    if jsonl_sessions.is_empty() {
        return sqlite_sessions;
    }

    let sqlite_ids: HashSet<String> = sqlite_sessions
        .iter()
        .map(|session| session.session_id.clone())
        .collect();

    let mut merged = sqlite_sessions;
    for session in jsonl_sessions {
        if !sqlite_ids.contains(&session.session_id) {
            merged.push(session);
        }
    }
    merged
}

/// Scan recent sessions, newest first. Reuses the full scan and truncates to
/// the requested limit so the quick first-screen path stays correct.
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

fn state_db_path(root: &Path) -> PathBuf {
    root.join("state.db")
}

fn sessions_dir(root: &Path) -> PathBuf {
    root.join("sessions")
}

// ── SQLite scanning ────────────────────────────────────────────────

fn scan_sessions_sqlite(root: &Path) -> Vec<SessionMeta> {
    let db_path = state_db_path(root);
    if !db_path.exists() {
        return Vec::new();
    }

    let conn = match Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(connection) => connection,
        Err(_) => return Vec::new(),
    };

    let has_sessions: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='sessions'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !has_sessions {
        return Vec::new();
    }

    let columns = get_table_columns(&conn, "sessions");

    let query = "SELECT * FROM sessions ORDER BY rowid DESC LIMIT 500";
    let mut stmt = match conn.prepare(query) {
        Ok(statement) => statement,
        Err(_) => return Vec::new(),
    };

    let rows = match stmt.query_map([], |row| Ok(row_to_json(row, &columns))) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };

    let db_source = format!("sqlite:{}", db_path.display());

    let mut sessions = Vec::new();
    for row in rows.flatten() {
        if let Some(meta) = sqlite_row_to_session_meta(&row, &db_source) {
            sessions.push(meta);
        }
    }

    sessions
}

fn sqlite_row_to_session_meta(row: &Value, db_source: &str) -> Option<SessionMeta> {
    let obj = row.as_object()?;

    let session_id = obj.get("id").and_then(Value::as_str)?.to_string();

    let title = obj
        .get("title")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(|value| truncate_summary(value, TITLE_MAX_CHARS).to_string());

    let cwd = obj
        .get("cwd")
        .or_else(|| obj.get("directory"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let started_at = obj
        .get("started_at")
        .or_else(|| obj.get("created_at"))
        .and_then(parse_timestamp_to_ms);

    let ended_at = obj
        .get("ended_at")
        .or_else(|| obj.get("updated_at"))
        .and_then(parse_timestamp_to_ms);

    let source_path = format!("{db_source}#{session_id}");

    Some(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id,
        title,
        summary: None,
        project_dir: cwd,
        created_at: started_at,
        last_active_at: ended_at.or(started_at),
        source_path,
        resume_command: None,
        runtime_source: None,
        runtime_distro: None,
    })
}

/// Get column names for a table, tolerating unknown/missing schemas.
fn get_table_columns(conn: &Connection, table: &str) -> Vec<String> {
    let query = format!("PRAGMA table_info({table})");
    let mut stmt = match conn.prepare(&query) {
        Ok(statement) => statement,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map([], |row| {
        let name: String = row.get(1)?;
        Ok(name)
    }) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };
    rows.flatten().collect()
}

/// Convert a SQLite row to a JSON Value using the known column names.
fn row_to_json(row: &rusqlite::Row, columns: &[String]) -> Value {
    let mut map = serde_json::Map::new();
    for (index, column) in columns.iter().enumerate() {
        if let Ok(value) = row.get::<_, String>(index) {
            map.insert(column.clone(), Value::String(value));
        } else if let Ok(value) = row.get::<_, i64>(index) {
            map.insert(column.clone(), Value::Number(value.into()));
        } else if let Ok(value) = row.get::<_, f64>(index) {
            if let Some(number) = serde_json::Number::from_f64(value) {
                map.insert(column.clone(), Value::Number(number));
            }
        } else {
            map.insert(column.clone(), Value::Null);
        }
    }
    Value::Object(map)
}

/// Load messages from the Hermes SQLite database for a `sqlite:` source ref.
fn load_messages_sqlite(source: &str) -> Result<Vec<SessionMessage>, String> {
    let (db_path, session_id) = parse_sqlite_source(source)
        .ok_or_else(|| format!("Invalid Hermes SQLite source reference: {source}"))?;

    let conn = Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| format!("Failed to open Hermes database: {error}"))?;

    // Hermes persists message timestamps in the `timestamp` column as a
    // second-precision floating-point value (e.g. 1786801520.21303). Read it as
    // f64 and let parse_timestamp_to_ms normalise to millisecond epoch.
    let query =
        "SELECT role, content, timestamp FROM messages WHERE session_id = ?1 ORDER BY timestamp ASC";
    let mut stmt = match conn.prepare(query) {
        Ok(statement) => statement,
        Err(_) => return Ok(Vec::new()),
    };
    let rows = match stmt.query_map([session_id.as_str()], |row| {
        let role: String = row.get(0)?;
        let content: String = row.get(1)?;
        let ts: Option<f64> = row.get(2).ok();
        Ok::<(String, String, Option<f64>), rusqlite::Error>((role, content, ts))
    }) {
        Ok(rows) => rows,
        Err(_) => return Ok(Vec::new()),
    };

    let mut messages = Vec::new();
    for row in rows.flatten() {
        let (role, content, ts) = row;
        if content.trim().is_empty() {
            continue;
        }
        let ts_ms = ts.and_then(|value| {
            serde_json::Number::from_f64(value)
                .map(Value::Number)
                .and_then(|number| parse_timestamp_to_ms(&number))
        });
        messages.push(text_message(role, content, ts_ms));
    }

    assign_missing_message_ids(&mut messages, PROVIDER_ID);
    Ok(messages)
}

/// Delete a session row (and its messages) from the Hermes SQLite database.
fn delete_session_sqlite(root: &Path, source: &str, session_id: &str) -> Result<bool, String> {
    let (db_path, ref_session_id) = parse_sqlite_source(source)
        .ok_or_else(|| format!("Invalid Hermes SQLite source reference: {source}"))?;

    if ref_session_id != session_id {
        return Err(format!(
            "Hermes SQLite session ID mismatch: expected {session_id}, found {ref_session_id}"
        ));
    }

    let expected_db = state_db_path(root).canonicalize().map_err(|error| {
        format!(
            "Failed to resolve Hermes database path {}: {error}",
            state_db_path(root).display()
        )
    })?;
    let canonical_db = db_path.canonicalize().map_err(|error| {
        format!(
            "Failed to resolve Hermes database path {}: {error}",
            db_path.display()
        )
    })?;
    if canonical_db != expected_db {
        return Err("SQLite path does not match the expected Hermes database".to_string());
    }

    let conn = Connection::open(&db_path)
        .map_err(|error| format!("Failed to open Hermes database: {error}"))?;

    let tx = conn
        .unchecked_transaction()
        .map_err(|error| format!("Failed to begin transaction: {error}"))?;

    let _ = tx.execute("DELETE FROM messages WHERE session_id = ?1", [session_id]);

    let deleted = tx
        .execute("DELETE FROM sessions WHERE id = ?1", [session_id])
        .map_err(|error| format!("Failed to delete Hermes session: {error}"))?;

    tx.commit()
        .map_err(|error| format!("Failed to commit session deletion: {error}"))?;

    Ok(deleted > 0)
}

fn parse_sqlite_source(source: &str) -> Option<(PathBuf, String)> {
    let rest = source.strip_prefix("sqlite:")?;
    let hash_pos = rest.rfind('#')?;
    let db_path = PathBuf::from(&rest[..hash_pos]);
    let session_id = rest[hash_pos + 1..].to_string();
    if session_id.is_empty() {
        return None;
    }
    Some((db_path, session_id))
}

// ── JSONL scanning ─────────────────────────────────────────────────

fn scan_sessions_jsonl(root: &Path) -> Vec<SessionMeta> {
    let sessions_dir = sessions_dir(root);
    if !sessions_dir.exists() {
        return Vec::new();
    }

    let entries = match std::fs::read_dir(&sessions_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut sessions = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|extension| extension.to_str());
        if ext != Some("jsonl") && ext != Some("json") {
            continue;
        }
        if let Some(meta) = parse_jsonl_session(&path) {
            sessions.push(meta);
        }
    }
    sessions
}

fn parse_jsonl_session(path: &Path) -> Option<SessionMeta> {
    // Read head (metadata + first user message) and tail (last timestamp).
    let (head, tail) = read_head_tail_lines(path, 30, 10).ok()?;

    let mut first_user_msg: Option<String> = None;
    let mut first_ts: Option<i64> = None;
    let mut last_ts: Option<i64> = None;
    let mut session_id: Option<String> = None;
    let mut title: Option<String> = None;
    let mut cwd: Option<String> = None;

    for line in &head {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let ts = value
            .get("timestamp")
            .or_else(|| value.get("ts"))
            .and_then(parse_timestamp_to_ms);

        if first_ts.is_none() {
            first_ts = ts;
        }
        last_ts = ts.or(last_ts);

        let line_type = value.get("type").and_then(Value::as_str).unwrap_or("");

        if line_type == "session" || line_type == "init" {
            if session_id.is_none() {
                session_id = value
                    .get("id")
                    .or_else(|| value.get("sessionId"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            if title.is_none() {
                title = value
                    .get("title")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(|value| truncate_summary(value, TITLE_MAX_CHARS).to_string());
            }
            if cwd.is_none() {
                cwd = value
                    .get("cwd")
                    .or_else(|| value.get("directory"))
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
            }
        }

        if first_user_msg.is_none() {
            let role = value
                .get("role")
                .or_else(|| value.get("message").and_then(|message| message.get("role")))
                .and_then(Value::as_str);

            if role == Some("user") {
                let content = value.get("content").or_else(|| {
                    value
                        .get("message")
                        .and_then(|message| message.get("content"))
                });
                if let Some(content) = content {
                    let text = extract_text(content);
                    if !text.trim().is_empty() {
                        first_user_msg = Some(truncate_summary(&text, TITLE_MAX_CHARS).to_string());
                    }
                }
            }
        }
    }

    for line in tail.iter().rev() {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let ts = value
            .get("timestamp")
            .or_else(|| value.get("ts"))
            .and_then(parse_timestamp_to_ms);
        if let Some(ts) = ts {
            last_ts = Some(ts);
            break;
        }
    }

    let session_id = session_id.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("unknown")
            .to_string()
    });

    let source_path = path.to_string_lossy().to_string();

    Some(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id,
        title: title.or_else(|| first_user_msg.clone()),
        summary: first_user_msg,
        project_dir: cwd,
        created_at: first_ts,
        last_active_at: last_ts.or(first_ts),
        source_path,
        resume_command: None,
        runtime_source: None,
        runtime_distro: None,
    })
}

/// Load messages from a Hermes JSONL transcript file.
fn load_messages_jsonl(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let file = File::open(path).map_err(|error| {
        format!(
            "Failed to open Hermes session file {}: {error}",
            path.display()
        )
    })?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(value) => value,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        // Support both flat messages and nested {type:"message", message:{...}}.
        let (role_val, content_val, ts_val) =
            if value.get("type").and_then(Value::as_str) == Some("message") {
                let message = match value.get("message") {
                    Some(message) => message,
                    None => continue,
                };
                (
                    message.get("role"),
                    message.get("content"),
                    value.get("timestamp").or_else(|| message.get("ts")),
                )
            } else {
                (
                    value.get("role"),
                    value.get("content"),
                    value.get("timestamp").or_else(|| value.get("ts")),
                )
            };

        let role = match role_val.and_then(Value::as_str) {
            Some(role) => role.to_string(),
            None => continue,
        };

        let content = content_val.map(extract_text).unwrap_or_default();
        if content.trim().is_empty() {
            continue;
        }

        let ts = ts_val.and_then(parse_timestamp_to_ms);
        messages.push(text_message(role, content, ts));
    }

    assign_missing_message_ids(&mut messages, PROVIDER_ID);
    Ok(messages)
}

// ── Public dispatch helpers used by session_manager::mod ───────────

/// Load messages for a session, dispatching on its `source_path` shape:
/// `sqlite:` references read from the database, everything else is a JSONL file.
pub fn load_messages(source: &str) -> Result<Vec<SessionMessage>, String> {
    if source.starts_with("sqlite:") {
        load_messages_sqlite(source)
    } else {
        load_messages_jsonl(Path::new(source))
    }
}

/// Delete a session by its `source_path`. SQLite references remove the DB row;
/// plain paths remove the JSONL transcript file.
pub fn delete_session(root: &Path, source: &str) -> Result<(), String> {
    if let Some((_, session_id)) = parse_sqlite_source(source) {
        delete_session_sqlite(root, source, &session_id)?;
        return Ok(());
    }

    std::fs::remove_file(source)
        .map_err(|error| format!("Failed to delete Hermes session file {source}: {error}"))
}

/// Test whether a session's message content contains the given query, without
/// materializing the whole message list first.
pub fn scan_messages_for_query(source: &str, query_lower: &str) -> Result<bool, String> {
    if source.starts_with("sqlite:") {
        let (db_path, session_id) = parse_sqlite_source(source)
            .ok_or_else(|| format!("Invalid Hermes SQLite source reference: {source}"))?;
        let conn = Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|error| format!("Failed to open Hermes database: {error}"))?;
        let query =
            "SELECT content FROM messages WHERE session_id = ?1 AND content LIKE ?2 LIMIT 1";
        let pattern = format!("%{query_lower}%");
        let mut stmt = match conn.prepare(query) {
            Ok(statement) => statement,
            Err(_) => return Ok(false),
        };
        let rows = match stmt.query_map([session_id.as_str(), pattern.as_str()], |row| {
            let content: String = row.get(0)?;
            Ok::<String, rusqlite::Error>(content)
        }) {
            Ok(rows) => rows,
            Err(_) => return Ok(false),
        };
        return Ok(rows
            .flatten()
            .any(|content| text_contains_query(&content, query_lower)));
    }

    let file = File::open(source)
        .map_err(|error| format!("Failed to open Hermes session file {source}: {error}"))?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = match line {
            Ok(value) => value,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let text = extract_text(&value);
        if text_contains_query(&text, query_lower) {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn parse_sqlite_source_valid() {
        let (path, id) =
            parse_sqlite_source("sqlite:/home/user/.hermes/state.db#session-123").expect("parse");
        assert_eq!(path, PathBuf::from("/home/user/.hermes/state.db"));
        assert_eq!(id, "session-123");
    }

    #[test]
    fn parse_sqlite_source_invalid() {
        assert!(parse_sqlite_source("not-sqlite").is_none());
        assert!(parse_sqlite_source("sqlite:").is_none());
        assert!(parse_sqlite_source("sqlite:/path#").is_none());
    }

    #[test]
    fn parse_jsonl_session_extracts_metadata() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("test-session.jsonl");
        let mut file = File::create(&path).expect("create");
        writeln!(
            file,
            r#"{{"type":"session","id":"s1","title":"My Session","cwd":"/home/user/project"}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"message","message":{{"role":"user","content":"Hello world"}},"timestamp":"2026-01-01T00:00:00Z"}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"message","message":{{"role":"assistant","content":"Hi there"}},"timestamp":"2026-01-01T00:01:00Z"}}"#
        )
        .unwrap();
        file.flush().unwrap();

        let meta = parse_jsonl_session(&path).expect("parse");
        assert_eq!(meta.session_id, "s1");
        assert_eq!(meta.title.as_deref(), Some("My Session"));
        assert_eq!(meta.project_dir.as_deref(), Some("/home/user/project"));
        assert!(meta.created_at.is_some());
        assert!(meta.last_active_at.is_some());
    }

    #[test]
    fn parse_jsonl_session_fallback_to_filename() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("my-session.jsonl");
        let mut file = File::create(&path).expect("create");
        writeln!(
            file,
            r#"{{"role":"user","content":"Hello","ts":1700000000}}"#
        )
        .unwrap();
        file.flush().unwrap();

        let meta = parse_jsonl_session(&path).expect("parse");
        assert_eq!(meta.session_id, "my-session");
        assert!(meta.title.is_some());
    }

    #[test]
    fn load_messages_flat_format() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("session.jsonl");
        let mut file = File::create(&path).expect("create");
        writeln!(
            file,
            r#"{{"role":"user","content":"What is Rust?","ts":1700000000}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"role":"assistant","content":"A systems programming language.","ts":1700000001}}"#
        )
        .unwrap();
        file.flush().unwrap();

        let messages = load_messages(&path.to_string_lossy()).expect("load");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
    }

    #[test]
    fn load_messages_nested_format() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("session.jsonl");
        let mut file = File::create(&path).expect("create");
        writeln!(file, r#"{{"type":"session","id":"s1"}}"#).unwrap();
        writeln!(
            file,
            r#"{{"type":"message","message":{{"role":"user","content":"Hello"}},"timestamp":"2026-01-01T00:00:00Z"}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"message","message":{{"role":"assistant","content":"Hi"}},"timestamp":"2026-01-01T00:01:00Z"}}"#
        )
        .unwrap();
        file.flush().unwrap();

        let messages = load_messages(&path.to_string_lossy()).expect("load");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert!(messages[0].ts.is_some());
        assert!(messages[0].id.is_some());
    }

    #[test]
    fn load_messages_sqlite_uses_hermes_timestamp_column() {
        // Mirrors the real Hermes 0.20.1 `messages` schema: the time column is
        // `timestamp` (REAL seconds), NOT `created_at`. Regression test for the
        // schema-drift bug where the SQL prepared against `created_at` failed
        // and the detail view silently came back empty.
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("state.db");
        let conn = Connection::open(&db_path).expect("open db");
        conn.execute_batch(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, title TEXT, started_at REAL);
             CREATE TABLE messages (
                id INTEGER PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT,
                timestamp REAL NOT NULL
             );",
        )
        .expect("create tables");
        conn.execute(
            "INSERT INTO sessions (id, title, started_at) VALUES (?1, ?2, ?3)",
            rusqlite::params!["s1", "Test", 1786801520.0f64],
        )
        .expect("insert session");
        conn.execute(
            "INSERT INTO messages (session_id, role, content, timestamp) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["s1", "user", "hello", 1786801520.21303f64],
        )
        .expect("insert message");
        conn.execute(
            "INSERT INTO messages (session_id, role, content, timestamp) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["s1", "assistant", "hi there", 1786801524.63216f64],
        )
        .expect("insert message");

        let source = format!("sqlite:{}#s1", db_path.display());
        let messages = load_messages(&source).expect("load");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        assert!(messages[0].ts.is_some());
        assert_eq!(messages[0].content, "hello");
    }
}
