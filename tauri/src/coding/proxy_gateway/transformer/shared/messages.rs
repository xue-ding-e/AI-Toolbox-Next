use super::super::llm::{NamedToolChoice, Stop, ToolChoice, ToolFunction};
use serde_json::{json, Value};

pub fn content_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .or_else(|| part.get("content"))
                    .and_then(Value::as_str)
            })
            .collect::<Vec<_>>()
            .join(""),
        Some(other) => other.as_str().unwrap_or_default().to_string(),
        None => String::new(),
    }
}

pub fn message_parts(value: Option<&Value>) -> Vec<Value> {
    match value {
        Some(Value::Array(parts)) => parts.clone(),
        Some(Value::String(text)) => vec![json!({ "type": "text", "text": text })],
        _ => Vec::new(),
    }
}

pub fn json_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| "{}".to_string()),
    }
}

pub fn tool_arguments_value(raw: &str) -> Value {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return json!({});
    }
    if let Ok(value) = serde_json::from_str(trimmed) {
        return value;
    }
    if let Ok(value) = json5::from_str::<Value>(trimmed) {
        return value;
    }
    if let Ok(repaired) = repair_common_json(trimmed) {
        if let Ok(value) = serde_json::from_str(&repaired) {
            return value;
        }
    }
    Value::String(raw.to_string())
}

pub fn extract_reasoning_field_text(value: &Value) -> Option<String> {
    for field in ["reasoning_content", "reasoning"] {
        if let Some(text) = value
            .get(field)
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
        {
            return Some(text.to_string());
        }
    }

    if let Some(reasoning) = value.get("reasoning") {
        for field in ["content", "text", "summary"] {
            if let Some(text) = reasoning
                .get(field)
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
            {
                return Some(text.to_string());
            }
        }
    }

    value
        .get("reasoning_details")
        .and_then(extract_reasoning_details_text)
}

fn extract_reasoning_details_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => (!text.is_empty()).then(|| text.to_string()),
        Value::Array(parts) => {
            let text = parts
                .iter()
                .filter_map(extract_reasoning_detail_part_text)
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
            (!text.is_empty()).then_some(text)
        }
        Value::Object(_) => extract_reasoning_detail_part_text(value),
        _ => None,
    }
}

fn extract_reasoning_detail_part_text(value: &Value) -> Option<String> {
    for field in ["text", "content", "summary"] {
        if let Some(text) = value
            .get(field)
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
        {
            return Some(text.to_string());
        }
    }

    let parts = value.get("parts").and_then(Value::as_array)?;
    let text = parts
        .iter()
        .filter_map(extract_reasoning_detail_part_text)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    (!text.is_empty()).then_some(text)
}

pub fn split_leading_think_block(raw: &str) -> Option<(String, String)> {
    let rest = strip_leading_think_open_tag(raw)?;
    let close_index = rest.to_ascii_lowercase().find("</think>")?;
    let reasoning = rest[..close_index].trim().to_string();
    let answer = rest[close_index + "</think>".len()..]
        .trim_start_matches(['\r', '\n', ' ', '\t'])
        .to_string();
    (!reasoning.is_empty()).then_some((reasoning, answer))
}

pub fn strip_leading_think_open_tag(raw: &str) -> Option<&str> {
    let trimmed = raw.trim_start_matches(['\r', '\n', ' ', '\t']);
    let lower = trimmed.to_ascii_lowercase();
    lower
        .strip_prefix("<think>")
        .map(|_| &trimmed["<think>".len()..])
}

fn repair_common_json(raw: &str) -> Result<String, serde_json::Error> {
    let without_comments = strip_json_like_comments(raw);
    let without_trailing_commas = strip_trailing_json_commas(&without_comments);
    let single_quoted = normalize_single_quoted_json_strings(&without_trailing_commas);
    let quoted_keys = quote_unquoted_json_keys(&without_trailing_commas);
    let quoted_keys_after_single = quote_unquoted_json_keys(&single_quoted);

    for candidate in [
        without_trailing_commas.as_str(),
        single_quoted.as_str(),
        quoted_keys.as_str(),
        quoted_keys_after_single.as_str(),
    ] {
        if serde_json::from_str::<Value>(candidate).is_ok() {
            return Ok(candidate.to_string());
        }
    }

    serde_json::from_str::<Value>(&without_trailing_commas)?;
    Ok(without_trailing_commas)
}

fn strip_json_like_comments(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for next in chars.by_ref() {
                if next == '\n' || next == '\r' {
                    output.push(next);
                    break;
                }
            }
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut previous = '\0';
            for next in chars.by_ref() {
                if previous == '*' && next == '/' {
                    break;
                }
                previous = next;
            }
            continue;
        }
        output.push(ch);
    }
    output
}

fn strip_trailing_json_commas(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }
        if ch == ',' {
            let mut lookahead = chars.clone();
            while matches!(lookahead.peek(), Some(next) if next.is_whitespace()) {
                lookahead.next();
            }
            if matches!(lookahead.peek(), Some('}' | ']')) {
                continue;
            }
        }
        output.push(ch);
    }
    output
}

fn normalize_single_quoted_json_strings(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    let mut in_double_string = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if in_double_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_double_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_double_string = true;
            output.push(ch);
            continue;
        }

        if ch == '\'' {
            output.push('"');
            let mut single_escaped = false;
            for next in chars.by_ref() {
                if single_escaped {
                    if next == '"' {
                        output.push('\\');
                    }
                    output.push(next);
                    single_escaped = false;
                    continue;
                }
                match next {
                    '\\' => single_escaped = true,
                    '\'' => {
                        output.push('"');
                        break;
                    }
                    '"' => {
                        output.push('\\');
                        output.push('"');
                    }
                    other => output.push(other),
                }
            }
            continue;
        }

        output.push(ch);
    }

    output
}

fn quote_unquoted_json_keys(raw: &str) -> String {
    let chars = raw.chars().collect::<Vec<_>>();
    let mut output = String::with_capacity(raw.len());
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    let mut expecting_key = false;

    while index < chars.len() {
        let ch = chars[index];
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            index += 1;
            continue;
        }

        if ch == '"' {
            in_string = true;
            output.push(ch);
            expecting_key = false;
            index += 1;
            continue;
        }

        if ch == '{' || ch == ',' {
            expecting_key = true;
            output.push(ch);
            index += 1;
            continue;
        }

        if expecting_key && ch.is_whitespace() {
            output.push(ch);
            index += 1;
            continue;
        }

        if expecting_key && is_bare_json_key_start(ch) {
            let start = index;
            index += 1;
            while index < chars.len() && is_bare_json_key_char(chars[index]) {
                index += 1;
            }
            let mut lookahead = index;
            while lookahead < chars.len() && chars[lookahead].is_whitespace() {
                lookahead += 1;
            }
            if lookahead < chars.len() && chars[lookahead] == ':' {
                output.push('"');
                for key_char in &chars[start..index] {
                    output.push(*key_char);
                }
                output.push('"');
                expecting_key = false;
                continue;
            }
            for key_char in &chars[start..index] {
                output.push(*key_char);
            }
            expecting_key = false;
            continue;
        }

        if !ch.is_whitespace() {
            expecting_key = false;
        }
        output.push(ch);
        index += 1;
    }

    output
}

fn is_bare_json_key_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_bare_json_key_char(ch: char) -> bool {
    matches!(ch, '_' | '-' | '.') || ch.is_ascii_alphanumeric()
}

pub fn stop_from_value(value: Option<&Value>) -> Option<Stop> {
    match value {
        Some(Value::String(text)) if !text.is_empty() => Some(Stop::String(text.clone())),
        Some(Value::Array(items)) => {
            let stops = items
                .iter()
                .filter_map(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            (!stops.is_empty()).then_some(Stop::Multiple(stops))
        }
        _ => None,
    }
}

pub fn stop_to_value(stop: Option<Stop>) -> Option<Value> {
    match stop {
        Some(Stop::String(text)) if !text.is_empty() => Some(json!(text)),
        Some(Stop::Multiple(items)) if !items.is_empty() => Some(json!(items)),
        _ => None,
    }
}

pub fn tool_choice_from_openai(value: Option<&Value>) -> Option<ToolChoice> {
    match value {
        Some(Value::String(text)) if !text.is_empty() => Some(ToolChoice::String(text.clone())),
        Some(Value::Object(object)) => {
            if let Some(mode) = object.get("mode").and_then(Value::as_str) {
                return Some(ToolChoice::String(mode.to_string()));
            }
            let name = object
                .get("function")
                .and_then(|function| function.get("name"))
                .or_else(|| object.get("name"))
                .and_then(Value::as_str)?;
            Some(ToolChoice::Named(NamedToolChoice {
                choice_type: "function".to_string(),
                function: ToolFunction {
                    name: name.to_string(),
                },
            }))
        }
        _ => None,
    }
}

pub fn tool_choice_from_anthropic(value: Option<&Value>) -> Option<ToolChoice> {
    if let Some(Value::String(choice)) = value {
        return match choice.as_str() {
            "any" => Some(ToolChoice::String("required".to_string())),
            "auto" | "none" => Some(ToolChoice::String(choice.clone())),
            _ => None,
        };
    }

    let object = value.and_then(Value::as_object)?;
    match object.get("type").and_then(Value::as_str) {
        Some("tool") => object.get("name").and_then(Value::as_str).map(|name| {
            ToolChoice::Named(NamedToolChoice {
                choice_type: "function".to_string(),
                function: ToolFunction {
                    name: name.to_string(),
                },
            })
        }),
        Some("any") => Some(ToolChoice::String("required".to_string())),
        Some("auto") | Some("none") => object
            .get("type")
            .and_then(Value::as_str)
            .map(|choice| ToolChoice::String(choice.to_string())),
        _ => None,
    }
}

pub fn tool_choice_to_anthropic(choice: Option<ToolChoice>) -> Option<Value> {
    match choice {
        Some(ToolChoice::String(choice)) => Some(json!({
            "type": match choice.as_str() {
                "required" => "any",
                "any" => "any",
                "none" => "none",
                _ => "auto",
            }
        })),
        Some(ToolChoice::Named(named)) => Some(json!({
            "type": "tool",
            "name": named.function.name
        })),
        None => None,
    }
}

pub fn tool_choice_to_openai(choice: Option<ToolChoice>) -> Option<Value> {
    match choice {
        Some(ToolChoice::String(choice)) if !choice.is_empty() => Some(json!(if choice == "any" {
            "required"
        } else {
            choice.as_str()
        })),
        Some(ToolChoice::Named(named)) => Some(json!({
            "type": "function",
            "function": {
                "name": named.function.name
            }
        })),
        _ => None,
    }
}

pub fn tool_choice_to_responses(choice: Option<ToolChoice>) -> Option<Value> {
    match choice {
        Some(ToolChoice::String(choice)) if !choice.is_empty() => Some(json!(if choice == "any" {
            "required"
        } else {
            choice.as_str()
        })),
        Some(ToolChoice::Named(named)) => Some(json!({
            "type": "function",
            "name": named.function.name
        })),
        _ => None,
    }
}

pub fn tool_choice_from_gemini(value: Option<&Value>) -> Option<ToolChoice> {
    let config = value?;
    let mode = config.get("mode").and_then(Value::as_str);
    let allowed = config
        .get("allowedFunctionNames")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    if mode == Some("ANY") {
        if allowed.len() == 1 {
            return Some(ToolChoice::Named(NamedToolChoice {
                choice_type: "function".to_string(),
                function: ToolFunction {
                    name: allowed[0].to_string(),
                },
            }));
        }
        if allowed.len() > 1 {
            return Some(ToolChoice::String("required".to_string()));
        }
    }
    match mode {
        Some("NONE") => Some(ToolChoice::String("none".to_string())),
        Some("ANY") => Some(ToolChoice::String("required".to_string())),
        Some("AUTO") => Some(ToolChoice::String("auto".to_string())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_arguments_value_repairs_common_json() {
        assert_eq!(
            tool_arguments_value(r#"{"path":"a",}"#),
            json!({"path": "a"})
        );
        assert_eq!(
            tool_arguments_value(r#"{path:'README.md'}"#),
            json!({"path": "README.md"})
        );
        assert_eq!(
            tool_arguments_value(r#"{'path':'README.md',}"#),
            json!({"path": "README.md"})
        );
        assert_eq!(
            tool_arguments_value(
                r#"{
                    // keep path
                    "path": "a"
                }"#
            ),
            json!({"path": "a"})
        );
        assert_eq!(
            tool_arguments_value(
                r#"{
                    /* json5 block comment */
                    path: 'README.md',
                    flags: ['read', 'cache',],
                    nested: { dryRun: true },
                    count: +1,
                }"#
            ),
            json!({
                "path": "README.md",
                "flags": ["read", "cache"],
                "nested": {"dryRun": true},
                "count": 1
            })
        );
    }

    #[test]
    fn tool_arguments_value_preserves_unrepairable_arguments() {
        assert_eq!(
            tool_arguments_value(r#"{"path":"README.md""#),
            Value::String(r#"{"path":"README.md""#.to_string())
        );
    }
}
