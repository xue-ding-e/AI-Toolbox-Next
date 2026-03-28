use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Config path info
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPathInfo {
    pub path: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OhMyOpenAgentLegacyUpgradeStatus {
    pub needs_upgrade: bool,
    pub has_legacy_plugin: bool,
    pub has_legacy_local_config: bool,
    pub has_legacy_custom_config_path: bool,
    pub has_legacy_wsl_mapping: bool,
    pub has_legacy_ssh_mapping: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_config_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OhMyOpenAgentLegacyUpgradeResult {
    pub changed: bool,
    pub plugin_updated: bool,
    pub local_config_renamed: bool,
    pub custom_config_path_updated: bool,
    pub wsl_mapping_updated: bool,
    pub wsl_file_renamed: bool,
    pub ssh_mapping_updated: bool,
}

/// Input type for creating/updating Agents Profile (简化版)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OhMyOpenAgentAgentsProfileInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>, // Optional - will be generated if not provided
    pub name: String,
    pub agents: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
}

/// Oh My OpenAgent Agents Profile stored in database (简化版)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OhMyOpenAgentAgentsProfile {
    pub id: String,
    pub name: String,
    pub is_applied: bool,
    pub is_disabled: bool,
    pub agents: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i32>, // For manual ordering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Oh My OpenAgent Agents Profile content for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OhMyOpenAgentAgentsProfileContent {
    pub name: String,
    pub is_applied: bool,
    pub is_disabled: bool,
    pub agents: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i32>, // For manual ordering
    pub created_at: String,
    pub updated_at: String,
}

/// Input type for Global Config
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OhMyOpenAgentGlobalConfigInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sisyphus_agent: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_agents: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_mcps: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_hooks: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsp: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_task: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_automation_engine: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_code: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
}

/// Oh My OpenAgent Global Config stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OhMyOpenAgentGlobalConfig {
    pub id: String, // 固定为 "global"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sisyphus_agent: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_agents: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_mcps: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_hooks: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsp: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_task: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_automation_engine: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_code: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Oh My OpenAgent Global Config content for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OhMyOpenAgentGlobalConfigContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sisyphus_agent: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_agents: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_mcps: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_hooks: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsp: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_task: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_automation_engine: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_code: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
    pub updated_at: String,
}

/// @deprecated 使用 OhMyOpenAgentAgentsProfileInput 代替
pub type OhMyOpenCodeAgentsProfileInput = OhMyOpenAgentAgentsProfileInput;

/// @deprecated 使用 OhMyOpenAgentAgentsProfile 代替
pub type OhMyOpenCodeAgentsProfile = OhMyOpenAgentAgentsProfile;

/// @deprecated 使用 OhMyOpenAgentAgentsProfileContent 代替
pub type OhMyOpenCodeAgentsProfileContent = OhMyOpenAgentAgentsProfileContent;

/// @deprecated 使用 OhMyOpenAgentAgentsProfileInput 代替
pub type OhMyOpenCodeConfigInput = OhMyOpenAgentAgentsProfileInput;

/// @deprecated 使用 OhMyOpenAgentAgentsProfile 代替
pub type OhMyOpenCodeConfig = OhMyOpenAgentAgentsProfile;

/// @deprecated 使用 OhMyOpenAgentAgentsProfileContent 代替
pub type OhMyOpenCodeConfigContent = OhMyOpenAgentAgentsProfileContent;

/// @deprecated 使用 OhMyOpenAgentGlobalConfigInput 代替
pub type OhMyOpenCodeGlobalConfigInput = OhMyOpenAgentGlobalConfigInput;

/// @deprecated 使用 OhMyOpenAgentGlobalConfig 代替
pub type OhMyOpenCodeGlobalConfig = OhMyOpenAgentGlobalConfig;

/// @deprecated 使用 OhMyOpenAgentGlobalConfigContent 代替
pub type OhMyOpenCodeGlobalConfigContent = OhMyOpenAgentGlobalConfigContent;

/// Input type for saving local config (both Agents Profile and Global Config)
/// Used when saving __local__ temporary config to database
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OhMyOpenAgentLocalConfigInput {
    /// Agents Profile config (optional, will be loaded from local file if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<OhMyOpenAgentAgentsProfileInput>,
    /// Global Config (optional, will be loaded from local file if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_config: Option<OhMyOpenAgentGlobalConfigInput>,
}

/// @deprecated 使用 OhMyOpenAgentLocalConfigInput 代替
pub type OhMyOpenCodeLocalConfigInput = OhMyOpenAgentLocalConfigInput;
