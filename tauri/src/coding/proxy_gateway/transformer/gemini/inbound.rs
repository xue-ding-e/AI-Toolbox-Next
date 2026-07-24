use super::super::error::ProtocolConversionError;
use super::super::llm::{Request, Response};
use super::super::traits::InboundTransformer;
use super::super::types::AiProtocol;
use super::convert::{gemini_error, gemini_request_to_llm, llm_response_to_gemini};
use serde_json::Value;

pub struct GeminiInbound;

impl InboundTransformer for GeminiInbound {
    fn protocol(&self) -> AiProtocol {
        AiProtocol::GeminiNative
    }

    fn request_to_llm(&self, body: Value) -> Result<Request, ProtocolConversionError> {
        Ok(gemini_request_to_llm(body))
    }

    fn response_from_llm(&self, response: Response) -> Result<Value, ProtocolConversionError> {
        Ok(llm_response_to_gemini(response))
    }

    fn error_from_llm(&self, error: Value) -> Value {
        gemini_error(error)
    }
}
