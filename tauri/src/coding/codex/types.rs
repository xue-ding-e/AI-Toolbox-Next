use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

// ============================================================================
// Codex Provider Types
// ============================================================================

/// CodexProvider - Database record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexProviderRecord {
    pub id: Thing,
    pub provider_id: String,
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
    pub is_applied: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// CodexProvider - API response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexProvider {
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
    pub is_applied: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<CodexProviderRecord> for CodexProvider {
    fn from(record: CodexProviderRecord) -> Self {
        CodexProvider {
            id: record.provider_id,
            name: record.name,
            category: record.category,
            settings_config: record.settings_config,
            source_provider_id: record.source_provider_id,
            website_url: record.website_url,
            notes: record.notes,
            icon: record.icon,
            icon_color: record.icon_color,
            sort_index: record.sort_index,
            is_applied: record.is_applied,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

/// CodexProvider - Content for create/update (Database storage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexProviderContent {
    pub provider_id: String,
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
    pub is_applied: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// CodexProvider - Input from frontend (for create operation)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexProviderInput {
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
}

// ============================================================================
// Codex Common Config Types
// ============================================================================

/// CodexCommonConfig - Database record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexCommonConfigRecord {
    pub id: Thing,
    pub config: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// CodexCommonConfig - API response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCommonConfig {
    pub config: String,
    pub updated_at: String,
}

/// Codex settings structure (for reading/writing config files)
/// auth.json + config.toml combined
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
}
