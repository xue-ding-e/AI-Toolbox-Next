use serde_json::{json, Value};

pub(super) fn inject_cache_control(body: &mut Value) -> bool {
    let mut changed = false;

    if body.get("system").and_then(Value::as_str).is_some() {
        let text = body["system"].as_str().unwrap_or_default().to_string();
        body["system"] = json!([{ "type": "text", "text": text }]);
        changed = true;
    }

    if let Some(system) = body.get_mut("system").and_then(Value::as_array_mut) {
        if let Some(last_block) = system.last_mut() {
            changed |= inject_block(last_block);
        }
    }

    if let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) {
        if let Some(message) = messages.iter_mut().rev().find(|message| {
            message
                .get("role")
                .and_then(Value::as_str)
                .is_some_and(|role| role == "user")
        }) {
            if let Some(content) = message.get_mut("content").and_then(Value::as_array_mut) {
                if let Some(block) = content.iter_mut().rev().find(|block| {
                    block
                        .get("type")
                        .and_then(Value::as_str)
                        .is_none_or(|block_type| {
                            !matches!(block_type, "thinking" | "redacted_thinking")
                        })
                }) {
                    changed |= inject_block(block);
                }
            }
        }
    }

    changed
}

fn inject_block(block: &mut Value) -> bool {
    let Some(object) = block.as_object_mut() else {
        return false;
    };
    if object.contains_key("cache_control") {
        return false;
    }
    object.insert("cache_control".to_string(), json!({ "type": "ephemeral" }));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn injects_system_string_as_last_block() {
        let mut body = json!({
            "system": "You are helpful",
            "messages": [{"role": "user", "content": "hi"}]
        });

        assert!(inject_cache_control(&mut body));
        assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn injects_last_user_content_block() {
        let mut body = json!({
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "first"},
                    {"type": "text", "text": "last"}
                ]
            }]
        });

        assert!(inject_cache_control(&mut body));
        assert!(body["messages"][0]["content"][1]
            .get("cache_control")
            .is_some());
        assert!(body["messages"][0]["content"][0]
            .get("cache_control")
            .is_none());
    }
}
