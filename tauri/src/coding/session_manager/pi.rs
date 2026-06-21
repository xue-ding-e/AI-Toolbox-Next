use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use chrono::Utc;
use regex::Regex;
use serde_json::{json, Map, Value};

use super::message_blocks::{
    message_from_blocks, text_block, thinking_block, tool_call_block, tool_result_block,
};
use super::utils::{
    build_resume_command, extract_prompt_title_text, extract_text, join_safe_relative,
    parse_timestamp_to_ms, read_head_tail_lines, sanitize_path_segment, strip_path_prefix,
    text_contains_query, truncate_summary,
};
use super::{assign_missing_message_ids, SessionMessage, SessionMessageUsage, SessionMeta};

const PROVIDER_ID: &str = "pi";

static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")
        .unwrap()
});

pub fn scan_sessions(root: &Path) -> Vec<SessionMeta> {
    let mut files = Vec::new();
    collect_jsonl_files(root, &mut files);

    files
        .into_iter()
        .filter_map(|path| parse_session(&path))
        .collect()
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let file =
        File::open(path).map_err(|error| format!("Failed to open Pi session file: {error}"))?;
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

        if let Some(message) = message_from_entry(&value) {
            messages.push(message);
        }
    }

    assign_missing_message_ids(&mut messages, PROVIDER_ID);
    Ok(messages)
}

pub fn scan_messages_for_query(path: &Path, query_lower: &str) -> Result<bool, String> {
    let file =
        File::open(path).map_err(|error| format!("Failed to open Pi session file: {error}"))?;
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

        if let Some(message) = message_from_entry(&value) {
            if text_contains_query(&message.content, query_lower) {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

pub fn delete_session(path: &Path) -> Result<(), String> {
    std::fs::remove_file(path).map_err(|error| {
        format!(
            "Failed to delete Pi session file {}: {error}",
            path.display()
        )
    })
}

pub fn rename_session(source_path: &str, next_title: &str) -> Result<(), String> {
    let normalized_title = next_title.trim();
    if normalized_title.is_empty() {
        return Err("Session title cannot be empty".to_string());
    }

    let session_path = Path::new(source_path);
    if !session_path.exists() {
        return Err(format!(
            "Pi session file not found: {}",
            session_path.display()
        ));
    }

    let entry = json!({
        "type": "session_info",
        "id": new_entry_id(),
        "parentId": null,
        "timestamp": Utc::now().to_rfc3339(),
        "name": normalized_title,
    });
    let line = serde_json::to_string(&entry)
        .map_err(|error| format!("Failed to serialize Pi session info: {error}"))?;
    std::fs::OpenOptions::new()
        .append(true)
        .open(session_path)
        .and_then(|mut file| {
            use std::io::Write;
            writeln!(file, "{line}")
        })
        .map_err(|error| format!("Failed to append Pi session info: {error}"))
}

pub fn export_native_snapshot(root: &Path, session_path: &Path) -> Result<Value, String> {
    let relative_session_path = strip_path_prefix(root, session_path).ok_or_else(|| {
        format!(
            "Session path {} is outside Pi session root {}",
            session_path.display(),
            root.display()
        )
    })?;
    let session_file_content = std::fs::read_to_string(session_path).map_err(|error| {
        format!(
            "Failed to read Pi session file {}: {error}",
            session_path.display()
        )
    })?;

    Ok(json!({
        "relativeSessionPath": relative_session_path,
        "sessionFileContent": session_file_content,
    }))
}

pub fn import_native_snapshot(
    root: &Path,
    session_id: &str,
    snapshot: &Value,
) -> Result<PathBuf, String> {
    let relative_session_path = snapshot
        .get("relativeSessionPath")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Pi snapshot missing relativeSessionPath".to_string())?;
    let session_file_content = snapshot
        .get("sessionFileContent")
        .and_then(Value::as_str)
        .ok_or_else(|| "Pi snapshot missing sessionFileContent".to_string())?;

    let fallback_file_name = format!("{}.jsonl", sanitize_path_segment(session_id, "pi-session"));
    let normalized_relative_path = if relative_session_path.ends_with(".jsonl") {
        relative_session_path.to_string()
    } else {
        format!(
            "{}/{}",
            relative_session_path.trim_end_matches('/'),
            fallback_file_name
        )
    };

    let target_path = join_safe_relative(root, &normalized_relative_path)?;
    if target_path.exists() {
        return Err(format!(
            "Pi session file already exists: {}",
            target_path.display()
        ));
    }

    let parent_dir = target_path.parent().ok_or_else(|| {
        format!(
            "Failed to determine Pi session parent directory for {}",
            target_path.display()
        )
    })?;
    std::fs::create_dir_all(parent_dir).map_err(|error| {
        format!(
            "Failed to create Pi session directory {}: {error}",
            parent_dir.display()
        )
    })?;
    std::fs::write(&target_path, session_file_content).map_err(|error| {
        format!(
            "Failed to write Pi session file {}: {error}",
            target_path.display()
        )
    })?;

    Ok(target_path)
}

fn collect_jsonl_files(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
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

fn parse_session(path: &Path) -> Option<SessionMeta> {
    let (head, tail) = read_head_tail_lines(path, 80, 80).ok()?;
    let mut session_id = infer_session_id_from_filename(path);
    let mut project_dir = None;
    let mut created_at = None;
    let mut last_active_at = None;
    let mut first_user_title = None;
    let mut latest_session_name = None;

    for line in head.iter().chain(tail.iter()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let entry_type = value.get("type").and_then(Value::as_str).unwrap_or("");
        if entry_type == "session" {
            if let Some(id) = value.get("id").and_then(Value::as_str) {
                session_id = Some(id.to_string());
            }
            project_dir = value
                .get("cwd")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or(project_dir);
            created_at = value
                .get("timestamp")
                .and_then(parse_timestamp_to_ms)
                .or(created_at);
        }

        let ts = value.get("timestamp").and_then(parse_timestamp_to_ms);
        if ts.is_some() && entry_type != "session_info" {
            last_active_at = ts.or(last_active_at);
        }

        if entry_type == "session_info" {
            if let Some(name) = value
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
            {
                latest_session_name = Some(name.to_string());
            }
        } else if first_user_title.is_none() {
            if let Some(message) = message_from_entry(&value) {
                if message.role.eq_ignore_ascii_case("user") && !message.content.trim().is_empty() {
                    first_user_title = extract_prompt_title_text(&message.content, 80)
                        .or_else(|| Some(truncate_summary(&message.content, 80)));
                }
            }
        }
    }

    let session_id = session_id?;
    let source_path = path.to_string_lossy().to_string();
    let resume_command = Some(build_resume_command(
        project_dir.as_deref(),
        &format!("pi --session {}", quote_session_arg(&source_path)),
    ));

    Some(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id,
        title: latest_session_name.or(first_user_title),
        summary: None,
        project_dir,
        created_at,
        last_active_at: last_active_at.or(created_at),
        source_path,
        resume_command,
    })
}

fn message_from_entry(entry: &Value) -> Option<SessionMessage> {
    let entry_type = entry.get("type").and_then(Value::as_str).unwrap_or("");
    let ts = entry.get("timestamp").and_then(parse_timestamp_to_ms);
    match entry_type {
        "message" => message_from_agent_message(entry.get("message")?, ts).map(|mut message| {
            message.id = entry.get("id").and_then(Value::as_str).map(str::to_string);
            message.parent_id = entry
                .get("parentId")
                .and_then(Value::as_str)
                .map(str::to_string);
            message.message_type = Some("message".to_string());
            message
        }),
        "custom_message" => {
            let content = entry
                .get("content")
                .map(extract_text)
                .filter(|text| !text.trim().is_empty())
                .unwrap_or_else(|| entry.to_string());
            let mut message = message_from_blocks("custom", ts, vec![text_block(content)]);
            message.id = entry.get("id").and_then(Value::as_str).map(str::to_string);
            message.parent_id = entry
                .get("parentId")
                .and_then(Value::as_str)
                .map(str::to_string);
            message.message_type = Some("custom_message".to_string());
            message.metadata = Some(entry.clone());
            Some(message)
        }
        "model_change" => {
            let provider = entry.get("provider").and_then(Value::as_str).unwrap_or("");
            let model = entry.get("modelId").and_then(Value::as_str).unwrap_or("");
            let mut message = message_from_blocks(
                "system",
                ts,
                vec![text_block(format!("Model changed to {provider}/{model}"))],
            );
            message.id = entry.get("id").and_then(Value::as_str).map(str::to_string);
            message.parent_id = entry
                .get("parentId")
                .and_then(Value::as_str)
                .map(str::to_string);
            message.message_type = Some("model_change".to_string());
            message.model = if model.is_empty() {
                None
            } else {
                Some(model.to_string())
            };
            message.metadata = Some(entry.clone());
            Some(message)
        }
        "thinking_level_change" => {
            let level = entry
                .get("thinkingLevel")
                .and_then(Value::as_str)
                .unwrap_or("");
            let mut message = message_from_blocks(
                "system",
                ts,
                vec![text_block(format!("Thinking level changed to {level}"))],
            );
            message.id = entry.get("id").and_then(Value::as_str).map(str::to_string);
            message.parent_id = entry
                .get("parentId")
                .and_then(Value::as_str)
                .map(str::to_string);
            message.message_type = Some("thinking_level_change".to_string());
            message.metadata = Some(entry.clone());
            Some(message)
        }
        "compaction" | "branch_summary" => {
            let summary = entry
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if summary.trim().is_empty() {
                return None;
            }
            let role = if entry_type == "compaction" {
                "compactionSummary"
            } else {
                "branchSummary"
            };
            let mut message = message_from_blocks(role, ts, vec![text_block(summary)]);
            message.id = entry.get("id").and_then(Value::as_str).map(str::to_string);
            message.parent_id = entry
                .get("parentId")
                .and_then(Value::as_str)
                .map(str::to_string);
            message.message_type = Some(entry_type.to_string());
            message.metadata = Some(entry.clone());
            Some(message)
        }
        _ => None,
    }
}

fn message_from_agent_message(message: &Value, entry_ts: Option<i64>) -> Option<SessionMessage> {
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let ts = message
        .get("timestamp")
        .and_then(parse_timestamp_to_ms)
        .or(entry_ts);

    let mut result = match role {
        "assistant" => assistant_message(message, ts),
        "toolResult" => tool_result_message(message, ts),
        "bashExecution" => bash_execution_message(message, ts),
        "branchSummary" | "compactionSummary" => summary_message(role, message, ts),
        "custom" => custom_agent_message(message, ts),
        _ => {
            let text = message
                .get("content")
                .map(extract_pi_content_text)
                .unwrap_or_default();
            message_from_blocks(role, ts, vec![text_block(text)])
        }
    };

    result.model = message
        .get("model")
        .and_then(Value::as_str)
        .map(str::to_string);
    result.usage = message.get("usage").and_then(pi_usage_from_value);
    result.cost_usd = message
        .get("usage")
        .and_then(|usage| usage.get("cost"))
        .and_then(|cost| cost.get("total"))
        .and_then(Value::as_f64);
    result.metadata = Some(message.clone());

    Some(result)
}

fn assistant_message(message: &Value, ts: Option<i64>) -> SessionMessage {
    let mut blocks = Vec::new();
    let content = message.get("content").unwrap_or(&Value::Null);
    if let Some(items) = content.as_array() {
        for item in items {
            let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
            match item_type {
                "text" => {
                    if let Some(text) = item.get("text").and_then(Value::as_str) {
                        if !text.trim().is_empty() {
                            blocks.push(text_block(text));
                        }
                    }
                }
                "thinking" => {
                    if let Some(text) = item.get("thinking").and_then(Value::as_str) {
                        if !text.trim().is_empty() {
                            blocks.push(thinking_block(text));
                        }
                    }
                }
                "toolCall" => {
                    let tool_id = item.get("id").and_then(Value::as_str).map(str::to_string);
                    let tool_name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    let input = item.get("arguments").cloned();
                    blocks.push(tool_call_block(tool_id, tool_name, input));
                }
                "image" => {
                    let mime_type = item
                        .get("mimeType")
                        .and_then(Value::as_str)
                        .unwrap_or("image");
                    blocks.push(text_block(format!("[Image: {mime_type}]")));
                }
                _ => blocks.push(text_block(item.to_string())),
            }
        }
    } else {
        let text = extract_pi_content_text(content);
        if !text.trim().is_empty() {
            blocks.push(text_block(text));
        }
    }

    message_from_blocks("assistant", ts, blocks)
}

fn tool_result_message(message: &Value, ts: Option<i64>) -> SessionMessage {
    let tool_id = message
        .get("toolCallId")
        .and_then(Value::as_str)
        .map(str::to_string);
    let tool_name = message
        .get("toolName")
        .and_then(Value::as_str)
        .map(str::to_string);
    let output = message.get("content").map(|content| {
        let text = extract_pi_content_text(content);
        if text.is_empty() {
            content.clone()
        } else {
            Value::String(text)
        }
    });
    message_from_blocks(
        "toolResult",
        ts,
        vec![tool_result_block(
            tool_id,
            tool_name,
            output,
            message.get("isError").and_then(Value::as_bool),
        )],
    )
}

fn bash_execution_message(message: &Value, ts: Option<i64>) -> SessionMessage {
    let command = message
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let output = message
        .get("output")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut output_object = Map::new();
    output_object.insert("command".to_string(), Value::String(command.to_string()));
    output_object.insert("output".to_string(), Value::String(output.to_string()));
    if let Some(exit_code) = message.get("exitCode").cloned() {
        output_object.insert("exitCode".to_string(), exit_code);
    }
    if let Some(cancelled) = message.get("cancelled").cloned() {
        output_object.insert("cancelled".to_string(), cancelled);
    }

    message_from_blocks(
        "bashExecution",
        ts,
        vec![tool_result_block(
            None,
            Some("bash".to_string()),
            Some(Value::Object(output_object)),
            message
                .get("exitCode")
                .and_then(Value::as_i64)
                .map(|exit_code| exit_code != 0),
        )],
    )
}

fn summary_message(role: &str, message: &Value, ts: Option<i64>) -> SessionMessage {
    let text = message
        .get("summary")
        .and_then(Value::as_str)
        .or_else(|| message.get("content").and_then(Value::as_str))
        .unwrap_or_default()
        .to_string();
    message_from_blocks(role, ts, vec![text_block(text)])
}

fn custom_agent_message(message: &Value, ts: Option<i64>) -> SessionMessage {
    let text = message
        .get("content")
        .map(extract_pi_content_text)
        .unwrap_or_default();
    message_from_blocks("custom", ts, vec![text_block(text)])
}

fn extract_pi_content_text(content: &Value) -> String {
    match content {
        Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
                match item_type {
                    "text" => item.get("text").and_then(Value::as_str).map(str::to_string),
                    "thinking" => item
                        .get("thinking")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    "toolCall" => item
                        .get("name")
                        .and_then(Value::as_str)
                        .map(|name| format!("[Tool: {name}]")),
                    "image" => item
                        .get("mimeType")
                        .and_then(Value::as_str)
                        .map(|mime_type| format!("[Image: {mime_type}]")),
                    _ => Some(item.to_string()),
                }
            })
            .filter(|text| !text.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => extract_text(content),
    }
}

fn pi_usage_from_value(value: &Value) -> Option<SessionMessageUsage> {
    let usage = SessionMessageUsage {
        input_tokens: number_field(value, &["input", "inputTokens", "input_tokens"]),
        output_tokens: number_field(value, &["output", "outputTokens", "output_tokens"]),
        cache_creation_input_tokens: number_field(value, &["cacheWrite", "cache_write"]),
        cache_read_input_tokens: number_field(value, &["cacheRead", "cache_read"]),
    };

    if usage.input_tokens.is_none()
        && usage.output_tokens.is_none()
        && usage.cache_creation_input_tokens.is_none()
        && usage.cache_read_input_tokens.is_none()
    {
        None
    } else {
        Some(usage)
    }
}

fn number_field(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| {
        let value = value.get(*key)?;
        value
            .as_i64()
            .or_else(|| value.as_u64().map(|value| value as i64))
    })
}

fn infer_session_id_from_filename(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy();
    UUID_RE
        .find(&name)
        .map(|matched| matched.as_str().to_string())
}

fn quote_session_arg(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\\\""))
}

fn new_entry_id() -> String {
    uuid::Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_field_falls_back_to_next_numeric_key() {
        let value = json!({
            "input": "n/a",
            "inputTokens": 5000,
        });

        assert_eq!(
            number_field(&value, &["input", "inputTokens", "input_tokens"]),
            Some(5000)
        );
    }

    #[test]
    fn rename_session_does_not_change_last_active_at() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let session_path = temp_dir.path().join("pi-session.jsonl");
        let original_content = [
            json!({
                "type": "session",
                "id": "pi-session-id",
                "timestamp": "2026-06-21T09:00:00.000Z",
                "cwd": "/tmp/project"
            })
            .to_string(),
            json!({
                "type": "message",
                "id": "user-message",
                "timestamp": "2026-06-21T09:05:00.000Z",
                "message": {
                    "role": "user",
                    "content": "Original request"
                }
            })
            .to_string(),
        ]
        .join("\n");
        std::fs::write(&session_path, format!("{original_content}\n")).expect("write session");

        let before = parse_session(&session_path).expect("parse before rename");
        rename_session(session_path.to_string_lossy().as_ref(), "Renamed title")
            .expect("rename session");
        let after = parse_session(&session_path).expect("parse after rename");

        assert_eq!(after.title.as_deref(), Some("Renamed title"));
        assert_eq!(after.last_active_at, before.last_active_at);
    }

    #[test]
    fn assistant_message_skips_whitespace_text_before_tool_call() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let session_path = temp_dir.path().join("pi-session.jsonl");
        let entry = json!({
            "type": "message",
            "id": "assistant-tool-call",
            "timestamp": "2026-06-21T09:53:33.916Z",
            "message": {
                "role": "assistant",
                "content": [
                    { "type": "thinking", "thinking": "路径问题，让我修正一下路径格式。\n" },
                    { "type": "text", "text": "\n\n" },
                    {
                        "type": "toolCall",
                        "id": "call_cd43a6c7dacf496296313468",
                        "name": "bash",
                        "arguments": { "command": "ls -la" }
                    }
                ],
                "model": "sensenova-6.7-flash-lite",
                "usage": { "input": 2166, "output": 130 }
            }
        });
        std::fs::write(&session_path, format!("{entry}\n")).expect("write session");

        let messages = load_messages(&session_path).expect("load messages");
        assert_eq!(messages.len(), 1);
        let message = &messages[0];

        assert_eq!(message.role, "assistant");
        assert_eq!(message.blocks.len(), 2);
        assert_eq!(message.blocks[0].kind, "thinking");
        assert_eq!(message.blocks[1].kind, "tool_call");
        assert!(!message.blocks.iter().any(|block| block.kind == "text"
            && block.text.as_deref().unwrap_or_default().trim().is_empty()));
        assert!(message.content.contains("[Tool: bash]"));
        assert_eq!(
            message.usage.as_ref().and_then(|usage| usage.input_tokens),
            Some(2166)
        );
    }
}
