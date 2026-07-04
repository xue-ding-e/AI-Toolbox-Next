pub const THINKING_BUDGET_MINIMAL: i64 = 1_024;
pub const THINKING_BUDGET_LOW: i64 = 4_096;
pub const THINKING_BUDGET_MEDIUM: i64 = 10_240;
pub const THINKING_BUDGET_HIGH: i64 = 32_768;

pub fn reasoning_effort_to_budget_tokens(effort: &str, max_tokens: Option<i64>) -> Option<i64> {
    match normalize_reasoning_effort(effort).as_deref() {
        Some("none") => Some(0),
        Some("minimal") => Some(THINKING_BUDGET_MINIMAL),
        Some("low") => Some(THINKING_BUDGET_LOW),
        Some("medium") => Some(THINKING_BUDGET_MEDIUM),
        Some("high") => Some(THINKING_BUDGET_HIGH),
        Some("xhigh" | "max") => Some(max_tokens.unwrap_or(THINKING_BUDGET_HIGH)),
        _ => None,
    }
}

pub fn budget_tokens_to_reasoning_effort(budget_tokens: i64) -> &'static str {
    if budget_tokens <= 0 {
        "none"
    } else if budget_tokens <= THINKING_BUDGET_MINIMAL {
        "minimal"
    } else if budget_tokens <= THINKING_BUDGET_LOW {
        "low"
    } else if budget_tokens <= THINKING_BUDGET_MEDIUM {
        "medium"
    } else if budget_tokens <= THINKING_BUDGET_HIGH {
        "high"
    } else {
        "xhigh"
    }
}

pub fn normalize_reasoning_effort(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "none" | "off" | "disabled" => Some("none".to_string()),
        "minimal" | "min" => Some("minimal".to_string()),
        "low" => Some("low".to_string()),
        "medium" => Some("medium".to_string()),
        "high" => Some("high".to_string()),
        "xhigh" | "extra_high" | "max" => Some(normalized),
        _ => None,
    }
}
