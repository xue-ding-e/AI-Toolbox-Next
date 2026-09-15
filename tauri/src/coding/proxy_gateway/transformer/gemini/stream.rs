use super::convert::gemini_error_from_parts;
use serde_json::{json, Value};

pub(crate) fn gemini_stream_error(code: &str, message: &str) -> Value {
    let code_value = (!code.is_empty()).then(|| json!(code));
    let kind = (!code.is_empty()).then(|| code.to_string());
    gemini_error_from_parts(message.to_string(), kind, code_value)
}

/// Merge repeated snapshots only when a native call ID proves identity. A
/// function name or a chunk-local position cannot identify parallel calls.
pub(crate) fn merge_gemini_function_call_part(parts: &mut Vec<Value>, incoming: &Value) {
    let existing = incoming
        .pointer("/functionCall/id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .and_then(|id| {
            parts
                .iter_mut()
                .find(|part| part.pointer("/functionCall/id").and_then(Value::as_str) == Some(id))
        });
    let Some(existing) = existing else {
        parts.push(incoming.clone());
        return;
    };
    if let (Some(target), Some(source)) = (
        existing
            .get_mut("functionCall")
            .and_then(Value::as_object_mut),
        incoming.get("functionCall").and_then(Value::as_object),
    ) {
        for (key, value) in source {
            if key == "args" {
                if let (Some(target_args), Some(source_args)) = (
                    target.get_mut("args").and_then(Value::as_object_mut),
                    value.as_object(),
                ) {
                    target_args.extend(source_args.clone());
                    continue;
                }
            }
            target.insert(key.clone(), value.clone());
        }
    }
    if let (Some(target), Some(source)) = (existing.as_object_mut(), incoming.as_object()) {
        for (key, value) in source
            .iter()
            .filter(|(key, _)| key.as_str() != "functionCall")
        {
            target.insert(key.clone(), value.clone());
        }
    }
}
