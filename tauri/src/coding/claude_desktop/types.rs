use serde::{Deserialize, Serialize};
use serde_json::Value;

// ============================================================================
// Claude Desktop Mode
// ============================================================================

/// How a Claude Desktop provider is applied to the disk profile.
///
/// Stored in the provider record's `meta.claude_desktop_mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaudeDesktopMode {
    /// Write the upstream credentials (ANTHROPIC_BASE_URL / ANTHROPIC_AUTH_TOKEN)
    /// directly into the 3P gateway profile. This is the core, fully supported path.
    Direct,
    /// Point Claude Desktop at the local gateway with model route mapping.
    /// The gateway wiring is a later task; see `config_writer` for the error today.
    Proxy,
}

impl Default for ClaudeDesktopMode {
    fn default() -> Self {
        Self::Direct
    }
}

/// A single model route inside `meta.claude_desktop_model_routes`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopModelRoute {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_override: Option<String>,
    #[serde(default)]
    pub supports_1m: bool,
    /// Claude Desktop `anthropicFamilyTier`: which Claude tier (haiku/sonnet/opus/
    /// fable/mythos) this model stands in for. None = omit the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier_alias: Option<String>,
}

// ============================================================================
// Claude Desktop Provider Types
// ============================================================================

/// ClaudeDesktopProvider - database record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeDesktopProviderRecord {
    pub id: String,
    pub name: String,
    pub category: String,
    pub settings_config: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    pub is_applied: bool,
    pub is_disabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// ClaudeDesktopProvider - API response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopProvider {
    pub id: String,
    pub name: String,
    pub category: String,
    pub settings_config: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    pub is_applied: bool,
    pub is_disabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ClaudeDesktopProviderRecord> for ClaudeDesktopProvider {
    fn from(record: ClaudeDesktopProviderRecord) -> Self {
        ClaudeDesktopProvider {
            id: record.id,
            name: record.name,
            category: record.category,
            settings_config: record.settings_config,
            source_provider_id: record.source_provider_id,
            website_url: record.website_url,
            notes: record.notes,
            icon: record.icon,
            icon_color: record.icon_color,
            sort_index: record.sort_index,
            meta: record.meta,
            is_applied: record.is_applied,
            is_disabled: record.is_disabled,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

/// ClaudeDesktopProvider - content for create/update (database storage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeDesktopProviderContent {
    pub name: String,
    pub category: String,
    pub settings_config: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    pub is_applied: bool,
    pub is_disabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// ClaudeDesktopProvider - input from frontend (for create/update)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopProviderInput {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub category: String,
    pub settings_config: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

// ============================================================================
// Claude Desktop Prompt Config Types
// ============================================================================

/// ClaudeDesktopPromptConfig - input from the frontend (for create/update).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopPromptConfigInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub content: String,
}

/// ClaudeDesktopPromptConfig - API response, serialized camelCase to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopPromptConfig {
    pub id: String,
    pub name: String,
    pub content: String,
    pub is_applied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// ClaudeDesktopPromptConfig - content stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeDesktopPromptConfigContent {
    pub name: String,
    pub content: String,
    pub is_applied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================================
// Claude Desktop Common Config Types
// ============================================================================

/// ClaudeDesktopCommonConfig - API response. The stored `config` is the raw JSON
/// body of the Claude Desktop config (the non-managed base, e.g. `mcpServers`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopCommonConfig {
    pub config: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopCommonConfigInput {
    pub config: String,
}

// ============================================================================
// Claude Desktop Paths & Status
// ============================================================================

/// Disk paths for the Claude Desktop 3P profile, serialized for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopPathInfo {
    pub supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normal_config_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threep_config_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_library_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Status of the current Claude Desktop on-disk configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopStatus {
    pub supported: bool,
    pub configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_library_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<ClaudeDesktopMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_base_url: Option<String>,
}
