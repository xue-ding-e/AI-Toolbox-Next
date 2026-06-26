use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::message_blocks::{
    message_from_blocks, text_block, thinking_block, tool_call_block, tool_result_block,
};
use super::utils::{
    build_resume_command, extract_prompt_title_text, extract_text,
    extract_wrapped_user_request_text, join_safe_relative, parse_timestamp_to_ms, path_basename,
    sanitize_path_segment, strip_path_prefix, text_contains_query, truncate_summary,
};
use super::{
    assign_missing_message_ids, SessionMessage, SessionMessageBlock, SessionMessageUsage,
    SessionMeta, SessionSubagentMeta,
};

const PROVIDER_ID: &str = "geminicli";
const SESSION_FILE_PREFIX: &str = "session-";

#[derive(Debug, Clone)]
struct GeminiConversation {
    session_id: String,
    start_time: Option<i64>,
    last_updated: Option<i64>,
    summary: Option<String>,
    kind: Option<String>,
    messages: Vec<Value>,
    first_user_message: Option<String>,
    has_user_or_assistant_message: bool,
}

fn load_project_registry(tmp_root: &Path) -> HashMap<String, String> {
    let Some(gemini_root) = tmp_root.parent() else {
        return HashMap::new();
    };
    let registry_path = gemini_root.join("projects.json");
    let Ok(content) = std::fs::read_to_string(registry_path) else {
        return HashMap::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&content) else {
        return HashMap::new();
    };
    let Some(projects) = value.get("projects").and_then(Value::as_object) else {
        return HashMap::new();
    };

    projects
        .iter()
        .filter_map(|(project_root, project_key)| {
            project_key
                .as_str()
                .map(|key| (key.to_string(), project_root.to_string()))
        })
        .collect()
}

pub fn scan_sessions(tmp_root: &Path) -> Vec<SessionMeta> {
    if !tmp_root.exists() {
        return Vec::new();
    }

    let project_entries = match std::fs::read_dir(tmp_root) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let mut sessions = Vec::new();
    let project_registry = load_project_registry(tmp_root);

    for project_entry in project_entries.flatten() {
        let project_dir = project_entry.path();
        if !project_dir.is_dir() {
            continue;
        }

        let chats_dir = project_dir.join("chats");
        if !chats_dir.is_dir() {
            continue;
        }

        let project_root = std::fs::read_to_string(project_dir.join(".project_root"))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| {
                project_dir
                    .file_name()
                    .and_then(|value| value.to_str())
                    .and_then(|project_key| project_registry.get(project_key).cloned())
            });
        let chat_entries = match std::fs::read_dir(&chats_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for chat_entry in chat_entries.flatten() {
            let path = chat_entry.path();
            if !is_supported_session_file(&path) {
                continue;
            }

            if let Some(mut meta) = parse_session(&path) {
                if meta.project_dir.is_none() {
                    meta.project_dir = project_root.clone();
                }
                meta.resume_command = Some(build_resume_command(
                    meta.project_dir.as_deref(),
                    &format!("gemini --resume {}", meta.session_id),
                ));
                sessions.push(meta);
            }
        }
    }

    sessions
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let conversation = read_conversation(path, true)?;
    let mut result = Vec::new();

    for message in &conversation.messages {
        let role = match message.get("type").and_then(Value::as_str) {
            Some("gemini") => "assistant",
            Some("user") => "user",
            Some("info") | Some("error") | Some("warning") => "system",
            Some(_) | None => continue,
        };

        let blocks = gemini_blocks_from_message(message, role);

        if blocks.is_empty() {
            continue;
        }

        let ts = message.get("timestamp").and_then(parse_timestamp_to_ms);
        let mut session_message = message_from_blocks(role, ts, blocks);
        session_message.id = message
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string);
        session_message.message_type = message
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_string);
        session_message.model = message
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string);
        session_message.usage = gemini_usage_from_tokens(message.get("tokens"));
        result.push(session_message);
    }

    assign_missing_message_ids(&mut result, PROVIDER_ID);
    Ok(result)
}

pub fn list_subagent_sessions(parent_session_path: &Path) -> Vec<SessionSubagentMeta> {
    let Some(chats_dir) = parent_session_path.parent() else {
        return Vec::new();
    };
    let Some(parent_session_id) = read_session_id_from_file(parent_session_path) else {
        return Vec::new();
    };
    let subagent_dir = chats_dir.join(sanitize_path_segment(&parent_session_id, "session"));
    let Ok(directory_metadata) = std::fs::symlink_metadata(&subagent_dir) else {
        return Vec::new();
    };
    if directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir() {
        return Vec::new();
    }

    let Ok(entries) = std::fs::read_dir(&subagent_dir) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if file_metadata.file_type().is_symlink() || !file_metadata.is_file() {
            continue;
        }
        if !matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("json") | Some("jsonl")
        ) {
            continue;
        }
        files.push(path);
    }
    files.sort();
    files.dedup();
    files
        .into_iter()
        .filter_map(|path| build_subagent_meta(&path))
        .collect()
}

pub fn scan_messages_for_query(path: &Path, query_lower: &str) -> Result<bool, String> {
    let messages = load_messages(path)?;
    Ok(messages
        .iter()
        .any(|message| text_contains_query(&message.content, query_lower)))
}

pub fn delete_session(path: &Path) -> Result<(), String> {
    let session = parse_session(path).ok_or_else(|| {
        format!(
            "Failed to parse Gemini CLI session metadata: {}",
            path.display()
        )
    })?;

    let chats_dir = path.parent().ok_or_else(|| {
        format!(
            "Failed to determine Gemini CLI chats directory for {}",
            path.display()
        )
    })?;
    let project_temp_dir = chats_dir.parent().ok_or_else(|| {
        format!(
            "Failed to determine Gemini CLI project temp directory for {}",
            path.display()
        )
    })?;
    let short_id = derive_short_session_id(&session.session_id)?;

    let mut cleanup_errors = Vec::new();
    let mut session_file_errors = Vec::new();

    for session_file in matching_session_files(chats_dir, path, &short_id) {
        let artifact_session_id =
            read_session_id_from_file(&session_file).unwrap_or_else(|| session.session_id.clone());
        if let Err(error) = delete_session_artifacts(project_temp_dir, &artifact_session_id) {
            cleanup_errors.push(error);
        }
        if let Err(error) = remove_file_if_exists(&session_file) {
            session_file_errors.push(error);
        }
    }

    if let Err(error) =
        delete_subagent_session_dir_and_artifacts(chats_dir, project_temp_dir, &session.session_id)
    {
        cleanup_errors.push(error);
    }
    log_cleanup_errors(cleanup_errors);

    if session_file_errors.is_empty() {
        Ok(())
    } else {
        Err(session_file_errors.join("; "))
    }
}

fn derive_short_session_id(session_id: &str) -> Result<String, String> {
    let short_id: String = session_id.chars().take(8).collect();
    if short_id.chars().count() == 8 {
        Ok(short_id)
    } else {
        Err(format!("Invalid Gemini CLI session id: {session_id}"))
    }
}

fn matching_session_files(chats_dir: &Path, original_path: &Path, short_id: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if original_path.is_file() {
        files.push(original_path.to_path_buf());
    }

    if let Ok(entries) = std::fs::read_dir(chats_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path == original_path
                || !path.is_file()
                || !is_matching_session_file(&path, short_id)
            {
                continue;
            }
            files.push(path);
        }
    }

    files.sort();
    files.dedup();
    files
}

fn is_matching_session_file(path: &Path, short_id: &str) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    file_name.starts_with(SESSION_FILE_PREFIX)
        && (file_name.ends_with(&format!("-{short_id}.json"))
            || file_name.ends_with(&format!("-{short_id}.jsonl")))
}

fn read_session_id_from_file(path: &Path) -> Option<String> {
    read_conversation(path, false)
        .ok()
        .map(|conversation| conversation.session_id)
}

fn build_subagent_meta(path: &Path) -> Option<SessionSubagentMeta> {
    let conversation = read_conversation(path, true).ok()?;
    let file_stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("subagent")
        .to_string();
    let title = conversation
        .summary
        .as_deref()
        .map(|value| truncate_summary(value, 80))
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            conversation
                .first_user_message
                .as_deref()
                .and_then(|text| extract_prompt_title_text(text, 80))
        })
        .unwrap_or_else(|| file_stem.clone());
    let message_count = conversation.messages.len();

    Some(SessionSubagentMeta {
        id: conversation.session_id,
        source_path: path.to_string_lossy().to_string(),
        title: title.clone(),
        summary: Some(truncate_summary(&title, 160)),
        subagent_type: conversation.kind,
        message_count,
        first_message_time: conversation.start_time,
        last_message_time: conversation.last_updated.or(conversation.start_time),
    })
}

fn delete_session_artifacts(project_temp_dir: &Path, session_id: &str) -> Result<(), String> {
    let safe_session_id = sanitize_path_segment(session_id, "session");
    remove_file_if_exists(
        &project_temp_dir
            .join("logs")
            .join(format!("session-{safe_session_id}.jsonl")),
    )?;
    remove_dir_if_exists(
        &project_temp_dir
            .join("tool-outputs")
            .join(format!("session-{safe_session_id}")),
    )?;
    remove_dir_if_exists(&project_temp_dir.join(safe_session_id))
}

fn delete_subagent_session_dir_and_artifacts(
    chats_dir: &Path,
    project_temp_dir: &Path,
    parent_session_id: &str,
) -> Result<(), String> {
    let safe_parent_session_id = sanitize_path_segment(parent_session_id, "session");
    let subagent_dir = chats_dir.join(safe_parent_session_id);
    if !subagent_dir.exists() {
        return Ok(());
    }

    let mut cleanup_errors = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&subagent_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(file_stem) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            let is_json_session = matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("json") | Some("jsonl")
            );
            if is_json_session {
                if let Err(error) = delete_session_artifacts(project_temp_dir, file_stem) {
                    cleanup_errors.push(error);
                }
            }
        }
    }

    if let Err(error) = remove_dir_if_exists(&subagent_dir) {
        cleanup_errors.push(error);
    }

    if cleanup_errors.is_empty() {
        Ok(())
    } else {
        Err(cleanup_errors.join("; "))
    }
}

fn log_cleanup_errors(errors: Vec<String>) {
    for error in errors {
        eprintln!("Gemini CLI session artifact cleanup warning: {error}");
    }
}

fn remove_file_if_exists(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "Failed to delete Gemini CLI session file {}: {error}",
            path.display()
        )),
    }
}

fn remove_dir_if_exists(path: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "Failed to delete Gemini CLI session directory {}: {error}",
            path.display()
        )),
    }
}

pub fn export_native_snapshot(tmp_root: &Path, session_path: &Path) -> Result<Value, String> {
    let session = parse_session(session_path).ok_or_else(|| {
        format!(
            "Failed to parse Gemini CLI session {}",
            session_path.display()
        )
    })?;
    let relative_session_path = strip_path_prefix(tmp_root, session_path).ok_or_else(|| {
        format!(
            "Session path {} is outside Gemini CLI tmp root {}",
            session_path.display(),
            tmp_root.display()
        )
    })?;
    let session_file_content = std::fs::read_to_string(session_path).map_err(|error| {
        format!(
            "Failed to read Gemini CLI session file {}: {error}",
            session_path.display()
        )
    })?;
    let session_file = if is_jsonl_path(session_path) {
        None
    } else {
        serde_json::from_str::<Value>(&session_file_content).ok()
    };
    let project_dir = session_path
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| {
            format!(
                "Failed to determine Gemini CLI project directory for {}",
                session_path.display()
            )
        })?;
    let project_relative_dir = strip_path_prefix(tmp_root, project_dir).ok_or_else(|| {
        format!(
            "Project path {} is outside Gemini CLI tmp root {}",
            project_dir.display(),
            tmp_root.display()
        )
    })?;
    let project_root = std::fs::read_to_string(project_dir.join(".project_root"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    Ok(serde_json::json!({
        "relativeSessionPath": relative_session_path,
        "projectRelativeDir": project_relative_dir,
        "sessionFileName": session_path.file_name().and_then(|name| name.to_str()).unwrap_or_default(),
        "sessionFileContent": session_file_content,
        "sessionFile": session_file,
        "projectRoot": project_root,
        "sessionId": session.session_id,
    }))
}

pub fn import_native_snapshot(
    tmp_root: &Path,
    session_id: &str,
    snapshot: &Value,
) -> Result<PathBuf, String> {
    let session_file_name = snapshot
        .get("sessionFileName")
        .and_then(Value::as_str)
        .filter(|value| is_supported_session_file_name(value))
        .map(str::to_string)
        .or_else(|| {
            snapshot
                .get("sessionFileContent")
                .and_then(Value::as_str)
                .map(|_| {
                    format!(
                        "session-{}.jsonl",
                        sanitize_path_segment(session_id, "gemini-session")
                    )
                })
        })
        .unwrap_or_else(|| {
            format!(
                "session-{}.json",
                sanitize_path_segment(session_id, "gemini-session")
            )
        });

    let session_file_content = if let Some(raw_content) = snapshot
        .get("sessionFileContent")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        let conversation =
            parse_conversation_content(raw_content, false, session_file_name.ends_with(".jsonl"))?;
        if conversation.session_id != session_id {
            return Err(format!(
                "Gemini CLI snapshot sessionId {} does not match export meta {}",
                conversation.session_id, session_id
            ));
        }
        ensure_trailing_newline(raw_content)
    } else {
        let session_file = snapshot
            .get("sessionFile")
            .cloned()
            .filter(|value| !value.is_null())
            .ok_or_else(|| "Gemini CLI snapshot missing sessionFile".to_string())?;
        let conversation = conversation_from_legacy_value(session_file.clone(), false)?;
        if conversation.session_id != session_id {
            return Err(format!(
                "Gemini CLI snapshot sessionId {} does not match export meta {}",
                conversation.session_id, session_id
            ));
        }
        let serialized = serde_json::to_string_pretty(&session_file)
            .map_err(|error| format!("Failed to serialize Gemini CLI session: {error}"))?;
        format!("{serialized}\n")
    };

    let relative_session_path = snapshot
        .get("relativeSessionPath")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .filter(|value| {
            Path::new(value)
                .file_name()
                .and_then(|name| name.to_str())
                .map(is_supported_session_file_name)
                .unwrap_or(false)
        });
    let project_relative_dir = snapshot
        .get("projectRelativeDir")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());

    let target_path = if let Some(relative_session_path) = relative_session_path {
        join_safe_relative(tmp_root, relative_session_path)?
    } else {
        let project_dir = project_relative_dir
            .map(str::to_string)
            .unwrap_or_else(|| sanitize_path_segment(session_id, "project"));
        join_safe_relative(
            tmp_root,
            &format!(
                "{}/chats/{}",
                project_dir.trim_end_matches('/'),
                session_file_name
            ),
        )?
    };

    if target_path.exists() {
        return Err(format!(
            "Gemini CLI session file already exists: {}",
            target_path.display()
        ));
    }
    let parent_dir = target_path.parent().ok_or_else(|| {
        format!(
            "Failed to determine Gemini CLI session parent directory for {}",
            target_path.display()
        )
    })?;
    std::fs::create_dir_all(parent_dir).map_err(|error| {
        format!(
            "Failed to create Gemini CLI session directory {}: {error}",
            parent_dir.display()
        )
    })?;
    std::fs::write(&target_path, session_file_content).map_err(|error| {
        format!(
            "Failed to write Gemini CLI session file {}: {error}",
            target_path.display()
        )
    })?;

    if let Some(project_root) = snapshot
        .get("projectRoot")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        let project_dir = target_path.parent().and_then(Path::parent).ok_or_else(|| {
            format!(
                "Failed to determine Gemini CLI project directory for {}",
                target_path.display()
            )
        })?;
        std::fs::write(project_dir.join(".project_root"), project_root).map_err(|error| {
            format!(
                "Failed to write Gemini CLI .project_root in {}: {error}",
                project_dir.display()
            )
        })?;
    }

    Ok(target_path)
}

fn parse_session(path: &Path) -> Option<SessionMeta> {
    let conversation = read_conversation(path, false).ok()?;
    if !conversation.has_user_or_assistant_message
        || conversation.kind.as_deref() == Some("subagent")
    {
        return None;
    }

    let title = conversation
        .summary
        .as_deref()
        .map(|value| truncate_summary(value, 80))
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            conversation
                .first_user_message
                .as_deref()
                .and_then(|text| extract_prompt_title_text(text, 80))
        })
        .or_else(|| {
            path.parent()
                .and_then(Path::parent)
                .and_then(|project_dir| project_dir.to_str())
                .and_then(path_basename)
        });

    Some(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id: conversation.session_id.clone(),
        title: title.clone(),
        summary: title.map(|value| truncate_summary(&value, 160)),
        project_dir: None,
        created_at: conversation.start_time,
        last_active_at: conversation.last_updated.or(conversation.start_time),
        source_path: path.to_string_lossy().to_string(),
        resume_command: Some(format!("gemini --resume {}", conversation.session_id)),
        runtime_source: None,
        runtime_distro: None,
    })
}

fn read_conversation(path: &Path, include_messages: bool) -> Result<GeminiConversation, String> {
    let data = std::fs::read_to_string(path)
        .map_err(|error| format!("Failed to read Gemini CLI session: {error}"))?;
    parse_conversation_content(&data, include_messages, is_jsonl_path(path)).map_err(|error| {
        format!(
            "Failed to parse Gemini CLI session {}: {error}",
            path.display()
        )
    })
}

fn parse_conversation_content(
    data: &str,
    include_messages: bool,
    prefer_jsonl: bool,
) -> Result<GeminiConversation, String> {
    if !prefer_jsonl {
        if let Ok(value) = serde_json::from_str::<Value>(data) {
            if value.get("sessionId").is_some() {
                return conversation_from_legacy_value(value, include_messages);
            }
        }
    }

    parse_jsonl_conversation(data, include_messages).or_else(|jsonl_error| {
        serde_json::from_str::<Value>(data)
            .ok()
            .filter(|value| value.get("sessionId").is_some())
            .map(|value| conversation_from_legacy_value(value, include_messages))
            .unwrap_or_else(|| Err(jsonl_error))
    })
}

fn conversation_from_legacy_value(
    value: Value,
    include_messages: bool,
) -> Result<GeminiConversation, String> {
    let session_id = value
        .get("sessionId")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing sessionId".to_string())?
        .to_string();
    let all_messages = value
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let first_user_message = extract_first_user_message(&all_messages);
    let has_user_or_assistant_message = has_user_or_assistant_message(&all_messages);

    Ok(GeminiConversation {
        session_id,
        start_time: value.get("startTime").and_then(parse_timestamp_to_ms),
        last_updated: value.get("lastUpdated").and_then(parse_timestamp_to_ms),
        summary: value
            .get("summary")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        kind: value
            .get("kind")
            .and_then(Value::as_str)
            .map(str::to_string),
        messages: if include_messages {
            all_messages
        } else {
            Vec::new()
        },
        first_user_message,
        has_user_or_assistant_message,
    })
}

fn parse_jsonl_conversation(
    data: &str,
    include_messages: bool,
) -> Result<GeminiConversation, String> {
    let mut session_id: Option<String> = None;
    let mut start_time: Option<i64> = None;
    let mut last_updated: Option<i64> = None;
    let mut summary: Option<String> = None;
    let mut kind: Option<String> = None;
    let mut metadata_messages: Option<Vec<Value>> = None;
    let mut messages: Vec<Value> = Vec::new();
    let mut message_indices: HashMap<String, usize> = HashMap::new();

    for line in data.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let record: Value = match serde_json::from_str(line) {
            Ok(record) => record,
            Err(_) => continue,
        };

        if let Some(rewind_id) = record.get("$rewindTo").and_then(Value::as_str) {
            rewind_messages(&mut messages, &mut message_indices, rewind_id);
            continue;
        }

        if is_message_record(&record) {
            upsert_message(&mut messages, &mut message_indices, record);
            continue;
        }

        if let Some(update) = record.get("$set").and_then(Value::as_object) {
            update_metadata(
                update,
                &mut session_id,
                &mut start_time,
                &mut last_updated,
                &mut summary,
                &mut kind,
                &mut metadata_messages,
            );
            continue;
        }

        if let Some(object) = record.as_object() {
            update_metadata(
                object,
                &mut session_id,
                &mut start_time,
                &mut last_updated,
                &mut summary,
                &mut kind,
                &mut metadata_messages,
            );
        }
    }

    let session_id = session_id.ok_or_else(|| "missing sessionId".to_string())?;
    let effective_messages = metadata_messages
        .filter(|items| !items.is_empty())
        .unwrap_or(messages);
    let first_user_message = extract_first_user_message(&effective_messages);
    let has_user_or_assistant_message = has_user_or_assistant_message(&effective_messages);

    Ok(GeminiConversation {
        session_id,
        start_time,
        last_updated,
        summary,
        kind,
        messages: if include_messages {
            effective_messages
        } else {
            Vec::new()
        },
        first_user_message,
        has_user_or_assistant_message,
    })
}

fn update_metadata(
    object: &serde_json::Map<String, Value>,
    session_id: &mut Option<String>,
    start_time: &mut Option<i64>,
    last_updated: &mut Option<i64>,
    summary: &mut Option<String>,
    kind: &mut Option<String>,
    metadata_messages: &mut Option<Vec<Value>>,
) {
    if let Some(value) = object.get("sessionId").and_then(Value::as_str) {
        *session_id = Some(value.to_string());
    }
    if let Some(value) = object.get("startTime").and_then(parse_timestamp_to_ms) {
        *start_time = Some(value);
    }
    if let Some(value) = object.get("lastUpdated").and_then(parse_timestamp_to_ms) {
        *last_updated = Some(value);
    }
    if let Some(value) = object
        .get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        *summary = Some(value.to_string());
    }
    if let Some(value) = object.get("kind").and_then(Value::as_str) {
        *kind = Some(value.to_string());
    }
    if let Some(value) = object.get("messages").and_then(Value::as_array) {
        *metadata_messages = Some(value.clone());
    }
}

fn upsert_message(
    messages: &mut Vec<Value>,
    message_indices: &mut HashMap<String, usize>,
    message: Value,
) {
    let Some(id) = message
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return;
    };

    if let Some(index) = message_indices.get(&id).copied() {
        messages[index] = message;
    } else {
        message_indices.insert(id, messages.len());
        messages.push(message);
    }
}

fn rewind_messages(
    messages: &mut Vec<Value>,
    message_indices: &mut HashMap<String, usize>,
    rewind_id: &str,
) {
    if let Some(index) = message_indices.get(rewind_id).copied() {
        messages.truncate(index);
    } else {
        messages.clear();
    }
    rebuild_message_indices(messages, message_indices);
}

fn rebuild_message_indices(messages: &[Value], message_indices: &mut HashMap<String, usize>) {
    message_indices.clear();
    for (index, message) in messages.iter().enumerate() {
        if let Some(id) = message.get("id").and_then(Value::as_str) {
            message_indices.insert(id.to_string(), index);
        }
    }
}

fn is_message_record(value: &Value) -> bool {
    value.get("id").and_then(Value::as_str).is_some()
        && value.get("type").and_then(Value::as_str).is_some()
        && value.get("content").is_some()
}

fn has_user_or_assistant_message(messages: &[Value]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message.get("type").and_then(Value::as_str),
            Some("user") | Some("gemini")
        )
    })
}

fn extract_first_user_message(messages: &[Value]) -> Option<String> {
    let mut fallback: Option<String> = None;

    for message in messages {
        if message.get("type").and_then(Value::as_str) != Some("user") {
            continue;
        }

        let text = normalize_message_content(message, "user");
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }

        if fallback.is_none() {
            fallback = Some(trimmed.to_string());
        }
        if !trimmed.starts_with('/') && !trimmed.starts_with('?') {
            return Some(trimmed.to_string());
        }
    }

    fallback
}

fn normalize_message_content(message: &Value, role: &str) -> String {
    let mut content = message
        .get("displayContent")
        .or_else(|| message.get("content"))
        .map(extract_text)
        .unwrap_or_default();

    if role == "user" {
        if let Some(user_request) = extract_wrapped_user_request_text(&content) {
            content = user_request;
        }
    }

    content
}

fn gemini_blocks_from_message(message: &Value, role: &str) -> Vec<SessionMessageBlock> {
    let mut blocks = Vec::new();

    if role == "assistant" {
        if let Some(thoughts) = message.get("thoughts").and_then(Value::as_array) {
            for thought in thoughts {
                if let Some(text) = gemini_thought_text(thought) {
                    blocks.push(thinking_block(text));
                }
            }
        }
    }

    let content_value = if message.get("content").and_then(Value::as_array).is_some() {
        message.get("content")
    } else {
        message
            .get("displayContent")
            .or_else(|| message.get("content"))
    };
    blocks.extend(gemini_blocks_from_content(content_value, role));

    if let Some(tool_calls) = message.get("toolCalls").and_then(Value::as_array) {
        for (index, call) in tool_calls.iter().enumerate() {
            let raw_name = call
                .get("displayName")
                .or_else(|| call.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let tool_name = gemini_tool_display_name(raw_name).to_string();
            let tool_id = gemini_tool_id(call)
                .unwrap_or_else(|| format!("gemini_tool_{index}_{}", sanitize_tool_id(raw_name)));
            let input = call
                .get("args")
                .or_else(|| call.get("arguments"))
                .or_else(|| call.get("input"))
                .cloned()
                .unwrap_or_else(|| Value::Object(Map::new()));

            blocks.push(tool_call_block(
                Some(tool_id.clone()),
                tool_name.clone(),
                Some(input),
            ));

            if let Some(result) = call.get("result") {
                let status = call
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("success");
                blocks.push(tool_result_block(
                    Some(tool_id),
                    Some(tool_name),
                    Some(extract_gemini_tool_result_content(result)),
                    Some(status.eq_ignore_ascii_case("error")),
                ));
            }

            if let Some(display_text) = call
                .get("resultDisplay")
                .and_then(gemini_result_display_text)
            {
                blocks.push(text_block(display_text));
            }
        }
    }

    blocks
}

fn gemini_blocks_from_content(content: Option<&Value>, role: &str) -> Vec<SessionMessageBlock> {
    let Some(content) = content else {
        return Vec::new();
    };

    match content {
        Value::String(text) => normalized_gemini_text_block(text, role)
            .into_iter()
            .collect(),
        Value::Array(parts) => {
            let mut blocks = Vec::new();
            let mut call_ids: HashMap<String, String> = HashMap::new();
            for (index, part) in parts.iter().enumerate() {
                if let Some(function_call) = part.get("functionCall") {
                    let name = function_call
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    let input = function_call
                        .get("args")
                        .cloned()
                        .unwrap_or_else(|| Value::Object(Map::new()));
                    let call_id = function_call
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("gemini_fc_{index}_{}", sanitize_tool_id(name)));
                    call_ids.insert(name.to_string(), call_id.clone());
                    blocks.push(tool_call_block(
                        Some(call_id),
                        gemini_tool_display_name(name).to_string(),
                        Some(input),
                    ));
                    continue;
                }

                if let Some(function_response) = part.get("functionResponse") {
                    let name = function_response
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    let call_id = function_response
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| call_ids.get(name).cloned())
                        .unwrap_or_else(|| format!("gemini_fc_{index}_{}", sanitize_tool_id(name)));
                    blocks.push(tool_result_block(
                        Some(call_id),
                        Some(gemini_tool_display_name(name).to_string()),
                        Some(extract_gemini_function_response_content(function_response)),
                        None,
                    ));
                    continue;
                }

                if let Some(block) = gemini_block_from_part(part, role) {
                    blocks.push(block);
                }
            }
            blocks
        }
        _ => Vec::new(),
    }
}

fn gemini_block_from_part(part: &Value, role: &str) -> Option<SessionMessageBlock> {
    if let Value::String(text) = part {
        return normalized_gemini_text_block(text, role);
    }

    if let Some(text) = part.get("text").and_then(Value::as_str) {
        if part
            .get("thought")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Some(thinking_block(text.to_string()));
        }
        return normalized_gemini_text_block(text, role);
    }

    if let Some(inline_data) = part.get("inlineData") {
        let mime_type = inline_data
            .get("mimeType")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let label = if mime_type.starts_with("image/") {
            "Image"
        } else {
            "Document"
        };
        return Some(text_block(format!("[{label}: {mime_type}]")));
    }

    if let Some(file_data) = part.get("fileData") {
        let file_uri = file_data
            .get("fileUri")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let mime_type = file_data
            .get("mimeType")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return Some(text_block(format!("[File: {mime_type}] {file_uri}")));
    }

    if let Some(executable_code) = part.get("executableCode") {
        let language = executable_code
            .get("language")
            .and_then(Value::as_str)
            .unwrap_or("python");
        let code = executable_code
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("");
        return Some(text_block(format!("```{language}\n{code}\n```")));
    }

    if let Some(code_result) = part.get("codeExecutionResult") {
        let outcome = code_result
            .get("outcome")
            .and_then(Value::as_str)
            .unwrap_or("UNKNOWN");
        let output = code_result
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or("");
        return Some(text_block(format!("[Code Execution: {outcome}]\n{output}")));
    }

    None
}

fn normalized_gemini_text_block(text: &str, role: &str) -> Option<SessionMessageBlock> {
    let mut content = text.to_string();
    if role == "user" {
        if let Some(user_request) = extract_wrapped_user_request_text(&content) {
            content = user_request;
        }
    }
    if content.trim().is_empty() {
        None
    } else {
        Some(text_block(content))
    }
}

fn gemini_thought_text(thought: &Value) -> Option<String> {
    let subject = thought
        .get("subject")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let description = thought
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let text = if subject.is_empty() {
        description.to_string()
    } else if description.is_empty() {
        subject.to_string()
    } else {
        format!("**{subject}**\n{description}")
    };
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn gemini_tool_id(call: &Value) -> Option<String> {
    call.get("id")
        .or_else(|| call.get("callId"))
        .or_else(|| call.get("toolCallId"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn gemini_tool_display_name(name: &str) -> &str {
    match name {
        "read_file" | "ReadFile" => "Read",
        "write_file" | "WriteFile" | "create_file" => "Write",
        "edit_file" | "EditFile" => "Edit",
        "shell" | "run_command" | "execute_command" => "Bash",
        "list_directory" | "list_dir" => "Glob",
        "search_files" | "grep" => "Grep",
        "web_search" => "WebSearch",
        _ => name,
    }
}

fn extract_gemini_tool_result_content(result: &Value) -> Value {
    if let Some(items) = result.as_array() {
        let text = items
            .iter()
            .filter_map(|item| item.get("functionResponse"))
            .map(extract_gemini_function_response_content)
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string())
            })
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        if !text.trim().is_empty() {
            return Value::String(text);
        }
    }
    match result {
        Value::String(text) => Value::String(text.to_string()),
        _ => result.clone(),
    }
}

fn extract_gemini_function_response_content(function_response: &Value) -> Value {
    function_response
        .get("response")
        .and_then(|response| response.get("output"))
        .cloned()
        .unwrap_or_else(|| {
            function_response
                .get("response")
                .cloned()
                .unwrap_or_else(|| Value::Object(Map::new()))
        })
}

fn gemini_result_display_text(display: &Value) -> Option<String> {
    match display {
        Value::String(text) if !text.trim().is_empty() => Some(text.to_string()),
        Value::Object(object) if object.contains_key("fileDiff") => {
            let file_name = object
                .get("fileName")
                .and_then(Value::as_str)
                .unwrap_or("unknown file");
            Some(format!("[File Change] {file_name}"))
        }
        Value::Object(object) if object.contains_key("todos") => {
            Some("[Task List Updated]".to_string())
        }
        Value::Object(object) if object.contains_key("isSubagentProgress") => {
            let agent_name = object
                .get("agentName")
                .and_then(Value::as_str)
                .unwrap_or("agent");
            Some(format!("[Subagent: {agent_name}]"))
        }
        _ => None,
    }
}

fn gemini_usage_from_tokens(tokens: Option<&Value>) -> Option<SessionMessageUsage> {
    let tokens = tokens?;
    let usage = SessionMessageUsage {
        input_tokens: tokens
            .get("input")
            .or_else(|| tokens.get("input_tokens"))
            .and_then(Value::as_u64)
            .map(|value| value as i64),
        output_tokens: tokens
            .get("output")
            .or_else(|| tokens.get("output_tokens"))
            .and_then(Value::as_u64)
            .map(|value| value as i64),
        cache_creation_input_tokens: None,
        cache_read_input_tokens: tokens
            .get("cached")
            .or_else(|| tokens.get("cache_read_input_tokens"))
            .and_then(Value::as_u64)
            .map(|value| value as i64),
    };

    if usage.input_tokens.is_none()
        && usage.output_tokens.is_none()
        && usage.cache_read_input_tokens.is_none()
    {
        None
    } else {
        Some(usage)
    }
}

fn sanitize_tool_id(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    let normalized = normalized.trim_matches('_');
    if normalized.is_empty() {
        "tool".to_string()
    } else {
        normalized.to_string()
    }
}

fn is_supported_session_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    is_supported_session_file_name(file_name)
}

fn is_supported_session_file_name(value: &str) -> bool {
    value.starts_with(SESSION_FILE_PREFIX)
        && (value.ends_with(".json") || value.ends_with(".jsonl"))
}

fn is_jsonl_path(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some("jsonl")
}

fn ensure_trailing_newline(value: &str) -> String {
    if value.ends_with('\n') {
        value.to_string()
    } else {
        format!("{value}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "ai-toolbox-gemini-session-{label}-{}",
                uuid::Uuid::new_v4().simple()
            ));
            fs::create_dir_all(&path).expect("create test dir");
            Self { path }
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn load_messages_handles_array_content_and_tool_calls() {
        let test_dir = TestDir::new("array-content");
        let session_path = test_dir.path.join("session-2026-03-06T10-00-abc12345.json");
        fs::write(
            &session_path,
            r#"{
              "sessionId": "gemini-session-1",
              "messages": [
                {"timestamp":"2026-03-06T10:00:00Z","type":"user","content":[{"text":"hello"}]},
                {"timestamp":"2026-03-06T10:00:01Z","type":"gemini","content":"world","toolCalls":[{"name":"web_search"}]},
                {"timestamp":"2026-03-06T10:00:02Z","type":"info","content":"skip"}
              ]
            }"#,
        )
        .expect("write session");

        let messages = load_messages(&session_path).expect("load messages");
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "hello");
        assert_eq!(messages[1].role, "assistant");
        assert!(messages[1].content.contains("world"));
        assert!(messages[1].content.contains("[Tool: WebSearch]"));
        assert_eq!(
            messages[1].blocks[1].tool_name.as_deref(),
            Some("WebSearch")
        );
        assert_eq!(
            messages[1].blocks[1].normalized_tool_name.as_deref(),
            Some("web_search")
        );
        assert_eq!(messages[2].role, "system");
        assert_eq!(messages[2].content, "skip");
    }

    #[test]
    fn load_messages_structures_gemini_parts_and_tool_results() {
        let test_dir = TestDir::new("structured-parts");
        let session_path = test_dir
            .path
            .join("session-2026-03-06T10-00-structured.json");
        fs::write(
            &session_path,
            r#"{
              "sessionId": "gemini-structured-session",
              "messages": [
                {
                  "id": "gemini-structured-1",
                  "timestamp": "2026-03-06T10:00:00Z",
                  "type": "gemini",
                  "model": "gemini-2.5-pro",
                  "tokens": { "input": 10, "output": 4, "cached": 2 },
                  "thoughts": [
                    { "subject": "Plan", "description": "Read the file first." }
                  ],
                  "content": [
                    { "text": "Check assumptions", "thought": true },
                    { "text": "Reading file now." },
                    {
                      "functionCall": {
                        "name": "read_file",
                        "args": { "path": "AGENTS.md" }
                      }
                    },
                    {
                      "functionResponse": {
                        "name": "read_file",
                        "response": { "output": "file body" }
                      }
                    },
                    {
                      "executableCode": {
                        "language": "python",
                        "code": "print(1)"
                      }
                    },
                    {
                      "codeExecutionResult": {
                        "outcome": "OUTCOME_OK",
                        "output": "1"
                      }
                    }
                  ]
                },
                {
                  "id": "gemini-structured-2",
                  "timestamp": "2026-03-06T10:00:01Z",
                  "type": "gemini",
                  "content": "Running shell.",
                  "toolCalls": [
                    {
                      "id": "tool_shell_1",
                      "name": "shell",
                      "args": { "command": "pwd" },
                      "status": "error",
                      "result": [
                        {
                          "functionResponse": {
                            "response": { "output": "failed" }
                          }
                        }
                      ],
                      "resultDisplay": {
                        "fileDiff": true,
                        "fileName": "main.rs"
                      }
                    }
                  ]
                }
              ]
            }"#,
        )
        .expect("write session");

        let messages = load_messages(&session_path).expect("load messages");

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "assistant");
        assert_eq!(messages[0].model.as_deref(), Some("gemini-2.5-pro"));
        assert_eq!(
            messages[0]
                .usage
                .as_ref()
                .and_then(|usage| usage.cache_read_input_tokens),
            Some(2)
        );
        assert_eq!(messages[0].blocks[0].kind, "thinking");
        assert!(messages[0].blocks[0]
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("Plan"));
        assert_eq!(messages[0].blocks[1].kind, "thinking");

        let read_block = messages[0]
            .blocks
            .iter()
            .find(|block| block.kind == "tool_execution")
            .expect("read tool block");
        assert_eq!(read_block.tool_name.as_deref(), Some("Read"));
        assert_eq!(read_block.normalized_tool_name.as_deref(), Some("read"));
        assert_eq!(
            read_block
                .input
                .as_ref()
                .and_then(|input| input.get("path"))
                .and_then(serde_json::Value::as_str),
            Some("AGENTS.md")
        );
        assert_eq!(
            read_block
                .output
                .as_ref()
                .and_then(serde_json::Value::as_str),
            Some("file body")
        );

        let shell_block = messages[1]
            .blocks
            .iter()
            .find(|block| block.kind == "tool_execution")
            .expect("shell tool block");
        assert_eq!(shell_block.tool_name.as_deref(), Some("Bash"));
        assert_eq!(shell_block.normalized_tool_name.as_deref(), Some("bash"));
        assert_eq!(shell_block.status.as_deref(), Some("error"));
        assert_eq!(
            shell_block
                .output
                .as_ref()
                .and_then(serde_json::Value::as_str),
            Some("failed")
        );
        assert!(messages[1].content.contains("[File Change] main.rs"));
    }

    #[test]
    fn scan_sessions_reads_current_jsonl_format() {
        let test_dir = TestDir::new("jsonl");
        let project_dir = test_dir.path.join("ai-toolbox");
        let chats_dir = project_dir.join("chats");
        fs::create_dir_all(&chats_dir).expect("create chats");
        fs::write(project_dir.join(".project_root"), "D:/GitHub/ai-toolbox\n")
            .expect("write project root");
        let session_path = chats_dir.join("session-2026-05-10T01-24-a4e8a173.jsonl");
        fs::write(
            &session_path,
            r#"{"sessionId":"a4e8a173-e1b0-469d-8ed0-4f65b3705217","projectHash":"hash","startTime":"2026-05-10T01:24:08.951Z","lastUpdated":"2026-05-10T01:24:08.951Z","kind":"main"}
{"id":"user-1","timestamp":"2026-05-10T01:24:11.888Z","type":"user","content":[{"text":"upgrade"}]}
{"$set":{"lastUpdated":"2026-05-10T01:24:11.889Z"}}
{"id":"gemini-1","timestamp":"2026-05-10T01:24:15.891Z","type":"gemini","content":"working"}
{"id":"gemini-1","timestamp":"2026-05-10T01:24:15.891Z","type":"gemini","content":"done","toolCalls":[{"displayName":"ReadFile","name":"read_file"}]}
"#,
        )
        .expect("write session");

        let sessions = scan_sessions(&test_dir.path);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].title.as_deref(), Some("upgrade"));
        assert_eq!(
            sessions[0].session_id,
            "a4e8a173-e1b0-469d-8ed0-4f65b3705217"
        );
        assert_eq!(
            sessions[0].resume_command.as_deref(),
            Some("pushd \"D:/GitHub/ai-toolbox\" && gemini --resume a4e8a173-e1b0-469d-8ed0-4f65b3705217")
        );

        let messages = load_messages(&session_path).expect("load messages");
        assert_eq!(messages.len(), 2);
        assert!(messages[1].content.contains("done"));
        assert!(messages[1].content.contains("[Tool: Read]"));
        assert_eq!(messages[1].blocks[1].tool_name.as_deref(), Some("Read"));
        assert_eq!(
            messages[1].blocks[1].normalized_tool_name.as_deref(),
            Some("read")
        );
    }

    #[test]
    fn scan_sessions_uses_projects_registry_when_marker_is_missing() {
        let test_dir = TestDir::new("projects-registry");
        let gemini_root = test_dir.path.join(".gemini");
        let tmp_root = gemini_root.join("tmp");
        let project_dir = tmp_root.join("project-key");
        let chats_dir = project_dir.join("chats");
        fs::create_dir_all(&chats_dir).expect("create chats");
        fs::write(
            gemini_root.join("projects.json"),
            r#"{"projects":{"D:/GitHub/project with space":"project-key"}}"#,
        )
        .expect("write projects registry");
        let session_path = chats_dir.join("session-2026-05-10T01-24-a4e8a173.jsonl");
        fs::write(
            &session_path,
            r#"{"sessionId":"a4e8a173-e1b0-469d-8ed0-4f65b3705217","projectHash":"hash","startTime":"2026-05-10T01:24:08.951Z","lastUpdated":"2026-05-10T01:24:08.951Z","kind":"main"}
{"id":"user-1","timestamp":"2026-05-10T01:24:11.888Z","type":"user","content":[{"text":"resume me"}]}
"#,
        )
        .expect("write session");

        let sessions = scan_sessions(&tmp_root);
        assert_eq!(sessions.len(), 1);
        assert_eq!(
            sessions[0].project_dir.as_deref(),
            Some("D:/GitHub/project with space")
        );
        assert_eq!(
            sessions[0].resume_command.as_deref(),
            Some("pushd \"D:/GitHub/project with space\" && gemini --resume a4e8a173-e1b0-469d-8ed0-4f65b3705217")
        );
    }

    #[test]
    fn scan_sessions_keeps_bare_resume_when_project_root_is_unknown() {
        let test_dir = TestDir::new("unknown-project-root");
        let project_dir = test_dir.path.join("project-key");
        let chats_dir = project_dir.join("chats");
        fs::create_dir_all(&chats_dir).expect("create chats");
        let session_path = chats_dir.join("session-2026-05-10T01-24-a4e8a173.jsonl");
        fs::write(
            &session_path,
            r#"{"sessionId":"a4e8a173-e1b0-469d-8ed0-4f65b3705217","projectHash":"hash","startTime":"2026-05-10T01:24:08.951Z","lastUpdated":"2026-05-10T01:24:08.951Z","kind":"main"}
{"id":"user-1","timestamp":"2026-05-10T01:24:11.888Z","type":"user","content":[{"text":"resume me"}]}
"#,
        )
        .expect("write session");

        let sessions = scan_sessions(&test_dir.path);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].project_dir, None);
        assert_eq!(
            sessions[0].resume_command.as_deref(),
            Some("gemini --resume a4e8a173-e1b0-469d-8ed0-4f65b3705217")
        );
    }

    #[test]
    fn delete_session_removes_artifacts_and_subagents() {
        let test_dir = TestDir::new("delete-artifacts");
        let project_dir = test_dir.path.join("project");
        let chats_dir = project_dir.join("chats");
        let logs_dir = project_dir.join("logs");
        let session_id = "6fb5832d-70de-4c8d-b9e8-e73b455e6c72";
        let subagent_id = "subagent-1";
        fs::create_dir_all(&chats_dir).expect("create chats");
        fs::create_dir_all(&logs_dir).expect("create logs");

        let session_path = chats_dir.join("session-2026-05-10T10-20-6fb5832d.jsonl");
        fs::write(
            &session_path,
            format!(
                "{{\"sessionId\":\"{session_id}\",\"projectHash\":\"hash\",\"startTime\":\"2026-05-10T10:20:00.000Z\",\"kind\":\"main\"}}\n\
{{\"id\":\"user-1\",\"timestamp\":\"2026-05-10T10:20:01.000Z\",\"type\":\"user\",\"content\":[{{\"text\":\"delete me\"}}]}}\n"
            ),
        )
        .expect("write session");
        fs::write(logs_dir.join(format!("session-{session_id}.jsonl")), "{}").expect("write log");
        let tool_output_dir = project_dir
            .join("tool-outputs")
            .join(format!("session-{session_id}"));
        fs::create_dir_all(&tool_output_dir).expect("create tool output");
        fs::write(tool_output_dir.join("output.txt"), "secret").expect("write tool output");
        let session_scoped_dir = project_dir.join(session_id).join("plans");
        fs::create_dir_all(&session_scoped_dir).expect("create session dir");
        fs::write(session_scoped_dir.join("plan.md"), "plan").expect("write plan");

        let subagent_dir = chats_dir.join(session_id);
        fs::create_dir_all(&subagent_dir).expect("create subagent dir");
        fs::write(subagent_dir.join(format!("{subagent_id}.jsonl")), "{}")
            .expect("write subagent session");
        fs::write(logs_dir.join(format!("session-{subagent_id}.jsonl")), "{}")
            .expect("write subagent log");
        fs::create_dir_all(
            project_dir
                .join("tool-outputs")
                .join(format!("session-{subagent_id}")),
        )
        .expect("create subagent tool output");
        fs::create_dir_all(project_dir.join(subagent_id).join("tracker"))
            .expect("create subagent session dir");

        delete_session(&session_path).expect("delete session");

        assert!(!session_path.exists());
        assert!(!logs_dir
            .join(format!("session-{session_id}.jsonl"))
            .exists());
        assert!(!tool_output_dir.exists());
        assert!(!project_dir.join(session_id).exists());
        assert!(!subagent_dir.exists());
        assert!(!logs_dir
            .join(format!("session-{subagent_id}.jsonl"))
            .exists());
        assert!(!project_dir
            .join("tool-outputs")
            .join(format!("session-{subagent_id}"))
            .exists());
        assert!(!project_dir.join(subagent_id).exists());
    }

    #[test]
    fn delete_session_removes_session_file_when_artifact_cleanup_fails() {
        let test_dir = TestDir::new("delete-artifact-failure");
        let project_dir = test_dir.path.join("project");
        let chats_dir = project_dir.join("chats");
        let logs_dir = project_dir.join("logs");
        let session_id = "e49d6b52-c845-4b1a-8dc9-f4ed039e7c31";
        fs::create_dir_all(&chats_dir).expect("create chats");
        fs::create_dir_all(&logs_dir).expect("create logs");

        let session_path = chats_dir.join("session-2026-05-10T12-00-e49d6b52.jsonl");
        fs::write(
            &session_path,
            format!(
                "{{\"sessionId\":\"{session_id}\",\"projectHash\":\"hash\",\"startTime\":\"2026-05-10T12:00:00.000Z\",\"kind\":\"main\"}}\n\
{{\"id\":\"user-1\",\"timestamp\":\"2026-05-10T12:00:01.000Z\",\"type\":\"user\",\"content\":[{{\"text\":\"delete despite artifact error\"}}]}}\n"
            ),
        )
        .expect("write session");

        let blocked_log_path = logs_dir.join(format!("session-{session_id}.jsonl"));
        fs::create_dir_all(&blocked_log_path).expect("create blocking log directory");

        delete_session(&session_path).expect("delete session despite artifact cleanup failure");

        assert!(!session_path.exists());
        assert!(blocked_log_path.exists());
    }

    #[test]
    fn wrapped_user_request_is_used_for_title_and_detail() {
        let test_dir = TestDir::new("wrapped-user-request");
        let project_dir = test_dir.path.join("gemini-temp");
        let chats_dir = project_dir.join("chats");
        fs::create_dir_all(&chats_dir).expect("create chats");
        let session_path = chats_dir.join("session-2026-03-08T09-03-456becac.json");
        fs::write(
            &session_path,
            r#"{
              "sessionId": "456becac-355f-44a8-80bf-060bb6e7735d",
              "projectHash": "hash",
              "startTime": "2026-03-08T09:03:30.038Z",
              "lastUpdated": "2026-03-08T09:03:30.038Z",
              "messages": [
                {
                  "id": "user-1",
                  "timestamp": "2026-03-08T09:03:30.038Z",
                  "type": "user",
                  "content": [
                    {
                      "text": "[Assistant Rules - You MUST follow these instructions]\n[Available Skills]\n- cron: Scheduled task management.\n\n[User Request]\nping"
                    }
                  ]
                }
              ]
            }"#,
        )
        .expect("write session");

        let sessions = scan_sessions(&test_dir.path);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].title.as_deref(), Some("ping"));

        let messages = load_messages(&session_path).expect("load messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "ping");
    }

    #[test]
    fn export_import_preserves_jsonl_session_content() {
        let source_dir = TestDir::new("export-jsonl-source");
        let target_dir = TestDir::new("export-jsonl-target");
        let project_dir = source_dir.path.join("ai-toolbox");
        let chats_dir = project_dir.join("chats");
        fs::create_dir_all(&chats_dir).expect("create chats");
        let session_id = "6fb5832d-70de-4c8d-b9e8-e73b455e6c72";
        let session_path = chats_dir.join("session-2026-05-10T10-20-6fb5832d.jsonl");
        let content = format!(
            "{{\"sessionId\":\"{session_id}\",\"projectHash\":\"hash\",\"startTime\":\"2026-05-10T10:20:00.000Z\",\"lastUpdated\":\"2026-05-10T10:21:00.000Z\",\"kind\":\"main\"}}\n\
{{\"id\":\"user-1\",\"timestamp\":\"2026-05-10T10:20:01.000Z\",\"type\":\"user\",\"content\":[{{\"text\":\"restore this session\"}}]}}\n"
        );
        fs::write(&session_path, &content).expect("write session");

        let snapshot =
            export_native_snapshot(&source_dir.path, &session_path).expect("export snapshot");
        let imported_path =
            import_native_snapshot(&target_dir.path, session_id, &snapshot).expect("import");

        assert_eq!(
            fs::read_to_string(&imported_path).expect("read import"),
            content
        );
        assert!(imported_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_supported_session_file_name));
        let sessions = scan_sessions(&target_dir.path);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].title.as_deref(), Some("restore this session"));
    }
}
