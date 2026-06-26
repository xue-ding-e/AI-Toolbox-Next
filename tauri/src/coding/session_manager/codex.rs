use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::message_blocks::{
    message_from_blocks, text_block, thinking_block, tool_call_block, tool_result_block,
    usage_from_value,
};
use super::utils::{
    build_resume_command, extract_prompt_title_text, extract_text, join_safe_relative,
    parse_timestamp_to_ms, path_basename, read_head_tail_lines, sanitize_path_segment,
    strip_path_prefix, text_contains_query, truncate_summary,
};
use super::{assign_missing_message_ids, SessionMessage, SessionMeta};

const PROVIDER_ID: &str = "codex";
const SESSION_INDEX_FILE_NAME: &str = "session_index.jsonl";

static UUID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")
        .unwrap()
});

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionIndexEntry {
    id: String,
    thread_name: String,
    #[serde(default)]
    updated_at: String,
}

pub fn scan_sessions(root: &Path) -> Vec<SessionMeta> {
    let mut files = Vec::new();
    collect_jsonl_files(root, &mut files);

    let thread_names = read_thread_names_by_session_id(root);

    files
        .into_iter()
        .filter_map(|path| parse_session(&path))
        .map(|mut session| {
            if let Some(thread_name) = thread_names.get(&session.session_id) {
                session.title = Some(thread_name.clone());
            }
            session
        })
        .collect()
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let file = File::open(path).map_err(|error| format!("Failed to open session file: {error}"))?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();
    let mut current_model: Option<String> = None;
    let mut prev_token_usage = CodexTokenUsageTotals::default();

    for line in reader.lines() {
        let line = match line {
            Ok(value) => value,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&line) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };

        let record_type = value.get("type").and_then(Value::as_str).unwrap_or("");
        let ts = value.get("timestamp").and_then(parse_timestamp_to_ms);
        match record_type {
            "turn_context" => {
                if let Some(model) = value
                    .get("payload")
                    .and_then(|payload| payload.get("model"))
                    .and_then(Value::as_str)
                {
                    current_model = Some(model.to_string());
                }
                continue;
            }
            "response_item" => {}
            "event_msg" => {
                if let Some(payload) = value.get("payload") {
                    if apply_codex_event_message(&mut messages, payload, ts, &mut prev_token_usage)
                    {
                        continue;
                    }
                }
                continue;
            }
            "compacted" => {
                if let Some(payload) = value.get("payload") {
                    messages.push(codex_compacted_message(payload, ts));
                }
                continue;
            }
            _ => continue,
        }

        let payload = match value.get("payload") {
            Some(payload) => payload,
            None => continue,
        };

        let Some(message) = codex_message_from_payload(payload, ts, current_model.as_deref())
        else {
            continue;
        };
        if message.content.trim().is_empty() {
            continue;
        }
        messages.push(message);
    }

    assign_missing_message_ids(&mut messages, PROVIDER_ID);
    Ok(messages)
}

pub fn scan_messages_for_query(path: &Path, query_lower: &str) -> Result<bool, String> {
    let file = File::open(path).map_err(|error| format!("Failed to open session file: {error}"))?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();
    let mut current_model: Option<String> = None;
    let mut prev_token_usage = CodexTokenUsageTotals::default();

    for line in reader.lines() {
        let line = match line {
            Ok(value) => value,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&line) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };

        if let Some(text) = codex_record_search_text(
            &value,
            &mut messages,
            &mut current_model,
            &mut prev_token_usage,
        ) {
            if text_contains_query(&text, query_lower) {
                return Ok(true);
            }
        }
        if let Some(last_message) = messages.last() {
            if text_contains_query(&last_message.content, query_lower) {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

#[derive(Default)]
struct CodexTokenUsageTotals {
    input_tokens: i64,
    output_tokens: i64,
    cached_input_tokens: i64,
}

fn codex_record_search_text(
    value: &Value,
    messages: &mut Vec<SessionMessage>,
    current_model: &mut Option<String>,
    prev_token_usage: &mut CodexTokenUsageTotals,
) -> Option<String> {
    let record_type = value.get("type").and_then(Value::as_str).unwrap_or("");
    let ts = value.get("timestamp").and_then(parse_timestamp_to_ms);
    match record_type {
        "turn_context" => {
            if let Some(model) = value
                .get("payload")
                .and_then(|payload| payload.get("model"))
                .and_then(Value::as_str)
            {
                *current_model = Some(model.to_string());
            }
            None
        }
        "response_item" => {
            let payload = value.get("payload")?;
            let message = codex_message_from_payload(payload, ts, current_model.as_deref())?;
            if message.content.trim().is_empty() {
                return None;
            }
            let text = message.content.clone();
            messages.push(message);
            Some(text)
        }
        "event_msg" => {
            let payload = value.get("payload")?;
            let previous_len = messages.len();
            apply_codex_event_message(messages, payload, ts, prev_token_usage);
            if messages.len() > previous_len {
                return messages.last().map(|message| message.content.clone());
            }
            None
        }
        "compacted" => {
            let message = codex_compacted_message(value.get("payload")?, ts);
            let text = message.content.clone();
            messages.push(message);
            Some(text)
        }
        _ => None,
    }
}

fn codex_message_from_payload(
    payload: &Value,
    ts: Option<i64>,
    current_model: Option<&str>,
) -> Option<SessionMessage> {
    let payload_type = payload.get("type").and_then(Value::as_str).unwrap_or("");
    let mut message = match payload_type {
        "message" => {
            let role = payload
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let content = payload.get("content").map(extract_text).unwrap_or_default();
            if content.trim().is_empty() {
                return None;
            }
            message_from_blocks(role, ts, vec![text_block(content)])
        }
        "local_shell_call" => {
            let tool_id = codex_tool_id(payload);
            let input = codex_local_shell_input(payload);
            message_from_blocks(
                "assistant",
                ts,
                vec![tool_call_block(tool_id, "Bash".to_string(), Some(input))],
            )
        }
        "function_call" | "custom_tool_call" => {
            let name = codex_tool_name(payload_type, payload);
            let tool_id = codex_tool_id(payload);
            let input = codex_tool_input(payload_type, &name, payload);
            message_from_blocks(
                "assistant",
                ts,
                vec![tool_call_block(tool_id, name, Some(input))],
            )
        }
        "web_search_call" => {
            let tool_id = codex_tool_id(payload);
            let input = codex_web_search_input(
                payload
                    .get("action")
                    .cloned()
                    .unwrap_or_else(|| Value::Object(Map::new())),
            );
            message_from_blocks(
                "assistant",
                ts,
                vec![tool_call_block(
                    tool_id,
                    "WebSearch".to_string(),
                    Some(input),
                )],
            )
        }
        "function_call_output" | "custom_tool_call_output" => {
            let tool_id = codex_tool_id(payload);
            let output = payload
                .get("output")
                .cloned()
                .map(normalize_codex_tool_output);
            message_from_blocks(
                "tool",
                ts,
                vec![tool_result_block(tool_id, None, output, None)],
            )
        }
        "reasoning" => {
            let content = codex_reasoning_text(payload);
            if content.trim().is_empty() {
                return None;
            }
            message_from_blocks("assistant", ts, vec![thinking_block(content)])
        }
        _ => return None,
    };

    message.id = payload
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string);
    message.message_type = Some(payload_type.to_string());
    message.model = payload
        .get("model")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| current_model.map(str::to_string));
    message.usage = payload.get("usage").and_then(usage_from_value);
    Some(message)
}

fn apply_codex_event_message(
    messages: &mut Vec<SessionMessage>,
    payload: &Value,
    ts: Option<i64>,
    prev_token_usage: &mut CodexTokenUsageTotals,
) -> bool {
    let event_type = payload.get("type").and_then(Value::as_str).unwrap_or("");
    match event_type {
        "user_message" | "agent_message" => true,
        "token_count" => {
            if let Some(delta_usage) = codex_token_delta_usage(payload, prev_token_usage) {
                if let Some(last_message) = messages
                    .iter_mut()
                    .rev()
                    .find(|message| message.role == "assistant" && message.usage.is_none())
                {
                    last_message.usage = Some(delta_usage);
                }
            }
            true
        }
        "agent_reasoning" => true,
        "context_compacted" => {
            let mut message =
                message_from_blocks("system", ts, vec![text_block("Context compacted")]);
            message.message_type = Some(event_type.to_string());
            messages.push(message);
            true
        }
        "task_started" | "task_complete" => {
            let turn_id = payload.get("turn_id").and_then(Value::as_str).unwrap_or("");
            let text = if event_type == "task_started" {
                format!("[Task Started] turn: {turn_id}")
            } else {
                format!("[Task Completed] turn: {turn_id}")
            };
            let mut message = message_from_blocks("system", ts, vec![text_block(text)]);
            message.message_type = Some(event_type.to_string());
            message.metadata = Some(serde_json::json!({
                "turnId": turn_id,
            }));
            messages.push(message);
            true
        }
        "turn_aborted" => {
            let reason = payload
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let turn_id = payload.get("turn_id").and_then(Value::as_str).unwrap_or("");
            let text = format!("[Turn Aborted] reason: {reason}, turn: {turn_id}");
            let mut message = message_from_blocks("system", ts, vec![text_block(text)]);
            message.message_type = Some(event_type.to_string());
            messages.push(message);
            true
        }
        _ => false,
    }
}

fn codex_compacted_message(payload: &Value, ts: Option<i64>) -> SessionMessage {
    let replacement_history_count = payload
        .get("replacement_history")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let mut message = message_from_blocks("system", ts, vec![text_block("Conversation compacted")]);
    message.message_type = Some("compacted".to_string());
    message.metadata = Some(serde_json::json!({
        "replacementHistoryCount": replacement_history_count,
    }));
    message
}

fn codex_token_delta_usage(
    payload: &Value,
    previous: &mut CodexTokenUsageTotals,
) -> Option<super::SessionMessageUsage> {
    let total_usage = payload
        .get("info")
        .and_then(|info| info.get("total_token_usage"))
        .or_else(|| {
            payload
                .get("info")
                .and_then(|info| info.get("last_token_usage"))
        })?;
    let input_tokens = number_field(total_usage, &["input_tokens", "inputTokens"])?;
    let output_tokens = number_field(total_usage, &["output_tokens", "outputTokens"])?;
    let cached_input_tokens =
        number_field(total_usage, &["cached_input_tokens", "cachedInputTokens"]).unwrap_or(0);

    let delta_input = input_tokens.saturating_sub(previous.input_tokens);
    let delta_output = output_tokens.saturating_sub(previous.output_tokens);
    let delta_cached = cached_input_tokens.saturating_sub(previous.cached_input_tokens);
    previous.input_tokens = input_tokens;
    previous.output_tokens = output_tokens;
    previous.cached_input_tokens = cached_input_tokens;

    let non_cached_input = delta_input.saturating_sub(delta_cached);
    Some(super::SessionMessageUsage {
        input_tokens: Some(non_cached_input),
        output_tokens: Some(delta_output),
        cache_creation_input_tokens: None,
        cache_read_input_tokens: Some(delta_cached),
    })
}

fn codex_tool_id(payload: &Value) -> Option<String> {
    payload
        .get("call_id")
        .or_else(|| payload.get("callId"))
        .or_else(|| payload.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn codex_tool_name(payload_type: &str, payload: &Value) -> String {
    let raw_name = payload
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    match (payload_type, raw_name) {
        ("function_call", "exec_command" | "shell" | "write_stdin") => "Bash",
        _ => raw_name,
    }
    .to_string()
}

fn codex_tool_input(payload_type: &str, tool_name: &str, payload: &Value) -> Value {
    let mut input = if payload_type == "function_call" {
        parse_codex_tool_arguments(payload.get("arguments").or_else(|| payload.get("input")))
    } else {
        payload.get("input").cloned().unwrap_or(Value::Null)
    };

    if payload_type == "custom_tool_call" && !input.is_object() {
        input = if tool_name == "apply_patch" {
            serde_json::json!({ "patch": input.as_str().unwrap_or("") })
        } else {
            serde_json::json!({ "input": input })
        };
    }

    normalize_codex_tool_input(tool_name, input)
}

fn codex_local_shell_input(payload: &Value) -> Value {
    let command = payload
        .get("action")
        .and_then(|action| action.get("command"))
        .cloned()
        .unwrap_or(Value::Null);
    let command = match command {
        Value::Array(parts) => parts
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" "),
        Value::String(command) => command,
        _ => String::new(),
    };
    serde_json::json!({ "command": command })
}

fn parse_codex_tool_arguments(arguments: Option<&Value>) -> Value {
    match arguments {
        Some(Value::String(raw)) => {
            serde_json::from_str(raw).unwrap_or_else(|_| Value::Object(Map::new()))
        }
        Some(value) if value.is_object() || value.is_array() => value.clone(),
        _ => Value::Object(Map::new()),
    }
}

fn normalize_codex_tool_input(tool_name: &str, input: Value) -> Value {
    let Value::Object(mut input_object) = input else {
        return input;
    };

    if tool_name == "Bash" {
        if !input_object.contains_key("command") {
            if let Some(cmd) = input_object.get("cmd").cloned() {
                let command = match cmd {
                    Value::Array(parts) => parts
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" "),
                    Value::String(command) => command,
                    _ => String::new(),
                };
                if !command.is_empty() {
                    input_object.insert("command".to_string(), Value::String(command));
                }
            }
        }
        if let Some(Value::Array(parts)) = input_object.get("command").cloned() {
            let command = parts
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ");
            input_object.insert("command".to_string(), Value::String(command));
        }
    }

    Value::Object(input_object)
}

fn codex_web_search_input(action: Value) -> Value {
    let Value::Object(action_object) = action else {
        return Value::Object(Map::new());
    };

    let mut input = Map::new();
    for key in ["query", "url", "pattern"] {
        if let Some(value) = action_object.get(key).and_then(Value::as_str) {
            input.insert("query".to_string(), Value::String(value.to_string()));
            break;
        }
    }
    if let Some(queries) = action_object.get("queries").cloned() {
        input.insert("queries".to_string(), queries);
    }
    if let Some(action_type) = action_object.get("type").and_then(Value::as_str) {
        input.insert(
            "action_type".to_string(),
            Value::String(action_type.to_string()),
        );
    }
    Value::Object(input)
}

fn normalize_codex_tool_output(output: Value) -> Value {
    let Value::String(raw) = output else {
        return output;
    };

    if let Ok(parsed) = serde_json::from_str::<Value>(&raw) {
        if let Some(inner_output) = parsed.get("output") {
            return inner_output.clone();
        }
    }
    if let Some((_, output)) = raw.split_once("\nOutput:\n") {
        return Value::String(output.to_string());
    }
    Value::String(raw)
}

fn codex_reasoning_text(payload: &Value) -> String {
    payload
        .get("summary")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("text").and_then(Value::as_str))
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn number_field(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(Value::as_i64)
        .or_else(|| {
            keys.iter()
                .find_map(|key| value.get(*key))
                .and_then(Value::as_u64)
                .map(|value| value as i64)
        })
}

pub fn delete_session(path: &Path) -> Result<(), String> {
    std::fs::remove_file(path)
        .map_err(|error| format!("Failed to delete session file {}: {error}", path.display()))
}

pub fn rename_session(source_path: &str, next_title: &str) -> Result<(), String> {
    let normalized_title = next_title.trim();
    if normalized_title.is_empty() {
        return Err("Session title cannot be empty".to_string());
    }

    let session_path = Path::new(source_path);
    let session_id = parse_session(session_path)
        .map(|session| session.session_id)
        .or_else(|| infer_session_id_from_filename(session_path))
        .ok_or_else(|| {
            format!(
                "Failed to determine Codex session id from {}",
                session_path.display()
            )
        })?;
    let sessions_root = find_sessions_root(session_path).ok_or_else(|| {
        format!(
            "Failed to determine Codex sessions root from {}",
            session_path.display()
        )
    })?;

    append_thread_name(&sessions_root, &session_id, normalized_title)
}

pub fn export_native_snapshot(
    root: &Path,
    session_path: &Path,
) -> Result<serde_json::Value, String> {
    let session_id = parse_session(session_path)
        .map(|session| session.session_id)
        .or_else(|| infer_session_id_from_filename(session_path))
        .ok_or_else(|| {
            format!(
                "Failed to determine Codex session id from {}",
                session_path.display()
            )
        })?;
    let relative_session_path = strip_path_prefix(root, session_path).ok_or_else(|| {
        format!(
            "Session path {} is outside Codex session root {}",
            session_path.display(),
            root.display()
        )
    })?;
    let session_file_content = std::fs::read_to_string(session_path).map_err(|error| {
        format!(
            "Failed to read Codex session file {}: {error}",
            session_path.display()
        )
    })?;
    let session_index_entry = read_session_index_entry(root, &session_id);

    Ok(serde_json::json!({
        "relativeSessionPath": relative_session_path,
        "sessionFileContent": session_file_content,
        "sessionIndexEntry": session_index_entry,
    }))
}

pub fn import_native_snapshot(
    root: &Path,
    session_id: &str,
    snapshot: &serde_json::Value,
) -> Result<PathBuf, String> {
    let relative_session_path = snapshot
        .get("relativeSessionPath")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Codex snapshot missing relativeSessionPath".to_string())?;
    let session_file_content = snapshot
        .get("sessionFileContent")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Codex snapshot missing sessionFileContent".to_string())?;

    let fallback_file_name = format!(
        "{}.jsonl",
        sanitize_path_segment(session_id, "codex-session")
    );
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
            "Codex session file already exists: {}",
            target_path.display()
        ));
    }

    let parent_dir = target_path.parent().ok_or_else(|| {
        format!(
            "Failed to determine Codex session parent directory for {}",
            target_path.display()
        )
    })?;
    std::fs::create_dir_all(parent_dir).map_err(|error| {
        format!(
            "Failed to create Codex session directory {}: {error}",
            parent_dir.display()
        )
    })?;
    std::fs::write(&target_path, session_file_content).map_err(|error| {
        format!(
            "Failed to write Codex session file {}: {error}",
            target_path.display()
        )
    })?;
    append_session_index_entry(root, session_id, snapshot.get("sessionIndexEntry"))?;

    Ok(target_path)
}

fn read_thread_names_by_session_id(root: &Path) -> std::collections::HashMap<String, String> {
    let Some(session_index_path) = session_index_path(root) else {
        return std::collections::HashMap::new();
    };
    let data = match std::fs::read_to_string(session_index_path) {
        Ok(data) => data,
        Err(_) => return std::collections::HashMap::new(),
    };

    let mut thread_names = std::collections::HashMap::new();
    for line in data.lines() {
        let Ok(entry) = serde_json::from_str::<SessionIndexEntry>(line) else {
            continue;
        };
        let normalized_thread_name = entry.thread_name.trim();
        if entry.id.trim().is_empty() || normalized_thread_name.is_empty() {
            continue;
        }
        thread_names.insert(entry.id, normalized_thread_name.to_string());
    }

    thread_names
}

fn read_session_index_entry(root: &Path, session_id: &str) -> Option<SessionIndexEntry> {
    let Some(session_index_path) = session_index_path(root) else {
        return None;
    };
    let data = std::fs::read_to_string(session_index_path).ok()?;
    let mut latest_entry = None;

    for line in data.lines() {
        let Ok(entry) = serde_json::from_str::<SessionIndexEntry>(line) else {
            continue;
        };
        if entry.id == session_id && !entry.thread_name.trim().is_empty() {
            latest_entry = Some(entry);
        }
    }

    latest_entry
}

fn append_session_index_entry(
    root: &Path,
    session_id: &str,
    session_index_entry: Option<&Value>,
) -> Result<(), String> {
    let Some(session_index_entry) = session_index_entry.filter(|value| !value.is_null()) else {
        return Ok(());
    };

    let mut parsed_entry = serde_json::from_value::<SessionIndexEntry>(session_index_entry.clone())
        .map_err(|error| format!("Invalid Codex sessionIndexEntry: {error}"))?;
    if parsed_entry.id.trim().is_empty() {
        return Ok(());
    }
    if parsed_entry.id != session_id {
        return Err(format!(
            "Codex sessionIndexEntry id {} does not match session {}",
            parsed_entry.id, session_id
        ));
    }

    let normalized_thread_name = parsed_entry.thread_name.trim();
    if normalized_thread_name.is_empty() {
        return Ok(());
    }
    parsed_entry.thread_name = normalized_thread_name.to_string();

    let session_index_path = session_index_path(root)
        .ok_or_else(|| "Failed to determine Codex session index path".to_string())?;
    if let Some(parent_dir) = session_index_path.parent() {
        std::fs::create_dir_all(parent_dir).map_err(|error| {
            format!(
                "Failed to create Codex session index directory {}: {error}",
                parent_dir.display()
            )
        })?;
    }

    let mut serialized_entry = serde_json::to_string(&parsed_entry)
        .map_err(|error| format!("Failed to serialize Codex sessionIndexEntry: {error}"))?;
    serialized_entry.push('\n');
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&session_index_path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, serialized_entry.as_bytes()))
        .map_err(|error| {
            format!(
                "Failed to write Codex session index {}: {error}",
                session_index_path.display()
            )
        })?;

    Ok(())
}

fn append_thread_name(root: &Path, session_id: &str, thread_name: &str) -> Result<(), String> {
    let normalized_thread_name = thread_name.trim();
    if normalized_thread_name.is_empty() {
        return Err("Session title cannot be empty".to_string());
    }

    let parsed_entry = SessionIndexEntry {
        id: session_id.to_string(),
        thread_name: normalized_thread_name.to_string(),
        updated_at: Utc::now().to_rfc3339(),
    };
    let session_index_path = session_index_path(root)
        .ok_or_else(|| "Failed to determine Codex session index path".to_string())?;
    if let Some(parent_dir) = session_index_path.parent() {
        std::fs::create_dir_all(parent_dir).map_err(|error| {
            format!(
                "Failed to create Codex session index directory {}: {error}",
                parent_dir.display()
            )
        })?;
    }

    let mut serialized_entry = serde_json::to_string(&parsed_entry)
        .map_err(|error| format!("Failed to serialize Codex sessionIndexEntry: {error}"))?;
    serialized_entry.push('\n');
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&session_index_path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, serialized_entry.as_bytes()))
        .map_err(|error| {
            format!(
                "Failed to write Codex session index {}: {error}",
                session_index_path.display()
            )
        })?;

    Ok(())
}

fn parse_session(path: &Path) -> Option<SessionMeta> {
    let (head, tail) = read_head_tail_lines(path, 30, 30).ok()?;

    let mut session_id: Option<String> = None;
    let mut project_dir: Option<String> = None;
    let mut created_at: Option<i64> = None;
    let mut title: Option<String> = None;

    for line in &head {
        let value: Value = match serde_json::from_str(line) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };

        if created_at.is_none() {
            created_at = value.get("timestamp").and_then(parse_timestamp_to_ms);
        }

        if value.get("type").and_then(Value::as_str) == Some("session_meta") {
            if let Some(payload) = value.get("payload") {
                if session_id.is_none() {
                    session_id = payload
                        .get("id")
                        .and_then(Value::as_str)
                        .map(|value| value.to_string());
                }
                if project_dir.is_none() {
                    project_dir = payload
                        .get("cwd")
                        .and_then(Value::as_str)
                        .map(|value| value.to_string());
                }
                if let Some(timestamp) = payload.get("timestamp").and_then(parse_timestamp_to_ms) {
                    created_at.get_or_insert(timestamp);
                }
            }
        }

        if title.is_none() {
            title = extract_user_prompt_title(&value);
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

        if summary.is_none() && value.get("type").and_then(Value::as_str) == Some("response_item") {
            if let Some(payload) = value.get("payload") {
                if payload.get("type").and_then(Value::as_str) == Some("message") {
                    let text = payload.get("content").map(extract_text).unwrap_or_default();
                    if !text.trim().is_empty() {
                        summary = Some(text);
                    }
                }
            }
        }

        if last_active_at.is_some() && summary.is_some() {
            break;
        }
    }

    let session_id = session_id.or_else(|| infer_session_id_from_filename(path))?;
    let title = title.or_else(|| {
        project_dir
            .as_deref()
            .and_then(path_basename)
            .map(|value| value.to_string())
    });
    let resume_command = build_resume_command(
        project_dir.as_deref(),
        &format!("codex resume {session_id}"),
    );

    Some(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id: session_id.clone(),
        title,
        summary: summary.map(|text| truncate_summary(&text, 160)),
        project_dir,
        created_at,
        last_active_at,
        source_path: path.to_string_lossy().to_string(),
        resume_command: Some(resume_command),
        runtime_source: None,
        runtime_distro: None,
    })
}

fn extract_user_prompt_title(value: &Value) -> Option<String> {
    if value.get("type").and_then(Value::as_str) != Some("response_item") {
        return None;
    }

    let payload = value.get("payload")?;
    if payload.get("type").and_then(Value::as_str) != Some("message") {
        return None;
    }
    if payload.get("role").and_then(Value::as_str) != Some("user") {
        return None;
    }

    let text = payload.get("content").map(extract_text).unwrap_or_default();
    normalize_user_prompt_title(&text)
}

fn normalize_user_prompt_title(text: &str) -> Option<String> {
    extract_prompt_title_text(text, 80)
}

fn infer_session_id_from_filename(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_string_lossy();
    UUID_RE
        .find(&file_name)
        .map(|matched| matched.as_str().to_string())
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

fn find_sessions_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|ancestor| ancestor.file_name().and_then(|name| name.to_str()) == Some("sessions"))
        .map(Path::to_path_buf)
}

fn session_index_path(root: &Path) -> Option<PathBuf> {
    root.parent()
        .map(|codex_home| codex_home.join(SESSION_INDEX_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::{load_messages, scan_sessions};

    use std::fs;
    use std::path::{Path, PathBuf};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "ai-toolbox-codex-session-{label}-{}",
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
    fn load_messages_keeps_initial_developer_input_text() {
        let test_dir = TestDir::new("developer-input-text");
        let session_path = test_dir.path().join("rollout.jsonl");
        let content = [
            serde_json::json!({
                "timestamp": "2026-06-07T09:11:00Z",
                "type": "response_item",
                "payload": {
                    "id": "dev-1",
                    "type": "message",
                    "role": "developer",
                    "content": [
                        {
                            "type": "input_text",
                            "text": "Filesystem sandboxing defines which files can be read or written."
                        }
                    ]
                }
            })
            .to_string(),
            serde_json::json!({
                "timestamp": "2026-06-07T09:11:01Z",
                "type": "response_item",
                "payload": {
                    "id": "dev-2",
                    "type": "message",
                    "role": "developer",
                    "content": [
                        {
                            "type": "input_text",
                            "text": r#"# AGENTS.md instructions for D:\GitHub\ai-toolbox"#
                        }
                    ]
                }
            })
            .to_string(),
            serde_json::json!({
                "timestamp": "2026-06-07T09:11:02Z",
                "type": "response_item",
                "payload": {
                    "id": "user-1",
                    "type": "message",
                    "role": "user",
                    "content": [
                        {
                            "type": "input_text",
                            "text": r#"Inspect D:\GitHub\claude-code-history-viewer source"#
                        }
                    ]
                }
            })
            .to_string(),
        ]
        .join("\n");
        fs::write(&session_path, content).expect("failed to write Codex session");

        let messages = load_messages(&session_path).expect("load Codex messages");

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "developer");
        assert!(messages[0]
            .content
            .contains("Filesystem sandboxing defines"));
        assert_eq!(messages[1].role, "developer");
        assert!(messages[1].content.contains("AGENTS.md instructions"));
        assert_eq!(messages[2].role, "user");
        assert!(messages[2]
            .content
            .contains(r#"D:\GitHub\claude-code-history-viewer"#));
    }

    #[test]
    fn load_messages_structures_codex_tools_reasoning_and_events() {
        let test_dir = TestDir::new("structured-events");
        let session_path = test_dir.path().join("rollout.jsonl");
        let content = [
            serde_json::json!({
                "timestamp": "2026-06-07T09:12:00Z",
                "type": "turn_context",
                "payload": {
                    "model": "gpt-5"
                }
            })
            .to_string(),
            serde_json::json!({
                "timestamp": "2026-06-07T09:12:01Z",
                "type": "response_item",
                "payload": {
                    "id": "reasoning-1",
                    "type": "reasoning",
                    "summary": [
                        { "text": "Need to inspect the command output." }
                    ]
                }
            })
            .to_string(),
            serde_json::json!({
                "timestamp": "2026-06-07T09:12:02Z",
                "type": "response_item",
                "payload": {
                    "id": "shell-1",
                    "type": "local_shell_call",
                    "call_id": "tool_shell_1",
                    "action": {
                        "command": ["rg", "needle"]
                    }
                }
            })
            .to_string(),
            serde_json::json!({
                "timestamp": "2026-06-07T09:12:03Z",
                "type": "response_item",
                "payload": {
                    "id": "shell-output-1",
                    "type": "function_call_output",
                    "call_id": "tool_shell_1",
                    "output": "{\"output\":\"done\"}"
                }
            })
            .to_string(),
            serde_json::json!({
                "timestamp": "2026-06-07T09:12:04Z",
                "type": "event_msg",
                "payload": {
                    "type": "token_count",
                    "info": {
                        "total_token_usage": {
                            "input_tokens": 100,
                            "output_tokens": 20,
                            "cached_input_tokens": 10
                        }
                    }
                }
            })
            .to_string(),
            serde_json::json!({
                "timestamp": "2026-06-07T09:12:05Z",
                "type": "event_msg",
                "payload": {
                    "type": "task_started",
                    "turn_id": "turn-1"
                }
            })
            .to_string(),
            serde_json::json!({
                "timestamp": "2026-06-07T09:12:06Z",
                "type": "compacted",
                "payload": {
                    "replacement_history": [{ "id": "older" }]
                }
            })
            .to_string(),
        ]
        .join("\n");
        fs::write(&session_path, content).expect("failed to write Codex session");

        let messages = load_messages(&session_path).expect("load Codex messages");

        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0].blocks[0].kind, "thinking");
        assert_eq!(messages[0].model.as_deref(), Some("gpt-5"));

        let shell_block = &messages[1].blocks[0];
        assert_eq!(shell_block.kind, "tool_call");
        assert_eq!(shell_block.tool_name.as_deref(), Some("Bash"));
        assert_eq!(shell_block.normalized_tool_name.as_deref(), Some("bash"));
        assert_eq!(
            shell_block
                .input
                .as_ref()
                .and_then(|input| input.get("command"))
                .and_then(serde_json::Value::as_str),
            Some("rg needle")
        );
        assert_eq!(
            messages[1]
                .usage
                .as_ref()
                .and_then(|usage| usage.input_tokens),
            Some(90)
        );
        assert_eq!(
            messages[1]
                .usage
                .as_ref()
                .and_then(|usage| usage.cache_read_input_tokens),
            Some(10)
        );

        let result_block = &messages[2].blocks[0];
        assert_eq!(result_block.kind, "tool_result");
        assert_eq!(
            result_block
                .output
                .as_ref()
                .and_then(serde_json::Value::as_str),
            Some("done")
        );

        assert_eq!(messages[3].role, "system");
        assert_eq!(messages[3].message_type.as_deref(), Some("task_started"));
        assert!(messages[3].content.contains("[Task Started]"));

        assert_eq!(messages[4].role, "system");
        assert_eq!(messages[4].content, "Conversation compacted");
        assert_eq!(
            messages[4]
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.get("replacementHistoryCount"))
                .and_then(serde_json::Value::as_u64),
            Some(1)
        );
    }

    #[test]
    fn load_messages_ignores_duplicate_codex_reasoning_event() {
        let test_dir = TestDir::new("duplicate-reasoning-event");
        let session_path = test_dir.path().join("rollout.jsonl");
        let content = [
            serde_json::json!({
                "timestamp": "2026-06-07T09:13:00Z",
                "type": "event_msg",
                "payload": {
                    "type": "agent_reasoning",
                    "text": "Need to inspect the command output."
                }
            })
            .to_string(),
            serde_json::json!({
                "timestamp": "2026-06-07T09:13:01Z",
                "type": "response_item",
                "payload": {
                    "id": "reasoning-1",
                    "type": "reasoning",
                    "summary": [
                        { "text": "Need to inspect the command output." }
                    ]
                }
            })
            .to_string(),
        ]
        .join("\n");
        fs::write(&session_path, content).expect("failed to write Codex session");

        let messages = load_messages(&session_path).expect("load Codex messages");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].blocks[0].kind, "thinking");
        assert_eq!(messages[0].content, "Need to inspect the command output.");
        assert_eq!(messages[0].id.as_deref(), Some("reasoning-1"));
    }

    #[test]
    fn scan_sessions_prefixes_resume_with_project_directory() {
        let test_dir = TestDir::new("resume-project-dir");
        let sessions_root = test_dir.path().join("sessions");
        let session_id = "11111111-2222-3333-4444-555555555555";
        let session_path = sessions_root
            .join("2026")
            .join("05")
            .join("16")
            .join(format!("rollout-2026-05-16T10-00-00-{session_id}.jsonl"));
        if let Some(parent) = session_path.parent() {
            fs::create_dir_all(parent).expect("failed to create session parent");
        }
        fs::write(
            &session_path,
            serde_json::json!({
                "timestamp": "2026-05-16T10:00:00Z",
                "type": "session_meta",
                "payload": {
                    "id": session_id,
                    "timestamp": "2026-05-16T10:00:00Z",
                    "cwd": "D:/GitHub/project with space"
                }
            })
            .to_string(),
        )
        .expect("failed to write session");

        let sessions = scan_sessions(&sessions_root);

        assert_eq!(sessions.len(), 1);
        assert_eq!(
            sessions[0].resume_command.as_deref(),
            Some("pushd \"D:/GitHub/project with space\" && codex resume 11111111-2222-3333-4444-555555555555")
        );
    }
}
