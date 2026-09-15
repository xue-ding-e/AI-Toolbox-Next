mod convert;
mod inbound;
mod outbound;
mod stream;

pub(super) const SYNTHETIC_GEMINI_TOOL_ID_PREFIX: &str = "gemini_synth_";

pub(super) fn synthesize_gemini_tool_id() -> String {
    format!("{SYNTHETIC_GEMINI_TOOL_ID_PREFIX}{}", uuid::Uuid::new_v4())
}

pub(crate) use convert::{
    gemini_finish_to_openai_finish, gemini_usage_to_llm, llm_usage_to_gemini,
};
#[cfg(test)]
pub use convert::{
    gemini_request_to_llm, gemini_response_to_llm, llm_request_to_gemini, llm_response_to_gemini,
};
pub use inbound::GeminiInbound;
pub use outbound::GeminiOutbound;
pub(crate) use stream::{gemini_stream_error, merge_gemini_function_call_part};
