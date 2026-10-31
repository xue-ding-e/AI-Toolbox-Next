//! Pure gateway takeover capability semantics, shared by desktop, CLI and MCP.
//! No runtime status, owner, database, configuration, environment or file access.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayCliKey {
    Claude,
    ClaudeDesktop,
    Codex,
    Grok,
    Kimi,
    Gemini,
    Antigravity,
    OpenCode,
}

impl GatewayCliKey {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::ClaudeDesktop => "claude_desktop",
            Self::Codex => "codex",
            Self::Grok => "grok",
            Self::Kimi => "kimi",
            Self::Gemini => "gemini",
            Self::Antigravity => "antigravity",
            Self::OpenCode => "opencode",
        }
    }

    pub fn supported_mvp() -> Vec<Self> {
        vec![
            Self::Claude,
            Self::ClaudeDesktop,
            Self::Codex,
            Self::Grok,
            Self::Kimi,
            Self::Gemini,
            Self::Antigravity,
        ]
    }
}

impl Default for GatewayCliKey {
    fn default() -> Self {
        Self::Claude
    }
}

/// The single takeover admission predicate, intentionally narrower than MVP.
pub fn is_supported_cli(cli_key: GatewayCliKey) -> bool {
    matches!(
        cli_key,
        GatewayCliKey::Claude
            | GatewayCliKey::ClaudeDesktop
            | GatewayCliKey::Codex
            | GatewayCliKey::Grok
            | GatewayCliKey::Kimi
            | GatewayCliKey::Gemini
    )
}

/// Preserve MVP ordering, but expose only CLIs admitted by the takeover gate.
/// This static query neither enables takeover nor asserts a running gateway.
pub fn supported_cli_keys() -> Vec<String> {
    GatewayCliKey::supported_mvp()
        .into_iter()
        .filter(|cli_key| is_supported_cli(*cli_key))
        .map(|cli_key| cli_key.as_str().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takeover_query_preserves_exact_order_and_exclusions() {
        let actual = supported_cli_keys();
        assert_eq!(actual, ["claude", "claude_desktop", "codex", "grok", "kimi", "gemini"]);
        assert_eq!(actual.iter().collect::<std::collections::HashSet<_>>().len(), actual.len());
        assert!(GatewayCliKey::supported_mvp().contains(&GatewayCliKey::Antigravity));
        assert!(!GatewayCliKey::supported_mvp().contains(&GatewayCliKey::OpenCode));
        assert!(!is_supported_cli(GatewayCliKey::Antigravity));
        assert!(!is_supported_cli(GatewayCliKey::OpenCode));
    }

    #[test]
    fn enum_wire_format_and_as_str_remain_distinct_for_open_code() {
        assert_eq!(GatewayCliKey::default(), GatewayCliKey::Claude);
        // Preserve the original enum serde contract, not a new takeover key.
        assert_eq!(serde_json::to_string(&GatewayCliKey::OpenCode).unwrap(), "\"open_code\"");
        assert_eq!(GatewayCliKey::OpenCode.as_str(), "opencode");
        for key in GatewayCliKey::supported_mvp().into_iter().chain([GatewayCliKey::OpenCode]) {
            let encoded = serde_json::to_string(&key).unwrap();
            assert_eq!(serde_json::from_str::<GatewayCliKey>(&encoded).unwrap(), key);
        }
    }
}
