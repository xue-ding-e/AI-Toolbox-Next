use super::convert::gemini_error_from_parts;
use serde_json::{json, Value};

pub(crate) fn gemini_stream_error(code: &str, message: &str) -> Value {
    let code_value = (!code.is_empty()).then(|| json!(code));
    let kind = (!code.is_empty()).then(|| code.to_string());
    gemini_error_from_parts(message.to_string(), kind, code_value)
}
