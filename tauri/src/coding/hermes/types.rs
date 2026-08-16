use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesPathInfo {
    pub path: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesSettingsConfigRecord {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesSettingsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_dir: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesSettingsConfigInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_dir: Option<String>,
    #[serde(default)]
    pub clear_config_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesPromptConfigRecord {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesPromptConfig {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesPromptConfigContent {
    pub name: String,
    pub content: String,
    pub is_applied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesPromptConfigInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesBuiltinProvider {
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HermesProviderWarning {
    MissingProvider,
    MissingModel,
}

/// A single Hermes provider merged from `custom_providers` (list, writable),
/// the `providers:` dict (read-only overlay) or the default provider record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesRuntimeProviderView {
    pub provider_key: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_mode: Option<String>,
    /// Raw provider JSON (custom_providers entry or providers dict entry),
    /// with its `models` dict denormalized to a UI-friendly array.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_ids: Vec<String>,
    pub is_builtin: bool,
    /// True when the provider only exists in the read-only `providers:` dict.
    pub is_read_only: bool,
    pub is_default: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<HermesProviderWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesRuntimeConfig {
    pub root_path_info: HermesPathInfo,
    pub config_path: String,
    pub prompt_path: String,
    /// Raw `config.yaml` content as a JSON object (unknown top-level keys pass
    /// through untouched).
    pub config: Value,
    pub model_settings: HermesModelSettingsInput,
    /// Everything except the managed keys (`model`, `custom_providers`,
    /// `providers`, `mcp_servers`, `_config_version`).
    pub other_settings: Value,
    pub providers: Vec<HermesRuntimeProviderView>,
    pub builtin_providers: Vec<HermesBuiltinProvider>,
    /// Raw `config.yaml` file content for file-based preview.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_content: Option<String>,
    /// Raw prompt file content for file-based preview.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_content: Option<String>,
}

/// Top-level `model:` section, also used as the save input.
///
/// String fields follow pi's clear semantics: `Some("")` removes the key,
/// `Some(value)` writes it, `None` leaves it untouched. Numeric fields are
/// removed when `clear_context_length` / `clear_max_tokens` is set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesModelSettingsInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    #[serde(default)]
    pub clear_context_length: bool,
    #[serde(default)]
    pub clear_max_tokens: bool,
}

/// Upsert a single `custom_providers` entry, keyed by `name`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesModelsProviderInput {
    pub provider_key: String,
    pub provider: Value,
}

/// Which of Hermes' two memory blobs to operate on. Deserialized from the
/// `"memory"` / `"user"` strings the frontend sends, rejected at the IPC
/// boundary otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HermesMemoryKind {
    Memory,
    User,
}

/// Character budgets + enable flags for Hermes' two memory blobs, read from
/// the top-level `memory:` section of `config.yaml`. Defaults mirror Hermes'
/// own so the UI is usable even before the user edits config.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HermesMemoryLimits {
    pub memory: usize,
    pub user: usize,
    pub memory_enabled: bool,
    pub user_enabled: bool,
}
