use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Config path info
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPathInfo {
    pub path: String,
    pub source: String,
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_append: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// Sisyphus agent specific configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SisyphusAgentConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_builder_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planner_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replace_plan: Option<bool>,
}

/// LSP Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspServerConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initialization: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
}

/// Experimental features configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentalConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preemptive_compaction_threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncate_all_tool_outputs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggressive_truncation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_resume: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dcp_for_compaction: Option<bool>,
}

/// Input type for creating/updating Agents Profile (简化版)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OhMyOpenCodeAgentsProfileInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>, // Optional - will be generated if not provided
    pub name: String,
    pub agents: HashMap<String, AgentConfig>,
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
    pub agents: HashMap<String, AgentConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Oh My OpenCode Agents Profile content for database storage (snake_case)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OhMyOpenCodeAgentsProfileContent {
    pub config_id: String,
    pub name: String,
    pub is_applied: bool,
    pub agents: HashMap<String, AgentConfig>,
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
    pub sisyphus_agent: Option<SisyphusAgentConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_agents: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_mcps: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_hooks: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsp: Option<HashMap<String, LspServerConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<ExperimentalConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
}

/// Oh My OpenCode Global Config stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OhMyOpenCodeGlobalConfig {
    pub id: String, // 固定为 "global"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sisyphus_agent: Option<SisyphusAgentConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_agents: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_mcps: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_hooks: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsp: Option<HashMap<String, LspServerConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<ExperimentalConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_fields: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Oh My OpenCode Global Config content for database storage (snake_case)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OhMyOpenCodeGlobalConfigContent {
    pub config_id: String, // 固定为 "global"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sisyphus_agent: Option<SisyphusAgentConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_agents: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_mcps: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_hooks: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsp: Option<HashMap<String, LspServerConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<ExperimentalConfig>,
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

/// Oh My OpenCode JSON file structure (写入文件时使用)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OhMyOpenCodeJsonConfig {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents: Option<HashMap<String, AgentConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sisyphus_agent: Option<SisyphusAgentConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_agents: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_mcps: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_hooks: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lsp: Option<HashMap<String, LspServerConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<ExperimentalConfig>,
}

impl Default for OhMyOpenCodeJsonConfig {
    fn default() -> Self {
        Self {
            schema: Some("https://raw.githubusercontent.com/code-yeongyu/oh-my-opencode/master/assets/oh-my-opencode.schema.json".to_string()),
            agents: None,
            sisyphus_agent: None,
            disabled_agents: None,
            disabled_mcps: None,
            disabled_hooks: None,
            lsp: None,
            experimental: None,
        }
    }
}
