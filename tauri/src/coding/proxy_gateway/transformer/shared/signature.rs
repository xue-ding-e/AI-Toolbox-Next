use base64::Engine;
use serde_json::{json, Value};

pub const GEMINI_THOUGHT_SIGNATURE_METADATA_KEY: &str = "gemini_thought_signature";
pub const DEFAULT_GEMINI_THOUGHT_SIGNATURE: &str =
    "Y29udGV4dF9lbmdpbmVlcmluZ19pc190aGVfd2F5X3RvX2dv";

const ANTHROPIC_MARKER: &str = "ai-toolbox.sig.anthropic:";
const GEMINI_MARKER: &str = "ai-toolbox.sig.gemini:";
const OPENAI_RESPONSES_MARKER: &str = "ai-toolbox.sig.openai_responses:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureProvider {
    Anthropic,
    Gemini,
    OpenAiResponses,
    Unknown,
}

impl SignatureProvider {
    fn marker(self) -> Option<&'static str> {
        match self {
            Self::Anthropic => Some(ANTHROPIC_MARKER),
            Self::Gemini => Some(GEMINI_MARKER),
            Self::OpenAiResponses => Some(OPENAI_RESPONSES_MARKER),
            Self::Unknown => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureValue {
    pub provider: SignatureProvider,
    pub value: String,
}

pub fn encode_signature(provider: SignatureProvider, raw: &str) -> String {
    let Some(marker) = provider.marker() else {
        return raw.to_string();
    };
    if parse_marked_signature(raw).is_some_and(|signature| signature.provider == provider) {
        raw.to_string()
    } else {
        format!("{marker}{raw}")
    }
}

pub fn decode_signature_for(provider: SignatureProvider, value: &str) -> Option<String> {
    if provider == SignatureProvider::Unknown || signature_provider(value) != provider {
        return None;
    }
    parse_marked_signature(value)
        .map(|signature| signature.value)
        .or_else(|| Some(value.to_string()))
}

pub fn signature_provider(value: &str) -> SignatureProvider {
    parse_marked_signature(value)
        .map(|signature| signature.provider)
        .unwrap_or_else(|| guess_signature_provider(value))
}

pub fn metadata_signature(raw: &str) -> Value {
    json!(raw)
}

pub fn metadata_signature_raw(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(ToString::to_string)
}

fn parse_marked_signature(value: &str) -> Option<SignatureValue> {
    if let Some(raw) = value.strip_prefix(ANTHROPIC_MARKER) {
        return Some(SignatureValue {
            provider: SignatureProvider::Anthropic,
            value: raw.to_string(),
        });
    }
    if let Some(raw) = value.strip_prefix(GEMINI_MARKER) {
        return Some(SignatureValue {
            provider: SignatureProvider::Gemini,
            value: raw.to_string(),
        });
    }
    if let Some(raw) = value.strip_prefix(OPENAI_RESPONSES_MARKER) {
        return Some(SignatureValue {
            provider: SignatureProvider::OpenAiResponses,
            value: raw.to_string(),
        });
    }
    None
}

pub fn guess_signature_provider(raw: &str) -> SignatureProvider {
    let value = raw.trim().trim_matches('"').trim();
    if value.starts_with("gAAAA") || value.starts_with("gAAA") {
        return SignatureProvider::OpenAiResponses;
    }
    if is_standard_base64(value) {
        if let Some(bytes) = decode_std_base64(value) {
            // Real Anthropic thinking signatures are opaque (often
            // protobuf-like) bytes that embed the model name, so the marker
            // check must precede the Gemini protobuf heuristic — otherwise a
            // valid Anthropic signature would be misclassified as Gemini.
            if contains_anthropic_model(&bytes) {
                return SignatureProvider::Anthropic;
            }
            if looks_like_protobuf(&bytes) {
                return SignatureProvider::Gemini;
            }
        }
    }
    SignatureProvider::Unknown
}

/// Decodes standard Base64 accepting both padded and unpadded forms. Anthropic
/// signatures are opaque bytes and commonly have a length divisible by three,
/// but accepting the unpadded form keeps detection safe across proxies/clients.
fn decode_std_base64(value: &str) -> Option<Vec<u8>> {
    if value.contains('=') {
        base64::engine::general_purpose::STANDARD.decode(value).ok()
    } else {
        base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(value)
            .ok()
    }
}

/// Checks decoded opaque bytes for an Anthropic model marker. Signatures are
/// binary (often protobuf-like), so the model name is matched as an ASCII
/// substring rather than parsing the whole payload as text or wire format.
fn contains_anthropic_model(buf: &[u8]) -> bool {
    let lower: Vec<u8> = buf.iter().map(|byte| byte.to_ascii_lowercase()).collect();
    contains_subslice(&lower, b"claude") || contains_subslice(&lower, b"anthropic")
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn is_standard_base64(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    let mut padding_started = false;
    let mut padding_count = 0;
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' => {
                if padding_started {
                    return false;
                }
            }
            b'=' => {
                padding_started = true;
                padding_count += 1;
                if padding_count > 2 {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

fn looks_like_protobuf(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let mut offset = 0;
    while offset < bytes.len() {
        let Some((tag, tag_len)) = read_varint(&bytes[offset..]) else {
            return offset > 0;
        };
        offset += tag_len;
        let wire_type = tag & 0x07;
        let field_number = tag >> 3;
        if field_number == 0 || wire_type == 3 || wire_type == 4 {
            return false;
        }
        match wire_type {
            0 => {
                let Some((_, len)) = read_varint(&bytes[offset..]) else {
                    return false;
                };
                offset += len;
            }
            1 => {
                if offset + 8 > bytes.len() {
                    return false;
                }
                offset += 8;
            }
            2 => {
                let Some((len, len_size)) = read_varint(&bytes[offset..]) else {
                    return false;
                };
                let len = len as usize;
                if offset + len_size + len > bytes.len() {
                    return false;
                }
                offset += len_size + len;
            }
            5 => {
                if offset + 4 > bytes.len() {
                    return false;
                }
                offset += 4;
            }
            _ => return false,
        }
    }
    true
}

fn read_varint(bytes: &[u8]) -> Option<(u64, usize)> {
    let mut result = 0_u64;
    for (index, byte) in bytes.iter().take(10).enumerate() {
        result |= u64::from(byte & 0x7f) << (index * 7);
        if byte & 0x80 == 0 {
            return Some((result, index + 1));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marked_signatures_decode_only_for_matching_provider() {
        let anthropic = encode_signature(SignatureProvider::Anthropic, "EqQabc");
        assert_eq!(
            decode_signature_for(SignatureProvider::Anthropic, &anthropic),
            Some("EqQabc".to_string())
        );
        assert_eq!(
            decode_signature_for(SignatureProvider::OpenAiResponses, &anthropic),
            None
        );

        let gemini = encode_signature(SignatureProvider::Gemini, "CgR0ZXN0");
        assert_eq!(
            decode_signature_for(SignatureProvider::Gemini, &gemini),
            Some("CgR0ZXN0".to_string())
        );
        assert_eq!(
            decode_signature_for(SignatureProvider::Anthropic, &gemini),
            None
        );
    }

    #[test]
    fn encoding_only_treats_matching_marker_as_idempotent() {
        let nested = encode_signature(
            SignatureProvider::Anthropic,
            "ai-toolbox.sig.gemini:CgR0ZXN0",
        );
        assert_eq!(
            decode_signature_for(SignatureProvider::Anthropic, &nested),
            Some("ai-toolbox.sig.gemini:CgR0ZXN0".to_string())
        );
        assert_eq!(
            decode_signature_for(SignatureProvider::Gemini, &nested),
            None
        );
    }

    // Mirrors a current Anthropic signature: the encoded value starts with
    // "Eq0C" and its opaque decoded payload contains a Claude model name. This
    // is also protobuf-like, so it doubles as the ordering regression case —
    // it must classify as Anthropic, not Gemini.
    const REAL_ANTHROPIC_SIGNATURE: &str = "Eq0CCokBCBAYAipAmkim+S4ApjNpcVSh82hYj016e9aYlvNfdj8ZaVbASj64fkCHtgDxjvumIhTpVr6WsoYoGyBtZOuoFPg7JUV7vjIPY2xhdWRlLXNvbm5ldC01OABCCHRoaW5raW5nWiRjYTEwYTFhOS03ZWFmLTRiZDUtYWFkMy1iY2MyY2Q1MWQ1MDgSDETIROQHvz1/jQbeLxIMwjZzeFruDsqTqYxwGgy4ekwdZi3oeDEsWGsiMD3w0HGjBb28dNuTqZE1X2zCSndpSwOWYwRhbrXFV8RIg6jFiS+MSo6Gt0QUFWKh4CpD8q8wDmAKZYQ45z+1rFBwX7SWdXo02qQNUkGIwm1fTFf/GIRTRwIUTNdG35tcDHWh6pJ/if5LjcPdJTMiiw+bFgPTCBgB";

    #[test]
    fn guesses_provider_from_unmarked_known_shapes() {
        assert_eq!(
            guess_signature_provider(concat!("g", "AAAAABfixture-openai")),
            SignatureProvider::OpenAiResponses
        );
        // Old Eq*/Eqo*/Eqr* prefixes without a decoded model marker are no
        // longer recognized as Anthropic on prefix alone.
        assert_eq!(
            guess_signature_provider("EqQBCAEDEgQIAhAEGAAgAigBMOzOAg=="),
            SignatureProvider::Unknown
        );
        assert_eq!(
            guess_signature_provider("EqoBxxxxxxxx"),
            SignatureProvider::Unknown
        );
        assert_eq!(
            guess_signature_provider("EqrBxxxxxxxx"),
            SignatureProvider::Unknown
        );
        assert_eq!(guess_signature_provider("EqQ"), SignatureProvider::Unknown);
        // Decoded payload carrying a Claude model marker is recognized as
        // Anthropic even though it is also protobuf-shaped.
        assert_eq!(
            guess_signature_provider(REAL_ANTHROPIC_SIGNATURE),
            SignatureProvider::Anthropic
        );
        assert_eq!(
            guess_signature_provider("CgR0ZXN0"),
            SignatureProvider::Gemini
        );
        assert_eq!(
            guess_signature_provider("plain-unknown-signature"),
            SignatureProvider::Unknown
        );
    }

    #[test]
    fn anthropic_signature_with_claude_marker_beats_gemini_protobuf_shape() {
        // Order regression: a payload that is both protobuf-like AND embeds a
        // Claude model name must resolve to Anthropic, never Gemini.
        let mut binary_signature = vec![0x12, 0xad, 0x02, 0x0a, 0x89, 0x01];
        binary_signature
            .extend_from_slice(b"claude-sonnet-5 thinking ca10a1a9-7eaf-4bd5-aad3-bcc2cd51d508");
        let raw = base64::engine::general_purpose::STANDARD.encode(&binary_signature);
        assert_eq!(guess_signature_provider(&raw), SignatureProvider::Anthropic);
    }

    #[test]
    fn contains_anthropic_model_detects_markers_case_insensitively() {
        assert!(contains_anthropic_model(&[
            0x12, 0x03, 0x00, b'C', b'l', b'a', b'u', b'd', b'e'
        ]));
        assert!(contains_anthropic_model(&[
            0x00, 0x01, b'a', b'n', b't', b'h', b'r', b'o', b'p', b'i', b'c'
        ]));
        assert!(!contains_anthropic_model(&[0x0a, 0x02, 0x08, 0x01]));
        assert!(contains_anthropic_model(b"CLAUDE-OPUS"));
    }

    #[test]
    fn unmarked_unknown_signature_is_not_decoded_for_any_provider() {
        assert_eq!(
            decode_signature_for(SignatureProvider::Anthropic, "plain-unknown-signature"),
            None
        );
        assert_eq!(
            decode_signature_for(SignatureProvider::Gemini, "plain-unknown-signature"),
            None
        );
        assert_eq!(
            decode_signature_for(
                SignatureProvider::OpenAiResponses,
                "plain-unknown-signature"
            ),
            None
        );
    }
}
