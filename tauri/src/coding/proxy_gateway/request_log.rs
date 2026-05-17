use std::collections::BTreeMap;

const REDACTED: &str = "[REDACTED]";

pub fn redact_headers(headers: &[(String, String)]) -> BTreeMap<String, String> {
    headers
        .iter()
        .map(|(name, value)| {
            let normalized_name = name.to_ascii_lowercase();
            let redacted_value = if is_sensitive_header(&normalized_name) {
                REDACTED.to_string()
            } else {
                value.clone()
            };
            (name.clone(), redacted_value)
        })
        .collect()
}

pub fn is_sensitive_header(normalized_name: &str) -> bool {
    matches!(
        normalized_name,
        "authorization"
            | "proxy-authorization"
            | "cookie"
            | "set-cookie"
            | "x-api-key"
            | "api-key"
            | "anthropic-api-key"
            | "openai-api-key"
    ) || normalized_name.contains("token")
        || normalized_name.ends_with("-api-key")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_headers_redacts_authorization_and_cookie() {
        let redacted = redact_headers(&[
            ("Authorization".to_string(), "Bearer secret".to_string()),
            ("Cookie".to_string(), "session=secret".to_string()),
        ]);

        assert_eq!(redacted.get("Authorization").unwrap(), REDACTED);
        assert_eq!(redacted.get("Cookie").unwrap(), REDACTED);
    }

    #[test]
    fn redact_headers_redacts_provider_api_keys_case_insensitively() {
        let redacted = redact_headers(&[
            ("Anthropic-Api-Key".to_string(), "secret".to_string()),
            ("X-Api-Key".to_string(), "secret".to_string()),
            ("Custom-Token".to_string(), "secret".to_string()),
        ]);

        assert_eq!(redacted.get("Anthropic-Api-Key").unwrap(), REDACTED);
        assert_eq!(redacted.get("X-Api-Key").unwrap(), REDACTED);
        assert_eq!(redacted.get("Custom-Token").unwrap(), REDACTED);
    }

    #[test]
    fn redact_headers_preserves_non_sensitive_headers() {
        let redacted = redact_headers(&[
            ("Content-Type".to_string(), "application/json".to_string()),
            ("User-Agent".to_string(), "ai-toolbox".to_string()),
        ]);

        assert_eq!(redacted.get("Content-Type").unwrap(), "application/json");
        assert_eq!(redacted.get("User-Agent").unwrap(), "ai-toolbox");
    }
}
