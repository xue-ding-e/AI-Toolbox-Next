#![allow(dead_code)]

const ANTHROPIC_MARKER: &str = "ai-toolbox.sig.anthropic:";
const GEMINI_MARKER: &str = "ai-toolbox.sig.gemini:";
const OPENAI_RESPONSES_MARKER: &str = "ai-toolbox.sig.openai_responses:";

fn encode(marker: &str, value: &str, footprint: &str) -> String {
    if footprint.is_empty() || value.starts_with(marker) {
        return value.to_string();
    }
    format!("{marker}{footprint}:{value}")
}

fn decode(marker: &str, value: &str, footprint: &str) -> Option<String> {
    if footprint.is_empty() {
        return Some(value.to_string());
    }
    let prefix = format!("{marker}{footprint}:");
    value
        .strip_prefix(&prefix)
        .map(ToString::to_string)
        .or_else(|| (!value.contains("ai-toolbox.sig.")).then(|| value.to_string()))
}

pub fn encode_anthropic_signature(value: &str, footprint: &str) -> String {
    encode(ANTHROPIC_MARKER, value, footprint)
}

pub fn decode_anthropic_signature(value: &str, footprint: &str) -> Option<String> {
    decode(ANTHROPIC_MARKER, value, footprint)
}

pub fn encode_gemini_signature(value: &str, footprint: &str) -> String {
    encode(GEMINI_MARKER, value, footprint)
}

pub fn decode_gemini_signature(value: &str, footprint: &str) -> Option<String> {
    decode(GEMINI_MARKER, value, footprint)
}

pub fn encode_openai_responses_signature(value: &str, footprint: &str) -> String {
    encode(OPENAI_RESPONSES_MARKER, value, footprint)
}

pub fn decode_openai_responses_signature(value: &str, footprint: &str) -> Option<String> {
    decode(OPENAI_RESPONSES_MARKER, value, footprint)
}
