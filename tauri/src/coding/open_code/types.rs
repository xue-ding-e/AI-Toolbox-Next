use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::sql::Thing;

// ============================================================================
// OpenCode Common Config Types
// ============================================================================

/// OpenCodeCommonConfig - Database record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeCommonConfigRecord {
    pub id: Thing,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    pub updated_at: String,
}

/// OpenCodeCommonConfig - API response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeCommonConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    pub updated_at: String,
}

// ============================================================================
// OpenCode Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigPathInfo {
    pub path: String,
    pub source: String, // "custom" | "env" | "shell" | "default"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeModelLimit {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeModelModalities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<OpenCodeModelLimit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modalities: Option<OpenCodeModelModalities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variants: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeProviderOptions {
    #[serde(rename = "baseURL", skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(rename = "apiKey", skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<serde_json::Value>,
    #[serde(rename = "setCacheKey", skip_serializing_if = "Option::is_none")]
    pub set_cache_key: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeProvider {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<OpenCodeProviderOptions>,
    pub models: HashMap<String, OpenCodeModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeConfig {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<HashMap<String, OpenCodeProvider>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(rename = "small_model", skip_serializing_if = "Option::is_none")]
    pub small_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<Vec<String>>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

// ============================================================================
// Free Models Types
// ============================================================================

/// Free model information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeModel {
    pub id: String,
    pub name: String,
    pub provider_id: String,         // Config key (e.g., "opencode")
    pub provider_name: String,       // Display name (e.g., "OpenCode Zen")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<i64>,
}

/// Provider models data stored in database
/// Table: provider_models, Record ID: {provider_id} (e.g., "opencode")
/// Value: The complete JSON object for that provider from models.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelsData {
    pub provider_id: String,         // Provider ID (e.g., "opencode")
    pub value: serde_json::Value,    // Complete JSON from models.json for this provider
    pub updated_at: String,          // ISO 8601 timestamp
}

/// Provider models database record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderModelsRecord {
    pub id: Thing,
    pub data: ProviderModelsData,
}

/// Response for get_opencode_free_models command
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetFreeModelsResponse {
    pub free_models: Vec<FreeModel>,
    pub total: usize,
    pub from_cache: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>, // ISO 8601 timestamp (only if from_cache)
}
