use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// OpenClaw Config Types (mirrors ~/.openclaw/openclaw.json)
// ============================================================================

/// Cost per token for a model
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawModelCost {
    #[serde(default)]
    pub input: f64,
    #[serde(default)]
    pub output: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<f64>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// A single model entry within a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawModel {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<OpenClawModelCost>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// A provider configuration under models.providers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawProviderConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<OpenClawModel>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// The `models` top-level section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawModelsSection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub providers: Option<indexmap::IndexMap<String, OpenClawProviderConfig>>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Default model configuration under agents.defaults.model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawDefaultModel {
    pub primary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallbacks: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Model catalog entry under agents.defaults.models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawModelCatalogEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// The `agents.defaults` section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawAgentsDefaults {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<OpenClawDefaultModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<HashMap<String, OpenClawModelCatalogEntry>>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// The `agents` top-level section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawAgentsSection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defaults: Option<OpenClawAgentsDefaults>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// The `env` top-level section (flat key-value)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawEnvConfig {
    #[serde(flatten)]
    pub vars: HashMap<String, serde_json::Value>,
}

/// The `tools` top-level section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawToolsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Top-level OpenClaw configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<OpenClawModelsSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents: Option<OpenClawAgentsSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<OpenClawEnvConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<OpenClawToolsConfig>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

// ============================================================================
// Config path info
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawConfigPathInfo {
    pub path: String,
    pub source: String, // "custom" | "default"
}

// ============================================================================
// Read config result (tagged enum)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum ReadOpenClawConfigResult {
    Success { config: OpenClawConfig },
    NotFound { path: String },
    ParseError {
        path: String,
        error: String,
        content_preview: Option<String>,
    },
    Error { error: String },
}

// ============================================================================
// Common Config (stored in DB)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawCommonConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    pub updated_at: String,
}
