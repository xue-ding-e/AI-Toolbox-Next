//! Claude Desktop backend module constants.
//!
//! Claude Desktop uses a JSON config file layout per platform. This module is a
//! "config file path module": it locates and rewrites the Claude Desktop 3P
//! profile files rather than managing a CLI root directory.

/// Name of the Claude Desktop config JSON file inside the Claude / Claude-3p dirs.
pub const CONFIG_FILE: &str = "claude_desktop_config.json";

/// Directory (inside the Claude-3p data dir) that holds per-profile JSON and the
/// shared `_meta.json` applied-state file.
pub const CONFIG_LIBRARY_DIR: &str = "configLibrary";

/// Fixed 3P profile ID used to identify this app's gateway profile on disk.
pub const PROFILE_ID: &str = "00000000-0000-4000-8000-000000157210";

/// Display name used for this app's profile entry in `_meta.json`.
pub const PROFILE_NAME: &str = "AI Toolbox";

/// Database ID of the seeded "Claude Desktop Official" provider.
pub const OFFICIAL_PROVIDER_ID: &str = "claude-desktop-official";

/// Reserved database ID holding the editable common (base) config for Claude
/// Desktop, stored inside the `claude_desktop_provider` table.
pub const COMMON_CONFIG_ID: &str = "__common__";

/// enterpriseConfig keys owned by the 3P gateway profile. `restore_official`
/// removes exactly these keys (and the whole `enterpriseConfig` object once empty).
pub const MANAGED_ENTERPRISE_CONFIG_KEYS: [&str; 5] = [
    "disableDeploymentModeChooser",
    "inferenceGatewayApiKey",
    "inferenceGatewayAuthScheme",
    "inferenceGatewayBaseUrl",
    "inferenceProvider",
];

/// Reserved profile keys written by `apply` / removed by `restore_official`.
pub const MANAGED_PROFILE_KEYS: [&str; 5] = [
    "coworkEgressAllowedHosts",
    "disableDeploymentModeChooser",
    "inferenceGatewayApiKey",
    "inferenceGatewayAuthScheme",
    "inferenceGatewayBaseUrl",
];

/// Claude Desktop model menu route-id prefixes Claude Desktop itself accepts.
pub const CLAUDE_ROUTE_PREFIX: &str = "claude-";
pub const ANTHROPIC_CLAUDE_ROUTE_PREFIX: &str = "anthropic/claude-";

/// 1M-context marker used by Claude Code env; Claude Desktop schema rejects it,
/// so it is also rejected as a safe model id.
pub const ONE_M_CONTEXT_MARKER: &str = "[1m]";

/// env keys that provide direct-mode credentials in `settings_config.env`.
pub const DIRECT_BASE_URL_ENV_KEY: &str = "ANTHROPIC_BASE_URL";
pub const DIRECT_AUTH_TOKEN_ENV_KEY: &str = "ANTHROPIC_AUTH_TOKEN";

/// Default proxy-mode route catalog (used only to keep proxy build logic around;
/// the local gateway is wired later, so this is not currently consumed end-to-end).
#[derive(Debug, Clone, Copy)]
pub struct DefaultProxyRoute {
    pub route_id: &'static str,
    pub env_key: &'static str,
    pub supports_1m: bool,
}

pub const DEFAULT_PROXY_ROUTES: &[DefaultProxyRoute] = &[
    DefaultProxyRoute {
        route_id: "claude-sonnet-5",
        env_key: "ANTHROPIC_DEFAULT_SONNET_MODEL",
        supports_1m: true,
    },
    DefaultProxyRoute {
        route_id: "claude-opus-5",
        env_key: "ANTHROPIC_DEFAULT_OPUS_MODEL",
        supports_1m: true,
    },
    DefaultProxyRoute {
        route_id: "claude-haiku-4-5",
        env_key: "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        supports_1m: true,
    },
    DefaultProxyRoute {
        route_id: "claude-fable-5",
        env_key: "ANTHROPIC_DEFAULT_FABLE_MODEL",
        supports_1m: true,
    },
];
