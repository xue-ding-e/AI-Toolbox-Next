use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

// ============================================================================
// Provider Types
// ============================================================================

/// Provider - Database record (with Thing id from SurrealDB)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRecord {
    pub id: Thing,
    pub provider_id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_cache_key: Option<bool>,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// Provider - API response (with string id)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_cache_key: Option<bool>,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ProviderRecord> for Provider {
    fn from(record: ProviderRecord) -> Self {
        Provider {
            id: record.provider_id,
            name: record.name,
            provider_type: record.provider_type,
            base_url: record.base_url,
            api_key: record.api_key,
            headers: record.headers,
            timeout: record.timeout,
            set_cache_key: record.set_cache_key,
            sort_order: record.sort_order,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

/// Provider - Content for create/update (without Thing id)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderContent {
    pub provider_id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_cache_key: Option<bool>,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInput {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_cache_key: Option<bool>,
    pub sort_order: i32,
}

// ============================================================================
// Model Types
// ============================================================================

/// Model - Database record (with Thing id from SurrealDB)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRecord {
    pub id: Thing,
    pub model_id: String,
    pub provider_id: String,
    pub name: String,
    pub context_limit: i64,
    pub output_limit: i64,
    pub options: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variants: Option<String>,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// Model - API response (with string id)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub context_limit: i64,
    pub output_limit: i64,
    pub options: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variants: Option<String>,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ModelRecord> for Model {
    fn from(record: ModelRecord) -> Self {
        Model {
            id: record.model_id,
            provider_id: record.provider_id,
            name: record.name,
            context_limit: record.context_limit,
            output_limit: record.output_limit,
            options: record.options,
            variants: record.variants,
            sort_order: record.sort_order,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

/// Model - Content for create/update (without Thing id)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelContent {
    pub model_id: String,
    pub provider_id: String,
    pub name: String,
    pub context_limit: i64,
    pub output_limit: i64,
    pub options: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variants: Option<String>,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInput {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub context_limit: i64,
    pub output_limit: i64,
    pub options: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variants: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderWithModels {
    pub provider: Provider,
    pub models: Vec<Model>,
}
