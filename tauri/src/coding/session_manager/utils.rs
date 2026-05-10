use std::fs::File;
use std::io::{self, BufRead, BufReader, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, FixedOffset};
use serde_json::Value;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PromptWrapperBlock {
    Instructions,
    Permissions,
    Skills,
    Environment,
    UserAction,
    CollaborationMode,
    BracketedPrompt,
}

pub fn read_head_tail_lines(
    path: &Path,
    head_n: usize,
    tail_n: usize,
) -> io::Result<(Vec<String>, Vec<String>)> {
    let file = File::open(path)?;
    let file_len = file.metadata()?.len();

    if file_len < 16_384 {
        let reader = BufReader::new(file);
        let all_lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
        let head = all_lines.iter().take(head_n).cloned().collect();
        let skip = all_lines.len().saturating_sub(tail_n);
        let tail = all_lines.into_iter().skip(skip).collect();
        return Ok((head, tail));
    }

    let reader = BufReader::new(file);
    let head: Vec<String> = reader.lines().take(head_n).map_while(Result::ok).collect();

    let seek_pos = file_len.saturating_sub(16_384);
    let mut tail_file = File::open(path)?;
    tail_file.seek(SeekFrom::Start(seek_pos))?;
    let tail_reader = BufReader::new(tail_file);
    let all_tail: Vec<String> = tail_reader.lines().map_while(Result::ok).collect();

    let skip_first = if seek_pos > 0 { 1 } else { 0 };
    let usable_tail: Vec<String> = all_tail.into_iter().skip(skip_first).collect();
    let skip = usable_tail.len().saturating_sub(tail_n);
    let tail = usable_tail.into_iter().skip(skip).collect();

    Ok((head, tail))
}

pub fn parse_timestamp_to_ms(value: &Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return Some(if number > 1_000_000_000_000 {
            number
        } else {
            number * 1000
        });
    }

    if let Some(number) = value.as_f64() {
        let number = number as i64;
        return Some(if number > 1_000_000_000_000 {
            number
        } else {
            number * 1000
        });
    }

    let raw = value.as_str()?;
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt: DateTime<FixedOffset>| dt.timestamp_millis())
}

pub fn extract_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.to_string(),
        Value::Array(items) => items
            .iter()
            .filter_map(extract_text_from_item)
            .filter(|text| !text.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(map) => map
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        _ => String::new(),
    }
}

fn extract_text_from_item(item: &Value) -> Option<String> {
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");

    if item_type == "tool_use" {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return Some(format!("[Tool: {name}]"));
    }

    if item_type == "tool_result" {
        if let Some(content) = item.get("content") {
            let text = extract_text(content);
            if !text.is_empty() {
                return Some(text);
            }
        }
        return None;
    }

    if let Some(text) = item.get("text").and_then(Value::as_str) {
        return Some(text.to_string());
    }

    if let Some(text) = item.get("input_text").and_then(Value::as_str) {
        return Some(text.to_string());
    }

    if let Some(text) = item.get("output_text").and_then(Value::as_str) {
        return Some(text.to_string());
    }

    if let Some(content) = item.get("content") {
        let text = extract_text(content);
        if !text.is_empty() {
            return Some(text);
        }
    }

    None
}

pub fn extract_prompt_title_text(text: &str, max_chars: usize) -> Option<String> {
    let unwrapped_user_request = extract_wrapped_user_request_text(text);
    let text = unwrapped_user_request.as_deref().unwrap_or(text);
    let mut active_wrapper: Option<PromptWrapperBlock> = None;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if is_bracketed_prompt_wrapper_start(line) {
            active_wrapper = Some(PromptWrapperBlock::BracketedPrompt);
            continue;
        }

        if let Some(wrapper) = active_wrapper {
            if is_prompt_wrapper_end(line, wrapper) {
                active_wrapper = None;
            }
            continue;
        }

        if let Some(wrapper) = detect_prompt_wrapper_start(line) {
            if !is_prompt_wrapper_end(line, wrapper) {
                active_wrapper = Some(wrapper);
            }
            continue;
        }

        if is_prompt_title_noise_line(line) {
            continue;
        }

        let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            continue;
        }

        return Some(truncate_summary(&collapsed, max_chars));
    }

    None
}

pub fn extract_wrapped_user_request_text(text: &str) -> Option<String> {
    let mut is_user_request_block = false;
    let mut lines = Vec::new();

    for raw_line in text.lines() {
        let line = raw_line.trim();

        if let Some(rest) = strip_user_request_marker(line) {
            is_user_request_block = true;
            if !rest.is_empty() {
                lines.push(rest.to_string());
            }
            continue;
        }

        if !is_user_request_block {
            continue;
        }

        if is_bracketed_prompt_section_header(line) {
            break;
        }

        lines.push(raw_line.trim_end().to_string());
    }

    let value = lines.join("\n");
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn strip_user_request_marker(line: &str) -> Option<&str> {
    if line == "[User Request]" {
        return Some("");
    }

    let rest = line.strip_prefix("[User Request]")?.trim_start();
    Some(rest.strip_prefix(':').unwrap_or(rest).trim_start())
}

fn is_bracketed_prompt_wrapper_start(line: &str) -> bool {
    line.starts_with("[Assistant Rules")
        || line == "[Available Skills]"
        || line == "[Available Tools]"
}

fn is_bracketed_prompt_section_header(line: &str) -> bool {
    line.len() >= 3 && line.len() <= 120 && line.starts_with('[') && line.ends_with(']')
}

fn detect_prompt_wrapper_start(line: &str) -> Option<PromptWrapperBlock> {
    if line.starts_with("# AGENTS.md instructions") || line == "<INSTRUCTIONS>" {
        return Some(PromptWrapperBlock::Instructions);
    }

    match line {
        "<permissions instructions>" => Some(PromptWrapperBlock::Permissions),
        "<skills_instructions>" => Some(PromptWrapperBlock::Skills),
        "<environment_context>" => Some(PromptWrapperBlock::Environment),
        "<user_action>" => Some(PromptWrapperBlock::UserAction),
        "<collaboration_mode>" => Some(PromptWrapperBlock::CollaborationMode),
        _ => None,
    }
}

fn is_prompt_wrapper_end(line: &str, wrapper: PromptWrapperBlock) -> bool {
    match wrapper {
        PromptWrapperBlock::Instructions => line == "</INSTRUCTIONS>",
        PromptWrapperBlock::Permissions => line == "</permissions instructions>",
        PromptWrapperBlock::Skills => line == "</skills_instructions>",
        PromptWrapperBlock::Environment => line == "</environment_context>",
        PromptWrapperBlock::UserAction => line == "</user_action>",
        PromptWrapperBlock::CollaborationMode => line == "</collaboration_mode>",
        PromptWrapperBlock::BracketedPrompt => false,
    }
}

fn is_prompt_title_noise_line(line: &str) -> bool {
    if line.starts_with('/') || line.starts_with("Based on this message") {
        return true;
    }

    if line.starts_with('<') && line.ends_with('>') {
        return true;
    }

    let lowercase = line.to_lowercase();
    matches!(
        lowercase.as_str(),
        "hi" | "hello" | "hey" | "在吗" | "在么" | "在不在" | "你好" | "您好" | "嗨"
    )
}

pub fn truncate_summary(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }

    let mut result = trimmed.chars().take(max_chars).collect::<String>();
    result.push_str("...");
    result
}

pub fn path_basename(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.trim_end_matches(['/', '\\']);
    let last = normalized
        .split(['/', '\\'])
        .next_back()
        .filter(|segment| !segment.is_empty())?;

    Some(last.to_string())
}

pub fn text_contains_query(value: &str, query_lower: &str) -> bool {
    if query_lower.is_empty() {
        return false;
    }

    value.to_lowercase().contains(query_lower)
}

pub fn strip_path_prefix(base: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(base)
        .ok()
        .map(|value| value.to_string_lossy().replace('\\', "/"))
}

pub fn join_safe_relative(base: &Path, relative: &str) -> Result<PathBuf, String> {
    let candidate = Path::new(relative.trim());
    if candidate.as_os_str().is_empty() {
        return Err("Relative path cannot be empty".to_string());
    }

    let mut resolved = PathBuf::from(base);
    for component in candidate.components() {
        match component {
            Component::Normal(value) => resolved.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("Unsafe relative path: {relative}"));
            }
        }
    }

    Ok(resolved)
}

pub fn sanitize_path_segment(value: &str, fallback: &str) -> String {
    let sanitized: String = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect();

    let normalized = sanitized.trim_matches(['-', '.', '_']);
    if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized.to_string()
    }
}
