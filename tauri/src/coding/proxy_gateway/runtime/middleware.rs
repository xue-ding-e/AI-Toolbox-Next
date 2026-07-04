use crate::coding::proxy_gateway::transformer::AiProtocol;
use serde_json::Value;

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub(super) struct PipelineContext {
    pub provider_type: Option<String>,
    pub target_protocol: Option<AiProtocol>,
    pub lossy_warnings: Vec<String>,
    pub billing_cch: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ErrorDecision {
    Propagate,
    Retry,
}

pub(super) trait Middleware: Send + Sync {
    fn on_inbound_request(
        &self,
        _body: &mut Value,
        _ctx: &mut PipelineContext,
    ) -> Result<(), String> {
        Ok(())
    }

    fn on_outbound_body(&self, _body: &mut Value, _ctx: &PipelineContext) -> Result<(), String> {
        Ok(())
    }

    fn on_stream_chunk(
        &self,
        _chunk: &mut Value,
        _ctx: &mut PipelineContext,
    ) -> Result<(), String> {
        Ok(())
    }

    fn on_error(&self, _message: &str, _ctx: &PipelineContext) -> ErrorDecision {
        ErrorDecision::Propagate
    }
}

const BILLING_HEADER_PREFIX: &str = "x-anthropic-billing-header:";

#[derive(Debug, Clone, Copy, Default)]
#[allow(dead_code)]
pub(super) struct BillingHeaderCchMiddleware;

impl Middleware for BillingHeaderCchMiddleware {
    fn on_inbound_request(
        &self,
        body: &mut Value,
        ctx: &mut PipelineContext,
    ) -> Result<(), String> {
        let mut captured_cch = None;
        visit_billing_texts_mut(body, |text| {
            let (stripped, cch, changed) = strip_billing_cch_from_text(text);
            if changed {
                *text = stripped;
                if captured_cch.is_none() {
                    captured_cch = cch;
                }
            }
        });
        if ctx.billing_cch.is_none() {
            ctx.billing_cch = captured_cch;
        }
        Ok(())
    }

    fn on_outbound_body(&self, body: &mut Value, ctx: &PipelineContext) -> Result<(), String> {
        if ctx.target_protocol != Some(AiProtocol::AnthropicMessages) {
            return Ok(());
        }
        let Some(cch) = ctx
            .billing_cch
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(());
        };
        visit_billing_texts_mut(body, |text| {
            let (restored, changed) = restore_billing_cch_to_text(text, cch);
            if changed {
                *text = restored;
            }
        });
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(super) struct EnsureMaxTokensMiddleware {
    default_max_tokens: i64,
}

impl EnsureMaxTokensMiddleware {
    #[allow(dead_code)]
    pub(super) fn new(default_max_tokens: i64) -> Self {
        Self { default_max_tokens }
    }
}

impl Middleware for EnsureMaxTokensMiddleware {
    fn on_outbound_body(&self, body: &mut Value, ctx: &PipelineContext) -> Result<(), String> {
        if self.default_max_tokens <= 0 {
            return Ok(());
        }
        match ctx.target_protocol {
            Some(AiProtocol::AnthropicMessages) => {
                ensure_numeric_cap(body, &["max_tokens"], self.default_max_tokens);
            }
            Some(AiProtocol::OpenAiResponses) => {
                ensure_numeric_cap(body, &["max_output_tokens"], self.default_max_tokens);
            }
            Some(AiProtocol::GeminiNative) => {
                ensure_gemini_max_output_tokens(body, self.default_max_tokens);
            }
            Some(AiProtocol::OpenAiChat) | None => {
                ensure_openai_chat_max_tokens(body, self.default_max_tokens);
            }
        }
        Ok(())
    }
}

fn ensure_openai_chat_max_tokens(body: &mut Value, default_max_tokens: i64) {
    if body.get("max_completion_tokens").is_some() {
        ensure_numeric_cap(body, &["max_completion_tokens"], default_max_tokens);
        return;
    }
    ensure_numeric_cap(body, &["max_tokens"], default_max_tokens);
}

fn ensure_gemini_max_output_tokens(body: &mut Value, default_max_tokens: i64) {
    let Value::Object(object) = body else {
        return;
    };
    let generation_config = object
        .entry("generationConfig".to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    ensure_numeric_cap(generation_config, &["maxOutputTokens"], default_max_tokens);
}

fn ensure_numeric_cap(body: &mut Value, path: &[&str], default_max_tokens: i64) {
    if path.is_empty() {
        return;
    }
    let Some(parent) = object_at_path_mut(body, &path[..path.len() - 1]) else {
        return;
    };
    let key = path[path.len() - 1];
    let current = parent.get(key).and_then(Value::as_i64);
    if current.is_none_or(|value| value > default_max_tokens) {
        parent.insert(key.to_string(), Value::Number(default_max_tokens.into()));
    }
}

fn object_at_path_mut<'a>(
    value: &'a mut Value,
    path: &[&str],
) -> Option<&'a mut serde_json::Map<String, Value>> {
    let mut current = value;
    for segment in path {
        let object = current.as_object_mut()?;
        current = object
            .entry((*segment).to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
    }
    current.as_object_mut()
}

fn visit_billing_texts_mut<F>(body: &mut Value, mut visit: F)
where
    F: FnMut(&mut String),
{
    visit_anthropic_system_texts_mut(body, &mut visit);
    visit_openai_system_message_texts_mut(body, &mut visit);
}

fn visit_anthropic_system_texts_mut<F>(body: &mut Value, visit: &mut F)
where
    F: FnMut(&mut String),
{
    let Some(system) = body.get_mut("system") else {
        return;
    };
    match system {
        Value::String(text) => visit(text),
        Value::Array(parts) => {
            for part in parts {
                if let Some(mut owned) = part
                    .get("text")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                {
                    let original = owned.clone();
                    visit(&mut owned);
                    if owned != original {
                        part["text"] = Value::String(owned);
                    }
                }
            }
        }
        _ => {}
    }
}

fn visit_openai_system_message_texts_mut<F>(body: &mut Value, visit: &mut F)
where
    F: FnMut(&mut String),
{
    let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) else {
        return;
    };
    for message in messages {
        if message.get("role").and_then(Value::as_str) != Some("system") {
            continue;
        }
        let Some(content) = message.get_mut("content") else {
            continue;
        };
        match content {
            Value::String(text) => visit(text),
            Value::Array(parts) => {
                for part in parts {
                    if part.get("type").and_then(Value::as_str) != Some("text") {
                        continue;
                    }
                    if let Some(mut owned) = part
                        .get("text")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                    {
                        let original = owned.clone();
                        visit(&mut owned);
                        if owned != original {
                            part["text"] = Value::String(owned);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn strip_billing_cch_from_text(text: &str) -> (String, Option<String>, bool) {
    let Some((line, rest)) = split_first_line(text) else {
        return strip_billing_cch_from_header_line(text).map_or_else(
            || (text.to_string(), None, false),
            |(line, cch)| (line, Some(cch), true),
        );
    };
    let Some((stripped_line, cch)) = strip_billing_cch_from_header_line(line) else {
        return (text.to_string(), None, false);
    };
    (format!("{stripped_line}{rest}"), Some(cch), true)
}

fn strip_billing_cch_from_header_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if !trimmed
        .to_ascii_lowercase()
        .starts_with(BILLING_HEADER_PREFIX)
    {
        return None;
    }
    let rest = trimmed[BILLING_HEADER_PREFIX.len()..].trim();
    let had_trailing_semi = rest.ends_with(';');
    let mut kept = Vec::new();
    let mut cch = None;
    for part in rest.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part.to_ascii_lowercase().starts_with("cch=") {
            if cch.is_none() {
                cch = Some(part["cch=".len()..].trim().to_string());
            }
            continue;
        }
        kept.push(part.to_string());
    }
    let cch = cch.filter(|value| !value.is_empty())?;
    let mut output = BILLING_HEADER_PREFIX.to_string();
    if !kept.is_empty() {
        output.push(' ');
        output.push_str(&kept.join("; "));
    }
    if had_trailing_semi || !kept.is_empty() {
        output.push(';');
    }
    Some((output, cch))
}

fn restore_billing_cch_to_text(text: &str, cch: &str) -> (String, bool) {
    let Some((line, rest)) = split_first_line(text) else {
        return restore_billing_cch_to_header_line(text, cch)
            .map(|line| (line, true))
            .unwrap_or_else(|| (text.to_string(), false));
    };
    let Some(restored_line) = restore_billing_cch_to_header_line(line, cch) else {
        return (text.to_string(), false);
    };
    (format!("{restored_line}{rest}"), true)
}

fn restore_billing_cch_to_header_line(line: &str, cch: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed
        .to_ascii_lowercase()
        .starts_with(BILLING_HEADER_PREFIX)
    {
        return None;
    }
    if trimmed[BILLING_HEADER_PREFIX.len()..]
        .split(';')
        .any(|part| part.trim().to_ascii_lowercase().starts_with("cch="))
    {
        return None;
    }
    let mut without_trailing = trimmed.trim_end_matches(';').trim_end().to_string();
    if !without_trailing.ends_with(BILLING_HEADER_PREFIX) {
        without_trailing.push_str("; ");
    } else {
        without_trailing.push(' ');
    }
    without_trailing.push_str("cch=");
    without_trailing.push_str(cch);
    without_trailing.push(';');
    Some(without_trailing)
}

fn split_first_line(text: &str) -> Option<(&str, &str)> {
    let line_end = text.find(['\n', '\r'])?;
    Some((&text[..line_end], &text[line_end..]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding::proxy_gateway::runtime::pipeline::Pipeline;
    use serde_json::json;
    use std::sync::Arc;

    fn run(body: &mut Value, protocol: AiProtocol, max_tokens: i64) {
        let pipeline = Pipeline::new(vec![Arc::new(EnsureMaxTokensMiddleware::new(max_tokens))]);
        pipeline
            .run_outbound_body(
                body,
                &PipelineContext {
                    target_protocol: Some(protocol),
                    ..PipelineContext::default()
                },
            )
            .unwrap();
    }

    #[test]
    fn ensure_max_tokens_sets_missing_openai_chat_value() {
        let mut body = json!({"model":"gpt-4o","messages":[]});

        run(&mut body, AiProtocol::OpenAiChat, 200);

        assert_eq!(body["max_tokens"], 200);
    }

    #[test]
    fn ensure_max_tokens_caps_existing_openai_chat_value() {
        let mut body = json!({"model":"gpt-5","messages":[],"max_completion_tokens":1000});

        run(&mut body, AiProtocol::OpenAiChat, 200);

        assert_eq!(body["max_completion_tokens"], 200);
        assert!(body.get("max_tokens").is_none());
    }

    #[test]
    fn ensure_max_tokens_keeps_smaller_existing_value() {
        let mut body = json!({"model":"gpt-4o","messages":[],"max_tokens":100});

        run(&mut body, AiProtocol::OpenAiChat, 200);

        assert_eq!(body["max_tokens"], 100);
    }

    #[test]
    fn ensure_max_tokens_sets_protocol_specific_fields() {
        let mut anthropic = json!({"model":"claude-sonnet","messages":[]});
        let mut responses = json!({"model":"gpt-5","input":[]});
        let mut gemini = json!({"contents":[]});

        run(&mut anthropic, AiProtocol::AnthropicMessages, 300);
        run(&mut responses, AiProtocol::OpenAiResponses, 300);
        run(&mut gemini, AiProtocol::GeminiNative, 300);

        assert_eq!(anthropic["max_tokens"], 300);
        assert_eq!(responses["max_output_tokens"], 300);
        assert_eq!(gemini["generationConfig"]["maxOutputTokens"], 300);
    }

    #[test]
    fn billing_header_cch_middleware_strips_and_restores_anthropic_system() {
        let mut body = json!({
            "system": [
                {"type":"text","text":"x-anthropic-billing-header: cc_version=2.1.42; cc_entrypoint=cli; cch=38a80;\n\nStable prompt"}
            ],
            "messages":[{"role":"user","content":"hi"}]
        });
        let middleware = BillingHeaderCchMiddleware;
        let mut ctx = PipelineContext {
            target_protocol: Some(AiProtocol::AnthropicMessages),
            ..PipelineContext::default()
        };

        middleware.on_inbound_request(&mut body, &mut ctx).unwrap();

        assert_eq!(ctx.billing_cch.as_deref(), Some("38a80"));
        assert_eq!(
            body["system"][0]["text"],
            "x-anthropic-billing-header: cc_version=2.1.42; cc_entrypoint=cli;\n\nStable prompt"
        );

        middleware.on_outbound_body(&mut body, &ctx).unwrap();

        assert_eq!(
            body["system"][0]["text"],
            "x-anthropic-billing-header: cc_version=2.1.42; cc_entrypoint=cli; cch=38a80;\n\nStable prompt"
        );
    }

    #[test]
    fn billing_header_cch_middleware_handles_openai_system_text_part() {
        let mut body = json!({
            "messages": [
                {
                    "role":"system",
                    "content":[
                        {"type":"text","text":"x-anthropic-billing-header: cc_version=2.1.42; cch=abcde;"}
                    ]
                },
                {"role":"user","content":"hi"}
            ]
        });
        let middleware = BillingHeaderCchMiddleware;
        let mut ctx = PipelineContext::default();

        middleware.on_inbound_request(&mut body, &mut ctx).unwrap();

        assert_eq!(ctx.billing_cch.as_deref(), Some("abcde"));
        assert_eq!(
            body["messages"][0]["content"][0]["text"],
            "x-anthropic-billing-header: cc_version=2.1.42;"
        );
    }
}
