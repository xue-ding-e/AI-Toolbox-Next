//! Reusable AI protocol conversion helpers for Proxy Gateway.
//!
//! The module is deliberately independent from database, Tauri commands and
//! provider storage. Runtime code supplies a source/target protocol and the
//! module only rewrites protocol payloads.

mod error;
mod json;
mod sse;
mod streaming;
mod types;

pub use error::ProtocolConversionError;
pub use json::{
    convert_error_response_body, convert_request_body, convert_request_value,
    convert_response_body, convert_response_value,
};
pub use streaming::convert_sse_stream;
pub use types::{AiProtocol, ConversionRoute};
