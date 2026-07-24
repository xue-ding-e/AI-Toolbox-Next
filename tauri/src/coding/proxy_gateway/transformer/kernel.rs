use super::anthropic::{AnthropicInbound, AnthropicOutbound};
use super::error::ProtocolConversionError;
use super::gemini::{GeminiInbound, GeminiOutbound};
use super::openai::chat::{OpenAiChatInbound, OpenAiChatOutbound};
use super::openai::codex_tools::{
    apply_codex_tool_context_to_chat_request, build_codex_tool_context_from_request,
    rewrite_response_with_codex_tool_context, rewrite_responses_request_for_chat_context,
    CodexToolContext,
};
use super::openai::responses::{
    llm_request_to_responses_compact, llm_response_to_responses_compact,
    responses_compact_request_to_llm, responses_compact_response_to_llm, OpenAiResponsesInbound,
    OpenAiResponsesOutbound,
};
use super::stream::StreamKernel;
use super::traits::{InboundTransformer, OutboundTransformer};
use super::types::{AiProtocol, ConversionRoute};
use futures_util::{stream, Stream, StreamExt};
use serde_json::Value;
use std::collections::VecDeque;
use std::pin::Pin;

pub type ConversionByteStream =
    Pin<Box<dyn Stream<Item = Result<Vec<u8>, String>> + Send + 'static>>;

#[derive(Debug, Clone, Default)]
pub struct ConversionContext {
    pub codex_tool_context: Option<CodexToolContext>,
}

impl ConversionContext {
    pub fn is_empty(&self) -> bool {
        self.codex_tool_context
            .as_ref()
            .map(CodexToolContext::is_empty)
            .unwrap_or(true)
    }
}

pub fn convert_request_body(
    route: ConversionRoute,
    body: &[u8],
) -> Result<Vec<u8>, ProtocolConversionError> {
    if route.identity() {
        return Ok(body.to_vec());
    }
    let value = serde_json::from_slice::<Value>(body)
        .map_err(|error| ProtocolConversionError::InvalidJson(error.to_string()))?;
    let converted = convert_request_value(route, value)?;
    serde_json::to_vec(&converted)
        .map_err(|error| ProtocolConversionError::Transform(error.to_string()))
}

pub struct ConvertedRequestBody {
    pub body: Vec<u8>,
    pub context: ConversionContext,
}

pub fn convert_responses_compact_request_body_to_target(
    target: AiProtocol,
    body: &[u8],
) -> Result<ConvertedRequestBody, ProtocolConversionError> {
    let value = serde_json::from_slice::<Value>(body)
        .map_err(|error| ProtocolConversionError::InvalidJson(error.to_string()))?;
    let (converted, context) = convert_responses_compact_request_value_to_target(target, value)?;
    let body = serde_json::to_vec(&converted)
        .map_err(|error| ProtocolConversionError::Transform(error.to_string()))?;
    Ok(ConvertedRequestBody { body, context })
}

pub fn convert_responses_compact_request_value_to_target(
    target: AiProtocol,
    value: Value,
) -> Result<(Value, ConversionContext), ProtocolConversionError> {
    if target == AiProtocol::OpenAiResponses {
        let request = responses_compact_request_to_llm(value);
        return Ok((
            llm_request_to_responses_compact(request),
            ConversionContext::default(),
        ));
    }

    if target == AiProtocol::OpenAiChat {
        let codex_tool_context = build_codex_tool_context_from_request(&value);
        let request_for_chat =
            rewrite_responses_request_for_chat_context(value.clone(), &codex_tool_context);
        let request = responses_compact_request_to_llm(request_for_chat);
        let mut converted = outbound_transformer(target).request_from_llm(request)?;
        apply_codex_tool_context_to_chat_request(&mut converted, &value, &codex_tool_context);
        let context = (!codex_tool_context.is_empty()).then_some(codex_tool_context);
        return Ok((
            converted,
            ConversionContext {
                codex_tool_context: context,
                ..ConversionContext::default()
            },
        ));
    }

    let request = responses_compact_request_to_llm(value);
    let converted = outbound_transformer(target).request_from_llm(request)?;
    Ok((converted, ConversionContext::default()))
}

pub fn convert_target_response_body_to_responses_compact(
    source: AiProtocol,
    body: &[u8],
    context: Option<&ConversionContext>,
) -> Result<Vec<u8>, ProtocolConversionError> {
    let value = serde_json::from_slice::<Value>(body)
        .map_err(|error| ProtocolConversionError::InvalidJson(error.to_string()))?;
    let converted = convert_target_response_value_to_responses_compact(source, value, context)?;
    serde_json::to_vec(&converted)
        .map_err(|error| ProtocolConversionError::Transform(error.to_string()))
}

pub fn convert_target_response_value_to_responses_compact(
    source: AiProtocol,
    value: Value,
    context: Option<&ConversionContext>,
) -> Result<Value, ProtocolConversionError> {
    let response = if source == AiProtocol::OpenAiResponses {
        responses_compact_response_to_llm(value)
    } else {
        outbound_transformer(source).response_to_llm(value)?
    };
    let mut converted = llm_response_to_responses_compact(response);
    if source == AiProtocol::OpenAiChat {
        if let Some(codex_tool_context) = context.and_then(|context| {
            context
                .codex_tool_context
                .as_ref()
                .filter(|tool_context| !tool_context.is_empty())
        }) {
            rewrite_response_with_codex_tool_context(&mut converted, codex_tool_context);
        }
    }
    Ok(converted)
}

pub fn convert_request_body_with_context(
    route: ConversionRoute,
    body: &[u8],
) -> Result<ConvertedRequestBody, ProtocolConversionError> {
    if route.identity() {
        return Ok(ConvertedRequestBody {
            body: body.to_vec(),
            context: ConversionContext::default(),
        });
    }
    let value = serde_json::from_slice::<Value>(body)
        .map_err(|error| ProtocolConversionError::InvalidJson(error.to_string()))?;
    let (converted, context) = convert_request_value_with_context(route, value)?;
    let body = serde_json::to_vec(&converted)
        .map_err(|error| ProtocolConversionError::Transform(error.to_string()))?;
    Ok(ConvertedRequestBody { body, context })
}

pub fn convert_response_body(
    route: ConversionRoute,
    body: &[u8],
) -> Result<Vec<u8>, ProtocolConversionError> {
    if route.identity() {
        return Ok(body.to_vec());
    }
    let value = serde_json::from_slice::<Value>(body)
        .map_err(|error| ProtocolConversionError::InvalidJson(error.to_string()))?;
    let converted = convert_response_value(route, value)?;
    serde_json::to_vec(&converted)
        .map_err(|error| ProtocolConversionError::Transform(error.to_string()))
}

pub fn convert_response_body_with_context(
    route: ConversionRoute,
    body: &[u8],
    context: Option<&ConversionContext>,
) -> Result<Vec<u8>, ProtocolConversionError> {
    if route.identity() {
        return Ok(body.to_vec());
    }
    let value = serde_json::from_slice::<Value>(body)
        .map_err(|error| ProtocolConversionError::InvalidJson(error.to_string()))?;
    let converted = convert_response_value_with_context(route, value, context)?;
    serde_json::to_vec(&converted)
        .map_err(|error| ProtocolConversionError::Transform(error.to_string()))
}

pub fn convert_error_response_body(route: ConversionRoute, body: &[u8]) -> Vec<u8> {
    if route.identity() {
        return body.to_vec();
    }
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return body.to_vec();
    };
    let normalized = outbound_transformer(route.source).error_to_llm(value);
    let converted = inbound_transformer(route.target).error_from_llm(normalized);
    serde_json::to_vec(&converted).unwrap_or_else(|_| body.to_vec())
}

pub fn convert_request_value(
    route: ConversionRoute,
    value: Value,
) -> Result<Value, ProtocolConversionError> {
    if route.identity() {
        return Ok(value);
    }
    let request = inbound_transformer(route.source).request_to_llm(value)?;
    outbound_transformer(route.target).request_from_llm(request)
}

pub fn convert_request_value_with_context(
    route: ConversionRoute,
    value: Value,
) -> Result<(Value, ConversionContext), ProtocolConversionError> {
    if route.identity() {
        return Ok((value, ConversionContext::default()));
    }
    if route.source == AiProtocol::OpenAiResponses && route.target == AiProtocol::OpenAiChat {
        let codex_tool_context = build_codex_tool_context_from_request(&value);
        let request_for_chat =
            rewrite_responses_request_for_chat_context(value.clone(), &codex_tool_context);
        let request = inbound_transformer(route.source).request_to_llm(request_for_chat)?;
        let mut converted = outbound_transformer(route.target).request_from_llm(request)?;
        apply_codex_tool_context_to_chat_request(&mut converted, &value, &codex_tool_context);
        let context = (!codex_tool_context.is_empty()).then_some(codex_tool_context);
        return Ok((
            converted,
            ConversionContext {
                codex_tool_context: context,
                ..ConversionContext::default()
            },
        ));
    }
    let request = inbound_transformer(route.source).request_to_llm(value)?;
    let converted = outbound_transformer(route.target).request_from_llm(request)?;
    Ok((converted, ConversionContext::default()))
}

pub fn convert_response_value(
    route: ConversionRoute,
    value: Value,
) -> Result<Value, ProtocolConversionError> {
    convert_response_value_with_context(route, value, None)
}

pub fn convert_response_value_with_context(
    route: ConversionRoute,
    value: Value,
    context: Option<&ConversionContext>,
) -> Result<Value, ProtocolConversionError> {
    if route.identity() {
        return Ok(value);
    }
    let response = outbound_transformer(route.source).response_to_llm(value)?;
    let mut converted = inbound_transformer(route.target).response_from_llm(response)?;
    if route.source == AiProtocol::OpenAiChat && route.target == AiProtocol::OpenAiResponses {
        if let Some(codex_tool_context) = context.and_then(|context| {
            context
                .codex_tool_context
                .as_ref()
                .filter(|tool_context| !tool_context.is_empty())
        }) {
            rewrite_response_with_codex_tool_context(&mut converted, codex_tool_context);
        }
    }
    Ok(converted)
}

pub fn convert_sse_stream(
    route: ConversionRoute,
    inner: ConversionByteStream,
) -> ConversionByteStream {
    convert_sse_stream_with_context(route, inner, None)
}

pub fn convert_sse_stream_with_context(
    route: ConversionRoute,
    inner: ConversionByteStream,
    context: Option<ConversionContext>,
) -> ConversionByteStream {
    if route.identity() {
        return inner;
    }

    struct StreamState {
        inner: ConversionByteStream,
        kernel: StreamKernel,
        pending: VecDeque<Result<Vec<u8>, String>>,
        source_finished: bool,
    }

    let state = StreamState {
        inner,
        kernel: StreamKernel::with_context(route, context.unwrap_or_default()),
        pending: VecDeque::new(),
        source_finished: false,
    };

    Box::pin(stream::unfold(state, |mut state| async move {
        loop {
            if let Some(output) = state.pending.pop_front() {
                return Some((output, state));
            }
            if state.source_finished {
                return None;
            }
            match state.inner.next().await {
                Some(Ok(chunk)) => {
                    for output in state.kernel.push_chunk(&chunk) {
                        state.pending.push_back(Ok(output));
                    }
                }
                Some(Err(error)) => {
                    state.source_finished = true;
                    for output in state.kernel.fail(&error) {
                        state.pending.push_back(Ok(output));
                    }
                }
                None => {
                    state.source_finished = true;
                    for output in state.kernel.finish() {
                        state.pending.push_back(Ok(output));
                    }
                }
            }
        }
    }))
}

fn inbound_transformer(protocol: AiProtocol) -> Box<dyn InboundTransformer> {
    match protocol {
        AiProtocol::AnthropicMessages => Box::new(AnthropicInbound),
        AiProtocol::OpenAiChat => Box::new(OpenAiChatInbound),
        AiProtocol::OpenAiResponses => Box::new(OpenAiResponsesInbound),
        AiProtocol::GeminiNative => Box::new(GeminiInbound),
    }
}

fn outbound_transformer(protocol: AiProtocol) -> Box<dyn OutboundTransformer> {
    match protocol {
        AiProtocol::AnthropicMessages => Box::new(AnthropicOutbound),
        AiProtocol::OpenAiChat => Box::new(OpenAiChatOutbound),
        AiProtocol::OpenAiResponses => Box::new(OpenAiResponsesOutbound),
        AiProtocol::GeminiNative => Box::new(GeminiOutbound),
    }
}

#[cfg(test)]
#[path = "kernel_tests.rs"]
mod tests;
