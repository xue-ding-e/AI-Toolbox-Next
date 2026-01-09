use serde::{Deserialize, Serialize};
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

/// Input type for creating/updating config
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OhMyOpenCodeConfigInput {
    pub id: String,
    pub name: String,
    pub agents: HashMap<String, AgentConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sisyphus_agent: Option<SisyphusAgentConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_agents: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_mcps: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_hooks: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_commands: Option<Vec<String>>,
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

/// Oh My OpenCode configuration stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OhMyOpenCodeConfig {
    pub id: String,
    pub name: String,
    pub is_applied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub agents: HashMap<String, AgentConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sisyphus_agent: Option<SisyphusAgentConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_agents: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_mcps: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_hooks: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_commands: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Oh My OpenCode JSON file structure
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
    pub disabled_skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_commands: Option<Vec<String>>,
}

/// Oh My OpenCode configuration content for database storage (snake_case)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OhMyOpenCodeConfigContent {
    pub config_id: String,
    pub name: String,
    pub is_applied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub agents: HashMap<String, AgentConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sisyphus_agent: Option<SisyphusAgentConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_agents: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_mcps: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_hooks: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_commands: Option<Vec<String>>,
    pub created_at: String,
    pub updated_at: String,
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
            disabled_skills: None,
            disabled_commands: None,
        }
    }
}
