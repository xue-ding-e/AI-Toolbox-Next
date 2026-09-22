use serde_json::{json, Value};

const MAX_CACHE_CONTROL_BREAKPOINTS: usize = 4;
const ADAPTIVE_CACHE_CONTROL_BLOCK_WINDOW: usize = 20;

pub(super) fn inject_cache_control(body: &mut Value) -> bool {
    let original = body.clone();

    normalize_message_contents(body);
    sanitize_unsupported_cache_controls(body);
    trim_cache_controls_to_limit(body, MAX_CACHE_CONTROL_BREAKPOINTS);

    let mut remaining = remaining_cache_control_budget(body);
    if remaining > 0 {
        // Structural anchors are only added when the client has not placed one
        // on tools/system already, and never beyond the Anthropic limit.
        remaining = ensure_structural_cache_controls(body, remaining);

        // A client that planned its own message breakpoints keeps them: its
        // anchors are prefix-stable, while our distance-from-end window moves
        // with every turn and changes the serialized prefix hash, which makes
        // every request pay a full cache write (AxonHub 7444f537).
        if count_message_breakpoints(body) == 0 {
            let refs = collect_cacheable_message_block_refs(body);
            let message_anchors = desired_message_cache_anchors(refs.len()).min(remaining);
            inject_planned_message_cache_controls(body, &refs, message_anchors);
        }
    }

    *body != original
}

fn remaining_cache_control_budget(body: &Value) -> usize {
    MAX_CACHE_CONTROL_BREAKPOINTS.saturating_sub(count_cache_controls(body))
}

fn count_message_breakpoints(body: &Value) -> usize {
    let Some(messages) = body.get("messages").and_then(Value::as_array) else {
        return 0;
    };
    messages
        .iter()
        .filter_map(|message| message.get("content").and_then(Value::as_array))
        .flatten()
        .filter(|block| block.get("cache_control").is_some())
        .count()
}

/// Trims client breakpoints down to Anthropic limit, dropping the earliest
/// message breakpoint first and only then the earliest structural one.
fn trim_cache_controls_to_limit(body: &mut Value, limit: usize) {
    while count_cache_controls(body) > limit {
        if !remove_earliest_message_breakpoint(body) && !remove_earliest_structural_breakpoint(body)
        {
            return;
        }
    }
}

fn remove_earliest_message_breakpoint(body: &mut Value) -> bool {
    let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) else {
        return false;
    };
    for message in messages {
        let Some(content) = message.get_mut("content").and_then(Value::as_array_mut) else {
            continue;
        };
        for block in content {
            if block.get("cache_control").is_some() {
                remove_cache_control(block);
                return true;
            }
        }
    }
    false
}

fn remove_earliest_structural_breakpoint(body: &mut Value) -> bool {
    if let Some(tools) = body.get_mut("tools").and_then(Value::as_array_mut) {
        for tool in tools {
            if tool.get("cache_control").is_some() {
                remove_cache_control(tool);
                return true;
            }
        }
    }

    if let Some(system) = body.get_mut("system").and_then(Value::as_array_mut) {
        for block in system {
            if block.get("cache_control").is_some() {
                remove_cache_control(block);
                return true;
            }
        }
    }

    false
}

fn normalize_message_contents(body: &mut Value) {
    let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) else {
        return;
    };
    for message in messages {
        let Some(content) = message.get_mut("content") else {
            continue;
        };
        let Some(text) = content.as_str().filter(|text| !text.is_empty()) else {
            continue;
        };
        *content = json!([{ "type": "text", "text": text }]);
    }
}

fn remove_cache_control(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.remove("cache_control");
    }
}

/// Injects the tools/system anchors the client did not provide and returns the
/// breakpoint budget left for message anchors. The budget is never exceeded:
/// Anthropic rejects more than four breakpoints per request.
fn ensure_structural_cache_controls(body: &mut Value, budget: usize) -> usize {
    let mut remaining = budget;

    if let Some(tools) = body.get_mut("tools").and_then(Value::as_array_mut) {
        if remaining > 0 && !tools.iter().any(|tool| tool.get("cache_control").is_some()) {
            if let Some(last_tool) = tools.last_mut() {
                if inject_block(last_tool) {
                    remaining -= 1;
                }
            }
        }
    }

    if remaining == 0 {
        return remaining;
    }

    // A string system prompt must become a block array before an anchor can be
    // attached to its last block.
    if body.get("system").and_then(Value::as_str).is_some() {
        let text = body["system"].as_str().unwrap_or_default().to_string();
        if !text.is_empty() {
            body["system"] = json!([{ "type": "text", "text": text }]);
        }
    }

    if let Some(system) = body.get_mut("system").and_then(Value::as_array_mut) {
        if !system
            .iter()
            .any(|block| block.get("cache_control").is_some())
        {
            if let Some(last_block) = system.last_mut() {
                if inject_block(last_block) {
                    remaining -= 1;
                }
            }
        }
    }

    remaining
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MessageBlockRef {
    message_index: usize,
    block_index: usize,
}

fn collect_cacheable_message_block_refs(body: &Value) -> Vec<MessageBlockRef> {
    let Some(messages) = body.get("messages").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut refs = Vec::new();
    for (message_index, message) in messages.iter().enumerate() {
        let Some(content) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for (block_index, block) in content.iter().enumerate() {
            if is_cacheable_message_block(block) {
                refs.push(MessageBlockRef {
                    message_index,
                    block_index,
                });
            }
        }
    }
    refs
}

fn desired_message_cache_anchors(cacheable_blocks: usize) -> usize {
    if cacheable_blocks == 0 {
        0
    } else if cacheable_blocks >= ADAPTIVE_CACHE_CONTROL_BLOCK_WINDOW {
        2
    } else {
        1
    }
}

fn inject_planned_message_cache_controls(
    body: &mut Value,
    refs: &[MessageBlockRef],
    target: usize,
) {
    if refs.is_empty() || target == 0 {
        return;
    }

    inject_message_ref(body, refs[refs.len() - 1]);

    if target > 1 {
        let index = pick_window_anchor_index(refs.len(), ADAPTIVE_CACHE_CONTROL_BLOCK_WINDOW);
        if let Some(index) = index {
            inject_message_ref(body, refs[index]);
        }
    }
}

fn pick_window_anchor_index(len: usize, window: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(len.saturating_sub(1 + window))
}

fn inject_message_ref(body: &mut Value, block_ref: MessageBlockRef) {
    let Some(block) = body
        .get_mut("messages")
        .and_then(Value::as_array_mut)
        .and_then(|messages| messages.get_mut(block_ref.message_index))
        .and_then(|message| message.get_mut("content"))
        .and_then(Value::as_array_mut)
        .and_then(|content| content.get_mut(block_ref.block_index))
    else {
        return;
    };
    inject_block(block);
}

fn sanitize_unsupported_cache_controls(body: &mut Value) {
    let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) else {
        return;
    };
    for message in messages {
        let Some(content) = message.get_mut("content").and_then(Value::as_array_mut) else {
            continue;
        };
        for block in content {
            if !is_cacheable_message_block(block) {
                remove_cache_control(block);
            }
        }
    }
}

fn is_cacheable_message_block(block: &Value) -> bool {
    match block.get("type").and_then(Value::as_str) {
        Some("thinking" | "redacted_thinking") => false,
        Some("text") => block
            .get("text")
            .and_then(Value::as_str)
            .is_some_and(|text| !text.is_empty()),
        _ => true,
    }
}

fn inject_block(block: &mut Value) -> bool {
    let Some(object) = block.as_object_mut() else {
        return false;
    };
    object.insert("cache_control".to_string(), json!({ "type": "ephemeral" }));
    true
}

fn count_cache_controls(value: &Value) -> usize {
    let mut count = 0;
    if let Some(tools) = value.get("tools").and_then(Value::as_array) {
        count += tools
            .iter()
            .filter(|tool| tool.get("cache_control").is_some())
            .count();
    }
    if let Some(system) = value.get("system").and_then(Value::as_array) {
        count += system
            .iter()
            .filter(|block| block.get("cache_control").is_some())
            .count();
    }
    if let Some(messages) = value.get("messages").and_then(Value::as_array) {
        for message in messages {
            let Some(content) = message.get("content").and_then(Value::as_array) else {
                continue;
            };
            count += content
                .iter()
                .filter(|block| block.get("cache_control").is_some())
                .count();
        }
    }
    count
}

#[cfg(test)]
fn message_block_has_cache_control(
    value: &Value,
    message_index: usize,
    block_index: usize,
) -> bool {
    value
        .pointer(&format!(
            "/messages/{message_index}/content/{block_index}/cache_control"
        ))
        .is_some()
}

#[cfg(test)]
fn tool_has_cache_control(value: &Value, tool_index: usize) -> bool {
    value
        .pointer(&format!("/tools/{tool_index}/cache_control"))
        .is_some()
}

#[cfg(test)]
fn system_has_cache_control(value: &Value, block_index: usize) -> bool {
    value
        .pointer(&format!("/system/{block_index}/cache_control"))
        .is_some()
}

#[cfg(test)]
fn text_block(index: usize) -> Value {
    json!({"type":"text","text":format!("block {index}")})
}

#[cfg(test)]
fn text_blocks(count: usize) -> Vec<Value> {
    (0..count).map(text_block).collect()
}

#[cfg(test)]
fn stale_cached_text_block(text: &str) -> Value {
    json!({"type":"text","text":text,"cache_control":{"type":"ephemeral"}})
}

#[cfg(test)]
fn stale_cached_thinking_block() -> Value {
    json!({"type":"thinking","thinking":"old","cache_control":{"type":"ephemeral"}})
}

#[cfg(test)]
fn stale_cached_empty_text_block() -> Value {
    json!({"type":"text","text":"","cache_control":{"type":"ephemeral"}})
}

#[cfg(test)]
fn stale_cached_tool() -> Value {
    json!({
        "name": "old_tool",
        "input_schema": {"type":"object"},
        "cache_control": {"type":"ephemeral"}
    })
}

#[cfg(test)]
fn tool(name: &str) -> Value {
    json!({"name":name,"input_schema":{"type":"object"}})
}

#[cfg(test)]
fn stale_cached_system_block(text: &str) -> Value {
    json!({"type":"text","text":text,"cache_control":{"type":"ephemeral"}})
}

#[cfg(test)]
fn assert_ephemeral(value: &Value) {
    assert_eq!(value.get("type").and_then(Value::as_str), Some("ephemeral"));
}

#[cfg(test)]
fn cache_control_at<'a>(value: &'a Value, pointer: &str) -> &'a Value {
    value.pointer(pointer).expect("cache_control should exist")
}

#[cfg(test)]
fn assert_no_cache_control_at(value: &Value, pointer: &str) {
    assert!(
        value.pointer(pointer).is_none(),
        "{pointer} should not have cache_control"
    );
}

#[cfg(test)]
fn assert_cache_control_count(value: &Value, expected: usize) {
    assert_eq!(count_cache_controls(value), expected);
}

#[cfg(test)]
fn assert_no_unsupported_cache_controls(value: &Value) {
    let Some(messages) = value.get("messages").and_then(Value::as_array) else {
        return;
    };
    for message in messages {
        let Some(content) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for block in content {
            if !is_cacheable_message_block(block) {
                assert!(block.get("cache_control").is_none());
            }
        }
    }
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
        assert_ephemeral(cache_control_at(&body, "/system/0/cache_control"));
        assert_ephemeral(cache_control_at(
            &body,
            "/messages/0/content/0/cache_control",
        ));
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
        assert!(message_block_has_cache_control(&body, 0, 1));
        assert!(!message_block_has_cache_control(&body, 0, 0));
    }

    #[test]
    fn preserves_client_structural_breakpoints_and_fills_message_anchors() {
        let mut body = json!({
            "tools": [
                stale_cached_tool(),
                tool("new_tool")
            ],
            "system": [
                stale_cached_system_block("old"),
                {"type":"text","text":"current"}
            ],
            "messages": [{
                "role":"user",
                "content": text_blocks(25)
            }]
        });

        assert!(inject_cache_control(&mut body));

        // Client anchors stay where they were; the planner only adds the
        // missing message anchors within the remaining budget.
        assert_cache_control_count(&body, 4);
        assert!(tool_has_cache_control(&body, 0));
        assert!(!tool_has_cache_control(&body, 1));
        assert!(system_has_cache_control(&body, 0));
        assert!(!system_has_cache_control(&body, 1));
        assert!(message_block_has_cache_control(&body, 0, 4));
        assert!(message_block_has_cache_control(&body, 0, 24));
    }

    #[test]
    fn keeps_client_message_breakpoints_without_replanning() {
        let mut body = json!({
            "messages": [
                {
                    "role":"user",
                    "content": [{"type":"text","text":"first","cache_control":{"type":"ephemeral"}}]
                },
                {
                    "role":"user",
                    "content": text_blocks(25)
                }
            ]
        });

        // Nothing to do: the client already planned message breakpoints, so no
        // distance-from-end anchor is added (AxonHub 7444f537) and the body
        // stays byte-identical upstream.
        assert!(!inject_cache_control(&mut body));
        assert_cache_control_count(&body, 1);
        assert!(message_block_has_cache_control(&body, 0, 0));
        for block_index in 0..25 {
            assert!(!message_block_has_cache_control(&body, 1, block_index));
        }
    }

    #[test]
    fn keeps_structural_anchors_within_breakpoint_limit() {
        let mut body = json!({
            "tools": [tool("unmarked")],
            "system": [{"type":"text","text":"unmarked system"}],
            "messages": [{
                "role":"user",
                "content": [
                    {"type":"text","text":"client-1","cache_control":{"type":"ephemeral"}},
                    {"type":"text","text":"client-2","cache_control":{"type":"ephemeral"}},
                    {"type":"text","text":"client-3","cache_control":{"type":"ephemeral"}}
                ]
            }]
        });

        assert!(inject_cache_control(&mut body));

        // Three client breakpoints leave a single free slot: tools is filled
        // first and the unmarked system block stays unmarked instead of
        // pushing the request over Anthropic's four-breakpoint limit.
        assert_cache_control_count(&body, 4);
        assert!(tool_has_cache_control(&body, 0));
        assert!(!system_has_cache_control(&body, 0));
        assert!(message_block_has_cache_control(&body, 0, 0));
        assert!(message_block_has_cache_control(&body, 0, 1));
        assert!(message_block_has_cache_control(&body, 0, 2));
    }

    #[test]
    fn keeps_string_system_untouched_when_budget_is_spent() {
        let mut body = json!({
            "system": "You are helpful",
            "messages": [{
                "role":"user",
                "content": [
                    {"type":"text","text":"client-1","cache_control":{"type":"ephemeral"}},
                    {"type":"text","text":"client-2","cache_control":{"type":"ephemeral"}},
                    {"type":"text","text":"client-3","cache_control":{"type":"ephemeral"}},
                    {"type":"text","text":"client-4","cache_control":{"type":"ephemeral"}}
                ]
            }]
        });

        // The client already spent the whole budget, so normalizing the string
        // system prompt would only churn the upstream bytes.
        assert!(!inject_cache_control(&mut body));
        assert_eq!(body["system"], json!("You are helpful"));
        assert_cache_control_count(&body, 4);
    }
    #[test]
    fn trims_excess_client_breakpoints_deterministically() {
        let mut body = json!({
            "tools": [stale_cached_tool()],
            "system": [stale_cached_system_block("kept")],
            "messages": [{
                "role":"user",
                "content": [
                    {"type":"text","text":"dropped","cache_control":{"type":"ephemeral"}},
                    {"type":"text","text":"kept-1","cache_control":{"type":"ephemeral"}},
                    {"type":"text","text":"kept-2","cache_control":{"type":"ephemeral"}}
                ]
            }]
        });

        assert!(inject_cache_control(&mut body));

        // Anthropic allows four breakpoints: the earliest message breakpoint is
        // dropped first, structural anchors and later message anchors survive.
        assert_cache_control_count(&body, 4);
        assert!(tool_has_cache_control(&body, 0));
        assert!(system_has_cache_control(&body, 0));
        assert!(!message_block_has_cache_control(&body, 0, 0));
        assert!(message_block_has_cache_control(&body, 0, 1));
        assert!(message_block_has_cache_control(&body, 0, 2));
    }

    #[test]
    fn does_not_cache_thinking_or_empty_text_blocks() {
        let mut body = json!({
            "messages": [{
                "role":"user",
                "content": [
                    stale_cached_thinking_block(),
                    stale_cached_empty_text_block(),
                    stale_cached_text_block("usable")
                ]
            }]
        });

        assert!(inject_cache_control(&mut body));

        assert_cache_control_count(&body, 1);
        assert_no_unsupported_cache_controls(&body);
        assert_no_cache_control_at(&body, "/messages/0/content/0/cache_control");
        assert_no_cache_control_at(&body, "/messages/0/content/1/cache_control");
        assert!(message_block_has_cache_control(&body, 0, 2));
    }
}
