use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use rusqlite::Connection;
use serde_json::Value;

use super::utils::{parse_timestamp_to_ms, path_basename, text_contains_query, truncate_summary};
use super::{SessionMessage, SessionMeta};

const PROVIDER_ID: &str = "opencode";

pub fn scan_sessions(data_root: &Path, sqlite_db_path: &Path) -> Vec<SessionMeta> {
    let json_sessions = scan_sessions_json(data_root);
    let sqlite_sessions = scan_sessions_sqlite(sqlite_db_path);

    if sqlite_sessions.is_empty() {
        return json_sessions;
    }
    if json_sessions.is_empty() {
        return sqlite_sessions;
    }

    let sqlite_ids: std::collections::HashSet<String> = sqlite_sessions
        .iter()
        .map(|session| session.session_id.clone())
        .collect();

    let mut merged = sqlite_sessions;
    for session in json_sessions {
        if !sqlite_ids.contains(&session.session_id) {
            merged.push(session);
        }
    }

    merged
}

pub fn load_messages(source_path: &str) -> Result<Vec<SessionMessage>, String> {
    if source_path.starts_with("sqlite:") {
        return load_messages_sqlite(source_path);
    }

    load_messages_json(Path::new(source_path))
}

pub fn scan_messages_for_query(source_path: &str, query_lower: &str) -> Result<bool, String> {
    if source_path.starts_with("sqlite:") {
        return scan_messages_for_query_sqlite(source_path, query_lower);
    }

    scan_messages_for_query_json(Path::new(source_path), query_lower)
}

pub fn delete_session(source_path: &str) -> Result<(), String> {
    if source_path.starts_with("sqlite:") {
        let (database_path, session_id) = parse_sqlite_source(source_path)
            .ok_or_else(|| format!("Invalid SQLite source reference: {source_path}"))?;
        delete_session_from_sqlite(&database_path, &session_id)?;
        delete_session_json_artifacts(
            &database_path.parent().unwrap_or(Path::new("")),
            &session_id,
        )?;
        return Ok(());
    }

    let message_dir = Path::new(source_path);
    let session_id = message_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            format!(
                "Invalid OpenCode message directory: {}",
                message_dir.display()
            )
        })?
        .to_string();
    let storage_root = message_dir
        .parent()
        .and_then(|parent| parent.parent())
        .ok_or_else(|| {
            format!(
                "Cannot determine storage root from {}",
                message_dir.display()
            )
        })?;
    let data_root = storage_root.parent().ok_or_else(|| {
        format!(
            "Cannot determine OpenCode data root from {}",
            storage_root.display()
        )
    })?;

    delete_session_from_sqlite(&data_root.join("opencode.db"), &session_id)?;
    delete_session_json_artifacts(data_root, &session_id)
}

pub fn rename_session(source_path: &str, next_title: &str) -> Result<(), String> {
    let normalized_title = next_title.trim();
    if normalized_title.is_empty() {
        return Err("Session title cannot be empty".to_string());
    }

    let (data_root, database_path, session_id) = if source_path.starts_with("sqlite:") {
        let (database_path, session_id) = parse_sqlite_source(source_path)
            .ok_or_else(|| format!("Invalid SQLite source reference: {source_path}"))?;
        let data_root = database_path.parent().ok_or_else(|| {
            format!(
                "Cannot determine OpenCode data root from {}",
                database_path.display()
            )
        })?;
        (data_root.to_path_buf(), database_path, session_id)
    } else {
        let message_dir = Path::new(source_path);
        let session_id = message_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                format!(
                    "Invalid OpenCode message directory: {}",
                    message_dir.display()
                )
            })?
            .to_string();
        let storage_root = message_dir
            .parent()
            .and_then(|parent| parent.parent())
            .ok_or_else(|| {
                format!(
                    "Cannot determine storage root from {}",
                    message_dir.display()
                )
            })?;
        let data_root = storage_root.parent().ok_or_else(|| {
            format!(
                "Cannot determine OpenCode data root from {}",
                storage_root.display()
            )
        })?;
        (
            data_root.to_path_buf(),
            data_root.join("opencode.db"),
            session_id,
        )
    };

    update_session_title_in_sqlite(&database_path, &session_id, normalized_title)?;
    update_session_title_in_json(&data_root, &session_id, normalized_title)?;
    Ok(())
}

pub fn export_native_snapshot(
    source_path: &str,
    config_path: Option<&Path>,
    data_root: Option<&Path>,
) -> Result<Value, String> {
    let session_id = extract_session_id_from_source(source_path)?;
    let mut command = Command::new("opencode");
    command.arg("export").arg(&session_id);
    configure_opencode_command_env(&mut command, config_path, data_root);
    let output = command
        .output()
        .map_err(|error| format!("Failed to run `opencode export`: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let details = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            stdout.trim().to_string()
        };
        return Err(format!(
            "`opencode export {session_id}` failed with status {}: {}",
            output.status, details
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("OpenCode export output is not valid UTF-8: {error}"))?;
    let exported_json = serde_json::from_str::<Value>(&stdout).map_err(|error| {
        format!("Failed to parse OpenCode export JSON for session {session_id}: {error}")
    })?;

    Ok(serde_json::json!({
        "sessionId": session_id,
        "officialExport": exported_json,
    }))
}

pub fn import_native_snapshot(
    snapshot: &Value,
    preferred_project_dir: Option<&str>,
    config_path: Option<&Path>,
    data_root: Option<&Path>,
) -> Result<(), String> {
    let official_export = snapshot
        .get("officialExport")
        .cloned()
        .ok_or_else(|| "OpenCode snapshot missing officialExport".to_string())?;
    let serialized = serde_json::to_string_pretty(&official_export)
        .map_err(|error| format!("Failed to serialize OpenCode official export: {error}"))?;

    let temp_path = std::env::temp_dir().join(format!(
        "ai-toolbox-opencode-import-{}.json",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::write(&temp_path, serialized).map_err(|error| {
        format!(
            "Failed to write temporary OpenCode import file {}: {error}",
            temp_path.display()
        )
    })?;

    let mut command = Command::new("opencode");
    command.arg("import").arg(&temp_path);
    configure_opencode_command_env(&mut command, config_path, data_root);

    if let Some(project_dir) = preferred_project_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let project_path = Path::new(project_dir);
        if project_path.exists() && project_path.is_dir() {
            command.current_dir(project_path);
        }
    }

    let output = command
        .output()
        .map_err(|error| format!("Failed to run `opencode import`: {error}"))?;
    let _ = std::fs::remove_file(&temp_path);

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let details = if !stderr.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        stdout.trim().to_string()
    };
    Err(format!(
        "`opencode import` failed with status {}: {}",
        output.status, details
    ))
}

fn scan_sessions_json(data_root: &Path) -> Vec<SessionMeta> {
    let storage_root = data_root.join("storage");
    let session_root = storage_root.join("session");
    if !session_root.exists() {
        return Vec::new();
    }

    let mut json_files = Vec::new();
    collect_json_files(&session_root, &mut json_files);

    json_files
        .into_iter()
        .filter_map(|path| parse_session(&storage_root, &path))
        .collect()
}

fn extract_session_id_from_source(source_path: &str) -> Result<String, String> {
    if source_path.starts_with("sqlite:") {
        let (_, session_id) = parse_sqlite_source(source_path)
            .ok_or_else(|| format!("Invalid SQLite source reference: {source_path}"))?;
        return Ok(session_id);
    }

    Path::new(source_path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|value| value.to_string())
        .ok_or_else(|| format!("Invalid OpenCode message directory: {source_path}"))
}

fn scan_sessions_sqlite(sqlite_db_path: &Path) -> Vec<SessionMeta> {
    if !sqlite_db_path.exists() {
        return Vec::new();
    }

    let connection = match Connection::open_with_flags(
        sqlite_db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(connection) => connection,
        Err(_) => return Vec::new(),
    };

    let mut statement = match connection.prepare(
        "SELECT id, title, directory, time_created, time_updated FROM session ORDER BY time_updated DESC",
    ) {
        Ok(statement) => statement,
        Err(_) => return Vec::new(),
    };

    let database_display = sqlite_db_path.display().to_string();
    let rows = match statement.query_map([], |row| {
        let session_id: String = row.get(0)?;
        let title: String = row.get(1)?;
        let directory: String = row.get(2)?;
        let created_at: i64 = row.get(3)?;
        let last_active_at: i64 = row.get(4)?;
        Ok((session_id, title, directory, created_at, last_active_at))
    }) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };

    let mut sessions = Vec::new();
    for row in rows.flatten() {
        let (session_id, title, directory, created_at, last_active_at) = row;
        let display_title = if title.is_empty() {
            path_basename(&directory)
        } else {
            Some(title)
        };

        sessions.push(SessionMeta {
            provider_id: PROVIDER_ID.to_string(),
            session_id: session_id.clone(),
            title: display_title.clone(),
            summary: display_title,
            project_dir: if directory.is_empty() {
                None
            } else {
                Some(directory)
            },
            created_at: Some(created_at),
            last_active_at: Some(last_active_at),
            source_path: format!("sqlite:{}:{}", database_display, session_id),
            resume_command: Some(format!("opencode session resume {session_id}")),
        });
    }

    sessions
}

fn parse_sqlite_source(source: &str) -> Option<(PathBuf, String)> {
    let rest = source.strip_prefix("sqlite:")?;
    let separator = rest.rfind(":ses_")?;
    let database_path = PathBuf::from(&rest[..separator]);
    let session_id = rest[separator + 1..].to_string();
    Some((database_path, session_id))
}

fn delete_session_from_sqlite(database_path: &Path, session_id: &str) -> Result<(), String> {
    if !database_path.exists() {
        return Ok(());
    }

    let mut connection = Connection::open(database_path)
        .map_err(|error| format!("Failed to open OpenCode database: {error}"))?;
    let transaction = connection.transaction().map_err(|error| {
        format!("Failed to start OpenCode session deletion transaction: {error}")
    })?;

    transaction
        .execute("DELETE FROM part WHERE session_id = ?1", [session_id])
        .map_err(|error| format!("Failed to delete OpenCode session parts: {error}"))?;
    transaction
        .execute("DELETE FROM message WHERE session_id = ?1", [session_id])
        .map_err(|error| format!("Failed to delete OpenCode session messages: {error}"))?;
    transaction
        .execute(
            "DELETE FROM session_share WHERE session_id = ?1",
            [session_id],
        )
        .map_err(|error| format!("Failed to delete OpenCode session shares: {error}"))?;
    transaction
        .execute("DELETE FROM session WHERE id = ?1", [session_id])
        .map_err(|error| format!("Failed to delete OpenCode session record: {error}"))?;

    transaction
        .commit()
        .map_err(|error| format!("Failed to commit OpenCode session deletion: {error}"))?;

    Ok(())
}

fn update_session_title_in_sqlite(
    database_path: &Path,
    session_id: &str,
    next_title: &str,
) -> Result<(), String> {
    if !database_path.exists() {
        return Ok(());
    }

    let connection = Connection::open(database_path)
        .map_err(|error| format!("Failed to open OpenCode database: {error}"))?;
    connection
        .execute(
            "UPDATE session SET title = ?1 WHERE id = ?2",
            [next_title, session_id],
        )
        .map_err(|error| format!("Failed to update OpenCode session title: {error}"))?;

    Ok(())
}

fn update_session_title_in_json(
    data_root: &Path,
    session_id: &str,
    next_title: &str,
) -> Result<(), String> {
    let storage_root = data_root.join("storage");
    let session_file = find_session_json_path(&storage_root, session_id);
    let Some(session_file) = session_file else {
        return Ok(());
    };

    let data = std::fs::read_to_string(&session_file).map_err(|error| {
        format!(
            "Failed to read OpenCode session file {}: {error}",
            session_file.display()
        )
    })?;
    let mut value: Value = serde_json::from_str(&data).map_err(|error| {
        format!(
            "Failed to parse OpenCode session file {}: {error}",
            session_file.display()
        )
    })?;
    let map = value
        .as_object_mut()
        .ok_or_else(|| format!("Invalid OpenCode session JSON: {}", session_file.display()))?;
    map.insert("title".to_string(), Value::String(next_title.to_string()));

    let serialized = serde_json::to_string_pretty(&value).map_err(|error| {
        format!(
            "Failed to serialize OpenCode session file {}: {error}",
            session_file.display()
        )
    })?;
    std::fs::write(&session_file, serialized).map_err(|error| {
        format!(
            "Failed to write OpenCode session file {}: {error}",
            session_file.display()
        )
    })?;

    Ok(())
}

fn delete_session_json_artifacts(data_root: &Path, session_id: &str) -> Result<(), String> {
    let storage_root = data_root.join("storage");
    let message_dir = storage_root.join("message").join(session_id);
    let session_file = find_session_json_path(&storage_root, session_id);

    let mut message_ids = Vec::new();
    if message_dir.is_dir() {
        let mut message_files = Vec::new();
        collect_json_files(&message_dir, &mut message_files);

        for message_path in &message_files {
            let data = match std::fs::read_to_string(message_path) {
                Ok(data) => data,
                Err(_) => continue,
            };
            let value: Value = match serde_json::from_str(&data) {
                Ok(value) => value,
                Err(_) => continue,
            };
            if let Some(message_id) = value.get("id").and_then(Value::as_str) {
                message_ids.push(message_id.to_string());
            }
        }
    }

    if let Some(session_file) = session_file {
        if session_file.exists() {
            std::fs::remove_file(&session_file).map_err(|error| {
                format!(
                    "Failed to delete OpenCode session file {}: {error}",
                    session_file.display()
                )
            })?;
        }
    }

    if message_dir.exists() {
        std::fs::remove_dir_all(&message_dir).map_err(|error| {
            format!(
                "Failed to delete OpenCode message directory {}: {error}",
                message_dir.display()
            )
        })?;
    }

    for message_id in message_ids {
        let part_dir = storage_root.join("part").join(&message_id);
        if part_dir.exists() {
            std::fs::remove_dir_all(&part_dir).map_err(|error| {
                format!(
                    "Failed to delete OpenCode part directory {}: {error}",
                    part_dir.display()
                )
            })?;
        }
    }

    Ok(())
}

fn load_messages_json(path: &Path) -> Result<Vec<SessionMessage>, String> {
    if !path.is_dir() {
        return Err(format!("Message directory not found: {}", path.display()));
    }

    let storage_root = path
        .parent()
        .and_then(|parent| parent.parent())
        .ok_or_else(|| "Cannot determine storage root from message path".to_string())?;

    let mut message_files = Vec::new();
    collect_json_files(path, &mut message_files);

    let mut entries: Vec<(i64, String, String, String)> = Vec::new();
    for message_path in &message_files {
        let data = match std::fs::read_to_string(message_path) {
            Ok(data) => data,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&data) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let message_id = match value.get("id").and_then(Value::as_str) {
            Some(id) => id.to_string(),
            None => continue,
        };

        let role = value
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let created_at = value
            .get("time")
            .and_then(|time| time.get("created"))
            .and_then(parse_timestamp_to_ms)
            .unwrap_or(0);

        let part_dir = storage_root.join("part").join(&message_id);
        let content = collect_parts_text(&part_dir);
        if content.trim().is_empty() {
            continue;
        }

        entries.push((created_at, message_id, role, content));
    }

    entries.sort_by_key(|(timestamp, _, _, _)| *timestamp);

    Ok(entries
        .into_iter()
        .map(|(timestamp, _, role, content)| SessionMessage {
            role,
            content,
            ts: if timestamp > 0 { Some(timestamp) } else { None },
        })
        .collect())
}

fn scan_messages_for_query_json(path: &Path, query_lower: &str) -> Result<bool, String> {
    if !path.is_dir() {
        return Err(format!("Message directory not found: {}", path.display()));
    }

    let storage_root = path
        .parent()
        .and_then(|parent| parent.parent())
        .ok_or_else(|| "Cannot determine storage root from message path".to_string())?;

    let mut message_files = Vec::new();
    collect_json_files(path, &mut message_files);

    for message_path in &message_files {
        let data = match std::fs::read_to_string(message_path) {
            Ok(data) => data,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&data) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let Some(message_id) = value.get("id").and_then(Value::as_str) else {
            continue;
        };

        let part_dir = storage_root.join("part").join(message_id);
        let content = collect_parts_text(&part_dir);
        if text_contains_query(&content, query_lower) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn load_messages_sqlite(source: &str) -> Result<Vec<SessionMessage>, String> {
    let (database_path, session_id) = parse_sqlite_source(source)
        .ok_or_else(|| format!("Invalid SQLite source reference: {source}"))?;

    let connection = Connection::open_with_flags(
        &database_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| format!("Failed to open OpenCode database: {error}"))?;

    let mut message_statement = connection
        .prepare("SELECT id, time_created, data FROM message WHERE session_id = ?1 ORDER BY time_created ASC")
        .map_err(|error| format!("Failed to prepare message query: {error}"))?;
    let message_rows = message_statement
        .query_map([session_id.as_str()], |row| {
            let message_id: String = row.get(0)?;
            let timestamp: i64 = row.get(1)?;
            let data: String = row.get(2)?;
            Ok((message_id, timestamp, data))
        })
        .map_err(|error| format!("Failed to query messages: {error}"))?;

    let mut part_statement = connection
        .prepare(
            "SELECT message_id, data FROM part WHERE session_id = ?1 ORDER BY time_created ASC",
        )
        .map_err(|error| format!("Failed to prepare part query: {error}"))?;
    let part_rows = part_statement
        .query_map([session_id.as_str()], |row| {
            let message_id: String = row.get(0)?;
            let data: String = row.get(1)?;
            Ok((message_id, data))
        })
        .map_err(|error| format!("Failed to query parts: {error}"))?;

    let mut parts_map: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for row in part_rows.flatten() {
        let (message_id, data) = row;
        parts_map.entry(message_id).or_default().push(data);
    }

    let mut messages = Vec::new();
    for row in message_rows.flatten() {
        let (message_id, timestamp, data) = row;
        let message_value: Value = match serde_json::from_str(&data) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let role = message_value
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();

        let mut texts = Vec::new();
        if let Some(parts) = parts_map.get(&message_id) {
            for part_data in parts {
                let part_value: Value = match serde_json::from_str(part_data) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                if let Some(text) = extract_part_text(&part_value) {
                    texts.push(text);
                }
            }
        }

        let content = texts.join("\n");
        if content.trim().is_empty() {
            continue;
        }

        messages.push(SessionMessage {
            role,
            content,
            ts: Some(timestamp),
        });
    }

    Ok(messages)
}

fn scan_messages_for_query_sqlite(source: &str, query_lower: &str) -> Result<bool, String> {
    let (database_path, session_id) = parse_sqlite_source(source)
        .ok_or_else(|| format!("Invalid SQLite source reference: {source}"))?;

    let connection = Connection::open_with_flags(
        &database_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| format!("Failed to open OpenCode database: {error}"))?;

    let mut message_statement = connection
        .prepare("SELECT id FROM message WHERE session_id = ?1 ORDER BY time_created ASC")
        .map_err(|error| format!("Failed to prepare message query: {error}"))?;
    let message_rows = message_statement
        .query_map([session_id.as_str()], |row| row.get::<_, String>(0))
        .map_err(|error| format!("Failed to query messages: {error}"))?;

    let mut part_statement = connection
        .prepare("SELECT data FROM part WHERE session_id = ?1 AND message_id = ?2 ORDER BY time_created ASC")
        .map_err(|error| format!("Failed to prepare part query: {error}"))?;

    for message_id in message_rows.flatten() {
        let part_rows = part_statement
            .query_map([session_id.as_str(), message_id.as_str()], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|error| format!("Failed to query parts: {error}"))?;

        let mut texts = Vec::new();
        for part_data in part_rows.flatten() {
            let part_value: Value = match serde_json::from_str(&part_data) {
                Ok(value) => value,
                Err(_) => continue,
            };
            if let Some(text) = extract_part_text(&part_value) {
                texts.push(text);
            }
        }

        if text_contains_query(&texts.join("\n"), query_lower) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn parse_session(storage_root: &Path, path: &Path) -> Option<SessionMeta> {
    let data = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&data).ok()?;

    let session_id = value.get("id").and_then(Value::as_str)?.to_string();
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let directory = value
        .get("directory")
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let created_at = value
        .get("time")
        .and_then(|time| time.get("created"))
        .and_then(parse_timestamp_to_ms);
    let last_active_at = value
        .get("time")
        .and_then(|time| time.get("updated"))
        .and_then(parse_timestamp_to_ms);

    let has_title = title.is_some();
    let display_title = title.or_else(|| {
        directory
            .as_deref()
            .and_then(path_basename)
            .map(|value| value.to_string())
    });

    let source_path = storage_root
        .join("message")
        .join(&session_id)
        .to_string_lossy()
        .to_string();
    let summary = if has_title {
        display_title.clone()
    } else {
        get_first_user_summary(storage_root, &session_id)
    };

    Some(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id: session_id.clone(),
        title: display_title,
        summary,
        project_dir: directory,
        created_at,
        last_active_at: last_active_at.or(created_at),
        source_path,
        resume_command: Some(format!("opencode session resume {session_id}")),
    })
}

fn get_first_user_summary(storage_root: &Path, session_id: &str) -> Option<String> {
    let message_dir = storage_root.join("message").join(session_id);
    if !message_dir.is_dir() {
        return None;
    }

    let mut message_files = Vec::new();
    collect_json_files(&message_dir, &mut message_files);

    let mut user_messages: Vec<(i64, String)> = Vec::new();
    for message_path in &message_files {
        let data = match std::fs::read_to_string(message_path) {
            Ok(data) => data,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&data) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if value.get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }

        let message_id = match value.get("id").and_then(Value::as_str) {
            Some(message_id) => message_id.to_string(),
            None => continue,
        };
        let timestamp = value
            .get("time")
            .and_then(|time| time.get("created"))
            .and_then(parse_timestamp_to_ms)
            .unwrap_or(0);

        user_messages.push((timestamp, message_id));
    }

    user_messages.sort_by_key(|(timestamp, _)| *timestamp);
    let (_, first_message_id) = user_messages.first()?;
    let part_dir = storage_root.join("part").join(first_message_id);
    let text = collect_parts_text(&part_dir);
    if text.trim().is_empty() {
        return None;
    }

    Some(truncate_summary(&text, 160))
}

fn extract_part_text(part_value: &Value) -> Option<String> {
    match part_value.get("type").and_then(Value::as_str) {
        Some("text") => part_value
            .get("text")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .map(|text| text.to_string()),
        Some("tool") => {
            let tool = part_value
                .get("tool")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            Some(format!("[Tool: {tool}]"))
        }
        _ => None,
    }
}

fn collect_parts_text(part_dir: &Path) -> String {
    if !part_dir.is_dir() {
        return String::new();
    }

    let mut part_files = Vec::new();
    collect_json_files(part_dir, &mut part_files);
    part_files.sort();

    let mut texts = Vec::new();
    for part_path in &part_files {
        let data = match std::fs::read_to_string(part_path) {
            Ok(data) => data,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&data) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if let Some(text) = extract_part_text(&value) {
            texts.push(text);
        }
    }

    texts.join("\n")
}

fn collect_json_files(root: &Path, files: &mut Vec<PathBuf>) {
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
            collect_json_files(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            files.push(path);
        }
    }
}

fn find_session_json_path(storage_root: &Path, session_id: &str) -> Option<PathBuf> {
    let session_root = storage_root.join("session");
    if !session_root.exists() {
        return None;
    }

    let mut session_files = Vec::new();
    collect_json_files(&session_root, &mut session_files);
    session_files.into_iter().find(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name == format!("{session_id}.json"))
            .unwrap_or(false)
    })
}

fn configure_opencode_command_env(
    command: &mut Command,
    config_path: Option<&Path>,
    data_root: Option<&Path>,
) {
    if let Some(config_path) = config_path {
        command.env("OPENCODE_CONFIG", config_path);

        let config_dir = config_path.parent();
        let config_root = config_dir.and_then(Path::parent);
        if let Some(config_root) = config_root {
            if config_dir.and_then(Path::file_name).and_then(OsStr::to_str) == Some("opencode") {
                command.env("XDG_CONFIG_HOME", config_root);
            }
        }
    }

    if let Some(data_root) = data_root {
        let data_dir = data_root.parent();
        if let Some(data_dir) = data_dir {
            if data_root.file_name().and_then(OsStr::to_str) == Some("opencode") {
                command.env("XDG_DATA_HOME", data_dir);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::delete_session_json_artifacts;

    use std::fs;
    use std::path::{Path, PathBuf};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "ai-toolbox-opencode-{label}-{}",
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
    fn delete_session_json_artifacts_removes_nested_session_json() {
        let test_dir = TestDir::new("delete-session-json");
        let data_root = test_dir.path().join("data");
        let storage_root = data_root.join("storage");
        let session_id = "ses_delete_nested";
        let message_id = "msg_delete_nested";

        let session_file = storage_root
            .join("session")
            .join("global")
            .join(format!("{session_id}.json"));
        let message_file = storage_root
            .join("message")
            .join(session_id)
            .join(format!("{message_id}.json"));
        let part_file = storage_root
            .join("part")
            .join(message_id)
            .join("prt_delete_nested.json");

        if let Some(parent) = session_file.parent() {
            fs::create_dir_all(parent).expect("failed to create session dir");
        }
        if let Some(parent) = message_file.parent() {
            fs::create_dir_all(parent).expect("failed to create message dir");
        }
        if let Some(parent) = part_file.parent() {
            fs::create_dir_all(parent).expect("failed to create part dir");
        }

        fs::write(
            &session_file,
            format!(r#"{{"id":"{session_id}","directory":"/tmp/project"}}"#),
        )
        .expect("failed to write session file");
        fs::write(
            &message_file,
            format!(r#"{{"id":"{message_id}","role":"user","time":{{"created":1}}}}"#),
        )
        .expect("failed to write message file");
        fs::write(&part_file, r#"{"type":"text","text":"hello"}"#)
            .expect("failed to write part file");

        delete_session_json_artifacts(&data_root, session_id)
            .expect("delete_session_json_artifacts should succeed");

        assert!(
            !session_file.exists(),
            "nested session json should be removed"
        );
        assert!(
            !message_file.exists(),
            "message json should be removed with session artifacts"
        );
        assert!(
            !part_file.exists(),
            "part json should be removed with session artifacts"
        );
    }
}
