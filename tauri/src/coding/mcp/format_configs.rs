//! MCP Format Configurations
//!
//! Defines format conversion rules for different tools.
//! Each tool may have its own configuration format for MCP servers.

use crate::coding::tools::McpFormatConfig;

/// OpenCode format configuration
///
/// OpenCode uses a different format than ai-toolbox's unified format:
/// - `stdio` -> `local`, `sse/http` -> `remote`
/// - `command` + `args` merged into `command: [...]`
/// - `env` -> `environment`
/// - Requires `enabled: true` field
/// Note: `http` must come before `sse` so that `map_type_from_tool("remote")`
/// returns "http" (the preferred unified type) instead of "sse".
pub const OPENCODE_FORMAT: McpFormatConfig = McpFormatConfig {
    type_mappings: &[("stdio", "local"), ("http", "remote"), ("sse", "remote")],
    merge_command_args: true,
    env_field: "environment",
    requires_enabled: true,
    default_tool_type: "local",
    supports_timeout: true,
    remote_url_field_mappings: &[],
    infer_http_from_url_when_type_missing: false,
};

/// Gemini CLI / Qwen Code / Antigravity share the same MCP shape:
/// - `http` uses `httpUrl`
/// - `sse` uses `url`
/// - `stdio` keeps `command` / `args`
pub const GEMINI_LIKE_FORMAT: McpFormatConfig = McpFormatConfig {
    type_mappings: &[],
    merge_command_args: false,
    env_field: "env",
    requires_enabled: false,
    default_tool_type: "stdio",
    supports_timeout: false,
    remote_url_field_mappings: &[("http", "httpUrl"), ("sse", "url")],
    infer_http_from_url_when_type_missing: true,
};

/// Get the format config for a tool by key
pub fn get_format_config(tool_key: &str) -> Option<&'static McpFormatConfig> {
    match tool_key {
        "opencode" => Some(&OPENCODE_FORMAT),
        "gemini_cli" | "qwen_code" | "antigravity" => Some(&GEMINI_LIKE_FORMAT),
        _ => None,
    }
}
