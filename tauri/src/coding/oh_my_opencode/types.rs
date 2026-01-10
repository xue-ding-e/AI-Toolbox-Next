use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Config path info
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPathInfo {
    pub path: String,
    pub source: String,
}

/// Input type for creating/updating Agents Profile (简化版)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OhMyOpenCodeAgentsProfileInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>, // Optional - will be generated if not provided
    pub name: String,
    pub agents: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
}

/// Oh My OpenCode Agents Profile stored in database (简化版)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OhMyOpenCodeAgentsProfile {
    pub id: String,
    pub name: String,
    pub is_applied: bool,
    pub agents: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Oh My OpenCode Agents Profile content for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OhMyOpenCodeAgentsProfileContent {
    pub config_id: String,
    pub name: String,
    pub is_applied: bool,
    pub agents: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
}

/// Input type for Global Config
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OhMyOpenCodeGlobalConfigInput {
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
    pub lsp: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
}

/// Oh My OpenCode Global Config stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OhMyOpenCodeGlobalConfig {
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
    pub lsp: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Oh My OpenCode Global Config content for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OhMyOpenCodeGlobalConfigContent {
    pub config_id: String, // 固定为 "global"
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
    pub lsp: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<Value>, // JSON, no specific structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
    pub updated_at: String,
}

/// @deprecated 使用 OhMyOpenCodeAgentsProfileInput 代替
pub type OhMyOpenCodeConfigInput = OhMyOpenCodeAgentsProfileInput;

/// @deprecated 使用 OhMyOpenCodeAgentsProfile 代替
pub type OhMyOpenCodeConfig = OhMyOpenCodeAgentsProfile;

/// @deprecated 使用 OhMyOpenCodeAgentsProfileContent 代替
pub type OhMyOpenCodeConfigContent = OhMyOpenCodeAgentsProfileContent;
