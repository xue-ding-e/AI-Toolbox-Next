pub mod error;
pub mod lossy;
pub mod messages;
pub mod signature;
pub mod thinking_config;

use super::llm::ApiFormat;

pub(crate) use error::{
    extract_error_code, extract_error_message, extract_error_param, extract_error_type,
};
pub use messages::{
    content_text, extract_reasoning_field_text, json_string, message_parts,
    split_leading_think_block, stop_from_value, stop_to_value, strip_leading_think_open_tag,
    tool_arguments_value, tool_choice_from_anthropic, tool_choice_from_gemini,
    tool_choice_from_openai, tool_choice_to_anthropic, tool_choice_to_openai,
    tool_choice_to_responses,
};
pub use thinking_config::{budget_tokens_to_reasoning_effort, reasoning_effort_to_budget_tokens};

pub(crate) fn should_emit_openai_request_metadata(api_format: Option<ApiFormat>) -> bool {
    matches!(
        api_format,
        Some(
            ApiFormat::OpenAiChatCompletions
                | ApiFormat::OpenAiResponses
                | ApiFormat::OpenAiResponsesCompact
        )
    )
}
