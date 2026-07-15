use std::collections::BTreeSet;

/// Compact default matching historical gateway retry behavior.
/// Prefer collapsed ranges so save/normalize keeps the same display form.
pub const DEFAULT_RETRYABLE_STATUS_CODES_COMPACT: &str =
    "400-404,408,429,500-599";

// Only error statuses can enter retry/failover; 1xx-3xx never hit
// classify_status_failure as failures, so they are not configurable here.
const MIN_STATUS_CODE: u16 = 400;
const MAX_STATUS_CODE: u16 = 599;

/// Parse a comma-separated status-code expression into a sorted unique list.
///
/// Supported tokens:
/// - single code: `429`
/// - inclusive range: `500-599`
/// - optional surrounding whitespace
///
/// Empty / whitespace-only input falls back to the default list.
pub fn parse_retryable_status_codes(input: &str) -> Result<Vec<u16>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(default_retryable_status_codes());
    }

    let mut codes = BTreeSet::new();
    for raw_token in trimmed.split(',') {
        let token = raw_token.trim();
        if token.is_empty() {
            return Err("Retryable status codes contain an empty token".to_string());
        }

        if let Some((start_raw, end_raw)) = token.split_once('-') {
            let start = parse_status_code(start_raw.trim(), "range start")?;
            let end = parse_status_code(end_raw.trim(), "range end")?;
            if start > end {
                return Err(format!(
                    "Retryable status code range is inverted: {start}-{end}"
                ));
            }
            for code in start..=end {
                codes.insert(code);
            }
            continue;
        }

        codes.insert(parse_status_code(token, "status code")?);
    }

    if codes.is_empty() {
        return Err("Retryable status codes cannot be empty".to_string());
    }

    Ok(codes.into_iter().collect())
}

/// Normalize user input into a compact, sorted, range-collapsed expression.
/// Empty input and the historical default set keep the stable default string so
/// settings UI "restore defaults" can compare by exact text.
pub fn normalize_retryable_status_codes(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(default_retryable_status_codes_compact());
    }
    let codes = parse_retryable_status_codes(trimmed)?;
    if codes == default_retryable_status_codes() {
        return Ok(default_retryable_status_codes_compact());
    }
    Ok(format_retryable_status_codes(&codes))
}

pub fn default_retryable_status_codes() -> Vec<u16> {
    parse_retryable_status_codes(DEFAULT_RETRYABLE_STATUS_CODES_COMPACT)
        .expect("default retryable status codes must parse")
}

pub fn default_retryable_status_codes_compact() -> String {
    DEFAULT_RETRYABLE_STATUS_CODES_COMPACT.to_string()
}

/// Collapse consecutive codes into compact ranges for display/storage.
pub fn format_retryable_status_codes(codes: &[u16]) -> String {
    if codes.is_empty() {
        return default_retryable_status_codes_compact();
    }

    let mut parts = Vec::new();
    let mut index = 0;
    while index < codes.len() {
        let start = codes[index];
        let mut end = start;
        let mut next = index + 1;
        while next < codes.len() && codes[next] == end.saturating_add(1) {
            end = codes[next];
            next += 1;
        }
        if start == end {
            parts.push(start.to_string());
        } else {
            parts.push(format!("{start}-{end}"));
        }
        index = next;
    }
    parts.join(",")
}

pub fn retryable_status_code_set(input: &str) -> Result<BTreeSet<u16>, String> {
    Ok(parse_retryable_status_codes(input)?.into_iter().collect())
}

fn parse_status_code(raw: &str, label: &str) -> Result<u16, String> {
    if raw.is_empty() {
        return Err(format!("Retryable status codes {label} is empty"));
    }
    if !raw.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!(
            "Retryable status codes {label} must be a number: {raw}"
        ));
    }
    let code: u16 = raw
        .parse()
        .map_err(|_| format!("Retryable status codes {label} is out of range: {raw}"))?;
    if !(MIN_STATUS_CODE..=MAX_STATUS_CODE).contains(&code) {
        return Err(format!(
            "Retryable status codes {label} must be between {MIN_STATUS_CODE} and {MAX_STATUS_CODE}: {code}"
        ));
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_compact_expands_to_historical_codes() {
        let codes = default_retryable_status_codes();
        assert!(codes.contains(&400));
        assert!(codes.contains(&401));
        assert!(codes.contains(&402));
        assert!(codes.contains(&403));
        assert!(codes.contains(&404));
        assert!(codes.contains(&408));
        assert!(codes.contains(&429));
        assert!(codes.contains(&500));
        assert!(codes.contains(&599));
        assert!(!codes.contains(&422));
        assert!(!codes.contains(&415));
        assert_eq!(codes.len(), 7 + 100);
    }

    #[test]
    fn parse_supports_mixed_singles_and_ranges() {
        let codes = parse_retryable_status_codes("429, 502-504, 400").unwrap();
        assert_eq!(codes, vec![400, 429, 502, 503, 504]);
        assert_eq!(
            format_retryable_status_codes(&codes),
            "400,429,502-504"
        );
    }

    #[test]
    fn empty_input_falls_back_to_default() {
        let codes = parse_retryable_status_codes("   ").unwrap();
        assert_eq!(codes, default_retryable_status_codes());
        assert_eq!(
            normalize_retryable_status_codes("").unwrap(),
            DEFAULT_RETRYABLE_STATUS_CODES_COMPACT
        );
    }

    #[test]
    fn rejects_invalid_tokens() {
        assert!(parse_retryable_status_codes("abc").is_err());
        assert!(parse_retryable_status_codes("99").is_err());
        assert!(parse_retryable_status_codes("302").is_err());
        assert!(parse_retryable_status_codes("399").is_err());
        assert!(parse_retryable_status_codes("600").is_err());
        assert!(parse_retryable_status_codes("500-400").is_err());
        assert!(parse_retryable_status_codes("400,,429").is_err());
        assert!(parse_retryable_status_codes("200-599").is_err());
    }
}
