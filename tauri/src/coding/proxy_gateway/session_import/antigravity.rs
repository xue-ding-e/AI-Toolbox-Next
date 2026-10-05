use super::parsers::{native_record, read_jsonl_lenient, timestamp, ParsedSession};
use super::{GatewayUsageTool, SessionUsageGranularity, TokenUsage};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

/// Check if a path is a candidate Antigravity transcript:
/// `<cli_root>/brain/<uuid>/.system_generated/logs/transcript_full.jsonl`
/// or `<cli_root>/brain/<uuid>/.system_generated/logs/transcript.jsonl`.
pub(super) fn is_transcript(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
        return false;
    };
    if name != "transcript_full.jsonl" && name != "transcript.jsonl" {
        return false;
    }
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|v| v.to_str())
        == Some("logs")
}

/// For each conversation UUID, pick the larger mirror file between
/// `transcript_full.jsonl` and `transcript.jsonl`, matching the rule in
/// `session_manager/antigravity.rs`.
pub(super) fn select_sources(files: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut by_session: HashMap<String, PathBuf> = HashMap::new();
    for file in files {
        let Some(uuid) = extract_uuid_from_path(&file) else {
            continue;
        };
        match by_session.get(&uuid) {
            Some(existing) => {
                let existing_len = fs::metadata(existing).map(|m| m.len()).unwrap_or(0);
                let current_len = fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
                if current_len > existing_len {
                    by_session.insert(uuid, file);
                }
            }
            None => {
                by_session.insert(uuid, file);
            }
        }
    }
    by_session.into_values().collect()
}

pub(super) fn extract_uuid_from_path(path: &Path) -> Option<String> {
    let logs_dir = path.parent()?;
    let sys_dir = logs_dir.parent()?;
    let uuid_dir = sys_dir.parent()?;
    let uuid_str = uuid_dir.file_name()?.to_str()?;
    if uuid::Uuid::parse_str(uuid_str).is_ok() {
        Some(uuid_str.to_string())
    } else {
        None
    }
}

fn load_db_models(transcript_path: &Path, uuid: &str) -> HashMap<u64, String> {
    let mut map = HashMap::new();
    let Some(root) = transcript_path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
    else {
        return map;
    };
    let db_path = root.join("conversations").join(format!("{uuid}.db"));
    if !db_path.is_file() {
        return map;
    }

    let Ok(conn) = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return map;
    };

    let mut stmt = match conn.prepare("SELECT idx, data FROM gen_metadata ORDER BY idx ASC") {
        Ok(s) => s,
        Err(_) => return map,
    };

    let rows = stmt.query_map([], |row| {
        let idx: i64 = row.get(0)?;
        let data: Vec<u8> = row.get(1)?;
        Ok((idx, data))
    });

    if let Ok(rows) = rows {
        for row in rows.flatten() {
            let (idx, data) = row;
            if let Some(model) = extract_model_from_blob(&data) {
                map.insert(idx as u64, model);
            }
        }
    }

    map
}

fn extract_model_from_blob(data: &[u8]) -> Option<String> {
    let mut i = 0;
    while i < data.len() {
        if data[i] >= 0x20 && data[i] <= 0x7e {
            let start = i;
            while i < data.len() && data[i] >= 0x20 && data[i] <= 0x7e {
                i += 1;
            }
            if let Ok(s) = std::str::from_utf8(&data[start..i]) {
                let trimmed = s.trim();
                let lower = trimmed.to_ascii_lowercase();
                if (lower.starts_with("gemini-")
                    || lower.starts_with("claude-")
                    || lower.starts_with("gpt-")
                    || lower.starts_with("deepseek-")
                    || lower.starts_with("auto-gemini"))
                    && !lower.contains(' ')
                    && !lower.contains('(')
                    && !lower.contains(')')
                    && !lower.contains('^')
                {
                    return Some(trimmed.to_string());
                }
            }
        } else {
            i += 1;
        }
    }
    None
}

fn parse_model_from_settings_change(content: &str) -> Option<String> {
    let start_idx = content.find("`Model Selection` from ")?;
    let to_part = &content[start_idx..];
    let to_idx = to_part.find(" to ")?;
    let target = &to_part[to_idx + 4..];
    let end_idx = target
        .find(". ")
        .or_else(|| target.find('\n'))
        .or_else(|| target.find("</"))
        .unwrap_or(target.len());
    let raw_name = target[..end_idx].trim().trim_end_matches('.');
    if raw_name.is_empty() {
        return None;
    }
    let lower = raw_name.to_ascii_lowercase();
    if lower.contains("claude") && lower.contains("opus") {
        Some("claude-opus-4-6-thinking".to_string())
    } else if lower.contains("claude") && lower.contains("sonnet") {
        Some("claude-3-7-sonnet".to_string())
    } else if lower.contains("flash") {
        Some("gemini-2.5-flash".to_string())
    } else if lower.contains("pro") {
        Some("gemini-2.5-pro".to_string())
    } else {
        Some(raw_name.to_string())
    }
}

pub(super) fn parse(path: &Path, fallback: i64) -> Result<ParsedSession, String> {
    let session_id = extract_uuid_from_path(path).unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    });

    let db_models = load_db_models(path, &session_id);
    let mut current_model: Option<String> = None;
    let mut started_at: Option<i64> = None;
    let mut records = BTreeMap::new();

    let _pending = read_jsonl_lenient(path, |_line_idx, value| {
        if started_at.is_none() {
            started_at = timestamp(&value);
        }

        if let Some(content) = value.get("content").and_then(Value::as_str) {
            if content.contains("<USER_SETTINGS_CHANGE>") && content.contains("Model Selection") {
                if let Some(model) = parse_model_from_settings_change(content) {
                    current_model = Some(model);
                }
            }
        }

        let source = value.get("source").and_then(Value::as_str);
        if source != Some("MODEL") {
            return;
        }

        let step_index = value.get("step_index").and_then(Value::as_u64).unwrap_or(0);
        let created_at = timestamp(&value).unwrap_or(fallback);

        let model = db_models
            .get(&step_index)
            .cloned()
            .or_else(|| current_model.clone())
            .unwrap_or_else(|| "gemini-2.5-pro".to_string());

        let usage = TokenUsage {
            input_tokens: Some(0),
            output_tokens: Some(0),
            ..Default::default()
        };

        let mut record = native_record(
            GatewayUsageTool::Antigravity,
            &session_id,
            &format!("step:{step_index}"),
            Some(model),
            usage,
            created_at,
        );
        record.metadata.granularity = SessionUsageGranularity::Turn;
        record.metadata.call_count = Some(1);

        records.insert(record.request_id.clone(), record);
    })?;

    Ok(ParsedSession {
        records: records.into_values().collect(),
        started_at,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_transcript_matches_only_valid_mirror_files() {
        assert!(is_transcript(Path::new(
            "brain/11111111-2222-3333-4444-555555555555/.system_generated/logs/transcript_full.jsonl"
        )));
        assert!(is_transcript(Path::new(
            "brain/11111111-2222-3333-4444-555555555555/.system_generated/logs/transcript.jsonl"
        )));
        assert!(!is_transcript(Path::new(
            "brain/11111111-2222-3333-4444-555555555555/.system_generated/subagents/sub.json"
        )));
        assert!(!is_transcript(Path::new(
            "brain/11111111-2222-3333-4444-555555555555/.system_generated/logs/chunks/transcript/00000000.jsonl"
        )));
    }

    #[test]
    fn select_sources_picks_larger_file_for_same_session() {
        let temp = tempfile::tempdir().unwrap();
        let logs = temp
            .path()
            .join("brain")
            .join("11111111-2222-3333-4444-555555555555")
            .join(".system_generated")
            .join("logs");
        fs::create_dir_all(&logs).unwrap();

        let short_file = logs.join("transcript.jsonl");
        let long_file = logs.join("transcript_full.jsonl");

        fs::write(&short_file, "{\"short\":true}\n").unwrap();
        fs::write(
            &long_file,
            "{\"long\":true,\"content\":\"more content here\"}\n",
        )
        .unwrap();

        let selected = select_sources(vec![short_file, long_file.clone()]);
        assert_eq!(selected, vec![long_file]);
    }

    #[test]
    fn parse_model_from_settings_change_extracts_canonical_names() {
        let snippet = "<USER_SETTINGS_CHANGE>\nThe user changed setting `Model Selection` from None to Claude Opus 4.6 (Thinking). No need to comment.\n</USER_SETTINGS_CHANGE>";
        assert_eq!(
            parse_model_from_settings_change(snippet).as_deref(),
            Some("claude-opus-4-6-thinking")
        );
    }

    #[test]
    fn parse_transcript_creates_turn_records_with_call_count() {
        let temp = tempfile::tempdir().unwrap();
        let logs = temp
            .path()
            .join("brain")
            .join("11111111-2222-3333-4444-555555555555")
            .join(".system_generated")
            .join("logs");
        fs::create_dir_all(&logs).unwrap();
        let transcript = logs.join("transcript_full.jsonl");

        let content = r#"{"step_index":0,"source":"USER_EXPLICIT","type":"USER_INPUT","status":"DONE","created_at":"2026-09-20T10:00:00Z","content":"<USER_SETTINGS_CHANGE>\nThe user changed setting `Model Selection` from None to Gemini 2.5 Pro.\n</USER_SETTINGS_CHANGE>"}
{"step_index":1,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-09-20T10:00:02Z","content":"Hello world"}
{"step_index":2,"source":"SYSTEM","type":"EPHEMERAL_MESSAGE","status":"DONE","created_at":"2026-09-20T10:00:03Z","content":"system"}
{"step_index":3,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-09-20T10:00:05Z","content":"Step two"}
"#;
        fs::write(&transcript, content).unwrap();

        let parsed = parse(&transcript, 1700000000).unwrap();
        assert_eq!(parsed.records.len(), 2);
        assert_eq!(parsed.records[0].model, "gemini-2.5-pro");
        assert_eq!(
            parsed.records[0].metadata.granularity,
            SessionUsageGranularity::Turn
        );
        assert_eq!(parsed.records[0].metadata.call_count, Some(1));
        assert_eq!(
            parsed.records[0].request_id,
            "SESSION:antigravity:11111111-2222-3333-4444-555555555555:step:1"
        );
        assert_eq!(
            parsed.records[1].request_id,
            "SESSION:antigravity:11111111-2222-3333-4444-555555555555:step:3"
        );
    }

    #[test]
    fn parse_transcript_prefers_db_gen_metadata_models() {
        let temp = tempfile::tempdir().unwrap();
        let uuid = "11111111-2222-3333-4444-555555555555";
        let conversations = temp.path().join("conversations");
        fs::create_dir_all(&conversations).unwrap();
        let db_file = conversations.join(format!("{uuid}.db"));
        let conn = rusqlite::Connection::open(&db_file).unwrap();
        conn.execute(
            "CREATE TABLE gen_metadata (idx INTEGER PRIMARY KEY, data BLOB)",
            [],
        )
        .unwrap();
        // Insert binary blob with embedded model name
        let blob_data = b"\x12\x04\x00\x00claude-opus-4-6-thinking\x00\x00extra";
        conn.execute(
            "INSERT INTO gen_metadata (idx, data) VALUES (1, ?1)",
            [blob_data.as_slice()],
        )
        .unwrap();

        let logs = temp
            .path()
            .join("brain")
            .join(uuid)
            .join(".system_generated")
            .join("logs");
        fs::create_dir_all(&logs).unwrap();
        let transcript = logs.join("transcript_full.jsonl");

        let content = r#"{"step_index":1,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-09-20T10:00:02Z","content":"Hello world"}
"#;
        fs::write(&transcript, content).unwrap();

        let parsed = parse(&transcript, 1700000000).unwrap();
        assert_eq!(parsed.records.len(), 1);
        assert_eq!(parsed.records[0].model, "claude-opus-4-6-thinking");
    }
}
