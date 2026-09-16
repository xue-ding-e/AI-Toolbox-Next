//! Keep instruction (`system` / `developer`) messages at a consistent prompt position.
//!
//! Claude Code appends a `role:"system"` reminder (for example
//! `<total_tokens>N tokens left</total_tokens>`) to the tail of the conversation
//! every turn. Merging *every* instruction message into the prompt head — the
//! historical third-party Chat compat behavior — therefore rewrote the head on
//! every turn and broke upstream prefix caching (issue #356: `cache_read_tokens`
//! stuck at ~4K, hit rate 7.7% while the same upstream/model through the Codex
//! route reached 98.4%).
//!
//! The merge is still the right behavior for the other direction: Codex (OpenAI
//! Responses) puts its identity/instruction blocks in `developer` items, which are
//! stable across turns, so merging them into the head is cache-safe and keeps the
//! single leading `system` that strict upstreams require. cc-switch splits exactly
//! this way:
//! - Anthropic -> Chat (`src-tauri/src/proxy/providers/transform.rs`) keeps
//!   mid-conversation `system` messages at their original index. That is a *revert*
//!   of its own hoisting (`b724f5dd revert(proxy): drop Anthropic system-message
//!   hoisting (#3775)`); its release notes report hit rate 99% -> 20% before the
//!   revert.
//! - Responses -> Chat (`transform_codex_chat.rs`, `collapse_system_messages_to_head`)
//!   merges every system/developer message to the head unconditionally, including
//!   Codex's mid-stream `developer` instructions.
//! AxonHub reaches the same split from the other side: its Chat outbound is 1:1 and
//! its `llm/pipeline/cc/system_messages.go` middleware downgrades every instruction
//! message after the leading run to `user` for Claude Code clients, while its
//! Anthropic/Responses outbounds keep merging for everyone else.
//!
//! AI Toolbox therefore gates on the conversion source: Anthropic Messages (whose
//! canonical client, Claude Code, is the one that appends per-turn `system`
//! annotations) preserves instruction order, everything else keeps the historical
//! merge-everything-to-head behavior.

use super::super::llm::{ApiFormat, Message};
use super::super::types::AiProtocol;
use serde_json::{json, Value};

/// Placement of instruction messages that sit after the leading contiguous run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstructionPlacement {
    /// Merge every instruction message into the target's single instruction slot.
    /// Historical third-party Chat compat behavior; safe while the source client
    /// keeps its instruction content stable across turns.
    MergeToHead,
    /// Merge only the leading contiguous run. Later instruction messages keep their
    /// index with the role downgraded to `user`, so a per-turn reminder cannot
    /// rewrite the prompt head.
    PreserveOrder,
}

impl InstructionPlacement {
    /// Anthropic Messages is the only supported inbound wire whose canonical client
    /// (Claude Code) annotates the conversation with per-turn `system` messages
    /// mid-stream; those must not be hoisted.
    fn for_source(source_is_anthropic_messages: bool) -> Self {
        if source_is_anthropic_messages {
            Self::PreserveOrder
        } else {
            Self::MergeToHead
        }
    }
}

pub(crate) fn placement_for_api_format(api_format: Option<ApiFormat>) -> InstructionPlacement {
    InstructionPlacement::for_source(api_format == Some(ApiFormat::AnthropicMessages))
}

pub(crate) fn placement_for_protocol(source: AiProtocol) -> InstructionPlacement {
    InstructionPlacement::for_source(source == AiProtocol::AnthropicMessages)
}

/// Instruction roles that the compatible head merge owns.
pub(crate) fn is_instruction_role(role: &str) -> bool {
    role == "system" || role == "developer"
}

/// Length of the leading contiguous instruction run.
pub(crate) fn leading_instruction_run_len(messages: &[Message]) -> usize {
    messages
        .iter()
        .take_while(|message| is_instruction_role(&message.role))
        .count()
}

/// Per-message flag telling the IR-level writers whether that message must be
/// merged into the target's instruction slot instead of being written in place.
pub(crate) fn instruction_hoist_plan(
    messages: &[Message],
    placement: InstructionPlacement,
) -> Vec<bool> {
    let leading = leading_instruction_run_len(messages);
    messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            is_instruction_role(&message.role)
                && (placement == InstructionPlacement::MergeToHead || index < leading)
        })
        .collect()
}

/// Text of a Chat `system` message for the head merge, covering both the plain
/// string content and the text-parts array form.
fn chat_system_message_text(message: &Value) -> Option<String> {
    match message.get("content")? {
        Value::String(text) => Some(text.clone()),
        Value::Array(parts) => {
            let text = parts
                .iter()
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    }
}

/// Rewrites OpenAI Chat `messages[]` so instruction content sits at a consistent
/// position.
///
/// Hoistable instruction messages with mergeable text become the single head
/// `system` message (`"\n\n"` joined); ones without text content stay where they
/// are. Under [`InstructionPlacement::PreserveOrder`], instruction messages after
/// the leading run keep their index and content with the role downgraded to
/// `user`.
pub(crate) fn normalize_chat_system_messages(
    messages: Vec<Value>,
    placement: InstructionPlacement,
) -> Vec<Value> {
    let mut system_chunks = Vec::new();
    let mut rest = Vec::with_capacity(messages.len());
    let mut in_leading_run = true;

    for mut message in messages {
        let role = message.get("role").and_then(Value::as_str);
        let is_instruction = role.is_some_and(is_instruction_role);

        if !is_instruction {
            in_leading_run = false;
            rest.push(message);
            continue;
        }

        let hoist = placement == InstructionPlacement::MergeToHead || in_leading_run;
        if hoist {
            match chat_system_message_text(&message) {
                Some(text) => {
                    if !text.trim().is_empty() {
                        system_chunks.push(text);
                    }
                    // Whitespace-only instruction content has nothing to merge and
                    // is dropped with its message, as the merge always did.
                }
                None => rest.push(message),
            }
            continue;
        }

        if let Value::Object(object) = &mut message {
            object.insert("role".to_string(), Value::String("user".to_string()));
        }
        rest.push(message);
    }

    if system_chunks.is_empty() {
        return rest;
    }

    let mut normalized = Vec::with_capacity(rest.len() + 1);
    normalized.push(json!({
        "role": "system",
        "content": system_chunks.join("\n\n")
    }));
    normalized.extend(rest);
    normalized
}

/// Downgrades an instruction message that is written in place (that is, not
/// hoisted) to `user`, keeping content untouched.
pub(crate) fn downgrade_instruction_message(message: Message) -> Message {
    if is_instruction_role(&message.role) {
        Message {
            role: "user".to_string(),
            ..message
        }
    } else {
        message
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::llm::MessageContent;
    use super::*;

    fn text_message(role: &str, content: &str) -> Message {
        Message {
            role: role.to_string(),
            content: MessageContent::Text(content.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn placement_is_preserving_only_for_anthropic_sources() {
        assert_eq!(
            placement_for_api_format(Some(ApiFormat::AnthropicMessages)),
            InstructionPlacement::PreserveOrder
        );
        assert_eq!(
            placement_for_protocol(AiProtocol::AnthropicMessages),
            InstructionPlacement::PreserveOrder
        );
        for api_format in [
            Some(ApiFormat::OpenAiResponses),
            Some(ApiFormat::OpenAiChatCompletions),
            Some(ApiFormat::OpenAiResponsesCompact),
            Some(ApiFormat::GeminiContents),
            None,
        ] {
            assert_eq!(
                placement_for_api_format(api_format),
                InstructionPlacement::MergeToHead
            );
        }
        for protocol in [
            AiProtocol::OpenAiResponses,
            AiProtocol::OpenAiChat,
            AiProtocol::GeminiNative,
        ] {
            assert_eq!(
                placement_for_protocol(protocol),
                InstructionPlacement::MergeToHead
            );
        }
    }

    #[test]
    fn leading_run_is_only_the_contiguous_head_block() {
        let messages = vec![
            text_message("system", "prompt"),
            text_message("developer", "dev"),
            text_message("user", "hi"),
            text_message("system", "late"),
        ];
        assert_eq!(leading_instruction_run_len(&messages), 2);

        let messages = vec![text_message("user", "hi"), text_message("system", "late")];
        assert_eq!(leading_instruction_run_len(&messages), 0);
    }

    #[test]
    fn hoist_plan_merges_everything_for_codex_and_only_the_head_for_claude_code() {
        let messages = vec![
            text_message("developer", "identity"),
            text_message("user", "hi"),
            text_message("developer", "collaboration mode"),
        ];

        assert_eq!(
            instruction_hoist_plan(&messages, InstructionPlacement::MergeToHead),
            vec![true, false, true]
        );
        assert_eq!(
            instruction_hoist_plan(&messages, InstructionPlacement::PreserveOrder),
            vec![true, false, false]
        );
    }

    #[test]
    fn chat_normalize_preserving_merges_head_and_keeps_late_system_in_place() {
        let messages = vec![
            json!({"role": "system", "content": "You are Claude Code."}),
            json!({"role": "user", "content": "Hello"}),
            json!({"role": "assistant", "content": "Hi there!"}),
            json!({
                "role": "system",
                "content": "<total_tokens>14963538 tokens left</total_tokens>"
            }),
            json!({"role": "user", "content": "Continue"}),
        ];

        let normalized =
            normalize_chat_system_messages(messages, InstructionPlacement::PreserveOrder);
        assert_eq!(normalized.len(), 5);
        assert_eq!(normalized[0]["role"], "system");
        assert_eq!(normalized[0]["content"], "You are Claude Code.");
        // Downgraded in place: same index, same bytes, role only.
        assert_eq!(normalized[3]["role"], "user");
        assert_eq!(
            normalized[3]["content"],
            "<total_tokens>14963538 tokens left</total_tokens>"
        );
    }

    #[test]
    fn chat_normalize_merging_hoists_late_developer_and_system_into_head() {
        // Codex direction (issue #356's control case): the late developer block is
        // stable instruction content, so merging it into the head is cache-safe and
        // keeps a single leading system for strict upstreams.
        let messages = vec![
            json!({"role": "system", "content": "You are Codex."}),
            json!({"role": "user", "content": "hi"}),
            json!({"role": "developer", "content": "Collaboration Mode: Default"}),
            json!({"role": "user", "content": "more"}),
        ];

        let normalized =
            normalize_chat_system_messages(messages, InstructionPlacement::MergeToHead);
        assert_eq!(normalized.len(), 3);
        assert_eq!(normalized[0]["role"], "system");
        assert_eq!(
            normalized[0]["content"],
            "You are Codex.\n\nCollaboration Mode: Default"
        );
        assert!(normalized
            .iter()
            .all(|message| message["role"].as_str() != Some("developer")));
    }

    #[test]
    fn chat_normalize_head_stays_stable_when_a_tail_reminder_is_appended() {
        let base = vec![
            json!({"role": "system", "content": "You are Claude Code."}),
            json!({"role": "user", "content": "Hello"}),
        ];
        let next_turn = vec![
            json!({"role": "system", "content": "You are Claude Code."}),
            json!({"role": "user", "content": "Hello"}),
            json!({"role": "assistant", "content": "Hi there!"}),
            json!({"role": "system", "content": "<total_tokens>999 tokens left</total_tokens>"}),
        ];

        let placement = InstructionPlacement::PreserveOrder;
        assert_eq!(
            normalize_chat_system_messages(base, placement)[0],
            normalize_chat_system_messages(next_turn, placement)[0]
        );
    }

    #[test]
    fn chat_normalize_merges_leading_developer_run_into_one_head_system() {
        let messages = vec![
            json!({"role": "developer", "content": "developer instructions"}),
            json!({"role": "developer", "content": "uppercased developer"}),
            json!({"role": "user", "content": "hi"}),
        ];

        for placement in [
            InstructionPlacement::MergeToHead,
            InstructionPlacement::PreserveOrder,
        ] {
            let normalized = normalize_chat_system_messages(messages.clone(), placement);
            assert_eq!(normalized.len(), 2);
            assert_eq!(normalized[0]["role"], "system");
            assert_eq!(
                normalized[0]["content"],
                "developer instructions\n\nuppercased developer"
            );
            assert_eq!(normalized[1]["role"], "user");
        }
    }

    #[test]
    fn chat_normalize_keeps_a_body_without_leading_system_unhoisted() {
        let messages = vec![
            json!({"role": "user", "content": "hi"}),
            json!({"role": "system", "content": "late"}),
        ];

        let normalized =
            normalize_chat_system_messages(messages, InstructionPlacement::PreserveOrder);
        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0]["role"], "user");
        assert_eq!(normalized[1]["role"], "user");
        assert_eq!(normalized[1]["content"], "late");
    }

    #[test]
    fn downgrade_instruction_message_only_touches_instruction_roles() {
        let downgraded = downgrade_instruction_message(text_message("system", "late"));
        assert_eq!(downgraded.role, "user");
        assert_eq!(downgraded.content, MessageContent::Text("late".to_string()));

        let untouched = downgrade_instruction_message(text_message("assistant", "hi"));
        assert_eq!(untouched.role, "assistant");
    }
}
