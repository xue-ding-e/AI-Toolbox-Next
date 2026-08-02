//! Deep-link helper utilities: log redaction, tolerant base64 decode, secret masking.

use base64::engine::general_purpose;

use super::parser::DeepLinkError;

/// Replace every query value of a deep-link URL with a sentinel so logs never
/// leak secrets (e.g. `apiKey`). Keeps scheme/host/path and the sorted list of
/// query **keys** only.
pub fn redact_url_for_log(raw: &str) -> String {
    match url::Url::parse(raw) {
        Ok(mut url) => {
            let keys: Vec<String> = url.query_pairs().map(|(k, _)| k.to_string()).collect();
            if keys.is_empty() {
                url.set_query(None);
            } else {
                let redacted: Vec<String> = keys
                    .into_iter()
                    .map(|k| format!("{k}=***REDACTED***"))
                    .collect();
                url.set_query(Some(&redacted.join("&")));
            }
            // Strip userinfo and fragment too, defensively.
            let _ = url.set_password(None);
            let _ = url.set_username("");
            url.set_fragment(None);
            url.to_string()
        }
        Err(_) => "<unparseable deep-link URL>".to_string(),
    }
}

/// Decode a base64 param tolerantly. Tries standard / standard-no-pad /
/// url-safe / url-safe-no-pad alphabets in order (mirrors the cc-switch
/// decoder). Also restores `+` lost to form-encoding spaces in the standard
/// alphabet.
pub fn tolerant_base64_decode(raw: &str) -> Result<String, DeepLinkError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    // Restore the `+` char that `application/x-www-form-urlencoded` turns into
    // a space, and drop any whitespace the user may have pasted.
    let normalized: String = trimmed
        .chars()
        .map(|c| if c == ' ' { '+' } else { c })
        .collect();

    use base64::Engine as _;
    let engines = [
        general_purpose::STANDARD,
        general_purpose::STANDARD_NO_PAD,
        general_purpose::URL_SAFE,
        general_purpose::URL_SAFE_NO_PAD,
    ];

    // First try the input verbatim, then retry with padding added — covers both
    // padded and unpadded inputs across all alphabets.
    let candidates = [normalized.clone(), pad_base64(&normalized)];
    for candidate in &candidates {
        for engine in &engines {
            if let Ok(bytes) = engine.decode(candidate) {
                if let Ok(text) = String::from_utf8(bytes) {
                    return Ok(text);
                }
            }
        }
    }

    Err(DeepLinkError::InvalidBase64("config"))
}

/// Add `=` padding so the length is a multiple of 4 (best-effort; ignored on
/// no-pad alphabets).
fn pad_base64(s: &str) -> String {
    let mut out = s.trim_end_matches('=').to_string();
    match out.len() % 4 {
        2 => out.push_str("=="),
        3 => out.push('='),
        _ => {}
    }
    out
}

/// Mask a secret for display: first 4 chars + 20 asterisks; values of length
/// ≤ 4 are fully masked. Mirrors the cc-switch dialog masking.
///
/// Currently unused on the backend (the frontend masks for display), but kept
/// here for any future backend-side log redaction of provider secrets.
#[allow(dead_code)]
pub fn mask_api_key(value: &str) -> String {
    if value.is_empty() {
        return "****".to_string();
    }
    if value.chars().count() <= 4 {
        return "*".repeat(4);
    }
    let head: String = value.chars().take(4).collect();
    format!("{head}{}", "*".repeat(20))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_replaces_all_query_values() {
        let redacted = redact_url_for_log(
            "aitoolbox://v1/import?resource=provider&app=codex&apiKey=secret&baseUrl=https%3A%2F%2Fx",
        );
        assert!(redacted.contains("apiKey=***REDACTED***"));
        assert!(redacted.contains("baseUrl=***REDACTED***"));
        assert!(!redacted.contains("secret"));
    }

    #[test]
    fn redact_handles_unparseable() {
        let redacted = redact_url_for_log("not a url at all");
        assert!(redacted.contains("unparseable"));
    }

    #[test]
    fn base64_decodes_standard() {
        use base64::Engine as _;
        let encoded = general_purpose::STANDARD.encode("hello");
        let s = tolerant_base64_decode(&encoded).unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn base64_decodes_url_safe_no_pad() {
        use base64::Engine as _;
        let encoded = general_purpose::URL_SAFE_NO_PAD.encode("{\"a\":1}");
        let s = tolerant_base64_decode(&encoded).unwrap();
        assert_eq!(s, "{\"a\":1}");
    }

    #[test]
    fn base64_decodes_form_encoded_plus_as_standard() {
        use base64::Engine as _;
        // `+` becomes space in form-encoding; decoder should restore it.
        let raw = general_purpose::STANDARD.encode("abc");
        let with_space: String = raw
            .chars()
            .map(|c| if c == '+' { ' ' } else { c })
            .collect();
        let s = tolerant_base64_decode(&with_space).unwrap();
        assert_eq!(s, "abc");
    }

    #[test]
    fn base64_rejects_garbage() {
        assert!(tolerant_base64_decode("!!!!not-base64!!!!").is_err());
    }

    #[test]
    fn mask_shows_first_four_then_stars() {
        let masked = mask_api_key("sk-ant-abcdef");
        assert!(masked.starts_with("sk-a"));
        assert_eq!(masked.len(), 4 + 20);
        assert!(!masked.contains("nt-"));
    }

    #[test]
    fn mask_fully_masks_short_values() {
        assert_eq!(mask_api_key("ab"), "****");
        assert_eq!(mask_api_key("abcd"), "****");
        assert_eq!(mask_api_key(""), "****");
    }
}
