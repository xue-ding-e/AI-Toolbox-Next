use super::super::error::ProtocolConversionError;
use super::super::llm::{Request, Response};
use super::super::traits::OutboundTransformer;
use super::super::types::AiProtocol;
use super::convert::{gemini_response_to_llm, llm_request_to_gemini};
use serde_json::Value;

pub struct GeminiOutbound;

impl OutboundTransformer for GeminiOutbound {
    fn protocol(&self) -> AiProtocol {
        AiProtocol::GeminiNative
    }

    fn request_from_llm(&self, request: Request) -> Result<Value, ProtocolConversionError> {
        Ok(llm_request_to_gemini(request))
    }

    fn response_to_llm(&self, body: Value) -> Result<Response, ProtocolConversionError> {
        Ok(gemini_response_to_llm(body))
    }

    fn error_to_llm(&self, error: Value) -> Value {
        error
    }
}
