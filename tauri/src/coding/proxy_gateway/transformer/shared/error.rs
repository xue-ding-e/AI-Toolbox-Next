use serde_json::Value;

pub(crate) fn extract_error_message(error: &Value) -> Option<String> {
    if let Some(message) = error.as_str().filter(|message| !message.trim().is_empty()) {
        return Some(message.to_string());
    }

    for pointer in [
        "/error/message",
        "/errors/message",
        "/message",
        "/detail",
        "/msg",
        "/status_msg",
        "/base_resp/status_msg",
    ] {
        if let Some(message) = error
            .pointer(pointer)
            .and_then(Value::as_str)
            .filter(|message| !message.trim().is_empty())
        {
            return Some(message.to_string());
        }
    }

    if let Some(message) = error
        .get("error")
        .and_then(Value::as_str)
        .filter(|message| !message.trim().is_empty())
    {
        return Some(message.to_string());
    }

    if !error.is_null() {
        return Some(error.to_string());
    }

    None
}

pub(crate) fn extract_error_type(error: &Value) -> Option<String> {
    extract_error_string(
        error,
        &[
            "/error/type",
            "/errors/type",
            "/error/status",
            "/errors/status",
            "/type",
            "/status",
        ],
    )
}

pub(crate) fn extract_error_code(error: &Value) -> Option<Value> {
    for pointer in ["/error/code", "/errors/code", "/code"] {
        if let Some(code) = error.pointer(pointer).filter(|code| !code.is_null()) {
            return Some(code.clone());
        }
    }
    None
}

pub(crate) fn extract_error_param(error: &Value) -> Option<Value> {
    for pointer in ["/error/param", "/errors/param", "/param"] {
        if let Some(param) = error.pointer(pointer).filter(|param| !param.is_null()) {
            return Some(param.clone());
        }
    }
    None
}

fn extract_error_string(error: &Value, pointers: &[&str]) -> Option<String> {
    for pointer in pointers {
        if let Some(value) = error
            .pointer(pointer)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            return Some(value.to_string());
        }
    }
    None
}
