use crate::coding::proxy_gateway::types::GatewayCliKey;
use crate::coding::{claude_code, codex, gemini_cli};
use serde_json::Value;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;
use toml_edit::{DocumentMut, Item};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UpstreamProvider {
    pub(super) cli_key: GatewayCliKey,
    pub(super) id: String,
    pub(super) name: String,
    pub(super) base_url: String,
    pub(super) api_key: String,
    pub(super) is_applied: bool,
    pub(super) sort_index: Option<i32>,
}

pub(super) async fn load_candidate_providers(
    db: &Surreal<Db>,
    cli_key: GatewayCliKey,
) -> Result<Vec<UpstreamProvider>, String> {
    let table = match cli_key {
        GatewayCliKey::Claude => "claude_provider",
        GatewayCliKey::Codex => "codex_provider",
        GatewayCliKey::Gemini => "gemini_cli_provider",
        GatewayCliKey::OpenCode => {
            return Err(
                "OpenCode adapter is intentionally out of scope for the gateway MVP".to_string(),
            )
        }
    };
    let mut result = db
        .query(format!(
            "SELECT *, type::string(id) as id FROM {table} ORDER BY sort_index ASC, updated_at DESC"
        ))
        .await
        .map_err(|error| {
            format!(
                "Failed to query providers for {}: {error}",
                cli_key.as_str()
            )
        })?;
    let records: Vec<Value> = result.take(0).map_err(|error| {
        format!(
            "Failed to parse providers for {}: {error}",
            cli_key.as_str()
        )
    })?;

    let mut providers = Vec::new();
    let mut parse_errors = Vec::new();
    for record in records {
        match provider_from_record(cli_key, record) {
            Ok(Some(provider)) => providers.push(provider),
            Ok(None) => {}
            Err(error) => parse_errors.push(error),
        }
    }
    providers.sort_by(|left, right| {
        right
            .is_applied
            .cmp(&left.is_applied)
            .then_with(|| {
                left.sort_index
                    .unwrap_or(i32::MAX)
                    .cmp(&right.sort_index.unwrap_or(i32::MAX))
            })
            .then_with(|| left.name.cmp(&right.name))
    });

    if providers.is_empty() && !parse_errors.is_empty() {
        return Err(parse_errors.join("; "));
    }

    Ok(providers)
}

fn provider_from_record(
    cli_key: GatewayCliKey,
    record: Value,
) -> Result<Option<UpstreamProvider>, String> {
    match cli_key {
        GatewayCliKey::Claude => {
            let provider = claude_code::adapter::from_db_value_provider(record);
            if provider.is_disabled {
                return Ok(None);
            }
            let settings =
                parse_json_config(&provider.settings_config, "Claude provider settings_config")?;
            let env = settings.get("env").and_then(Value::as_object);
            let base_url = json_object_string(env, "ANTHROPIC_BASE_URL")
                .unwrap_or_else(|| "https://api.anthropic.com".to_string());
            let api_key = json_object_string(env, "ANTHROPIC_AUTH_TOKEN")
                .or_else(|| json_object_string(env, "ANTHROPIC_API_KEY"))
                .ok_or_else(|| {
                    format!("Applied Claude provider '{}' has no API key", provider.name)
                })?;
            Ok(Some(UpstreamProvider {
                cli_key,
                id: provider.id,
                name: provider.name,
                base_url,
                api_key,
                is_applied: provider.is_applied,
                sort_index: provider.sort_index,
            }))
        }
        GatewayCliKey::Codex => {
            let provider = codex::adapter::from_db_value_provider(record);
            if provider.is_disabled {
                return Ok(None);
            }
            let settings =
                parse_json_config(&provider.settings_config, "Codex provider settings_config")?;
            let auth = settings.get("auth").and_then(Value::as_object);
            let api_key = json_object_string(auth, "OPENAI_API_KEY").ok_or_else(|| {
                format!(
                    "Applied Codex provider '{}' has no OPENAI_API_KEY",
                    provider.name
                )
            })?;
            let config_toml = settings.get("config").and_then(Value::as_str).unwrap_or("");
            let base_url = codex_base_url_from_config(config_toml)
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            Ok(Some(UpstreamProvider {
                cli_key,
                id: provider.id,
                name: provider.name,
                base_url,
                api_key,
                is_applied: provider.is_applied,
                sort_index: provider.sort_index,
            }))
        }
        GatewayCliKey::Gemini => {
            let provider = gemini_cli::adapter::from_db_value_provider(record);
            if provider.is_disabled {
                return Ok(None);
            }
            let settings = parse_json_config(
                &provider.settings_config,
                "Gemini CLI provider settings_config",
            )?;
            let env = settings.get("env").and_then(Value::as_object);
            let api_key = json_object_string(env, "GEMINI_API_KEY")
                .or_else(|| json_object_string(env, "GOOGLE_API_KEY"))
                .ok_or_else(|| {
                    format!(
                        "Applied Gemini CLI provider '{}' has no API key",
                        provider.name
                    )
                })?;
            let base_url = json_object_string(env, "GOOGLE_GEMINI_BASE_URL")
                .or_else(|| json_object_string(env, "GOOGLE_VERTEX_BASE_URL"))
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".to_string());
            Ok(Some(UpstreamProvider {
                cli_key,
                id: provider.id,
                name: provider.name,
                base_url,
                api_key,
                is_applied: provider.is_applied,
                sort_index: provider.sort_index,
            }))
        }
        GatewayCliKey::OpenCode => unreachable!("OpenCode is rejected before query"),
    }
}

fn parse_json_config(raw: &str, label: &str) -> Result<Value, String> {
    serde_json::from_str(raw).map_err(|error| format!("Failed to parse {label}: {error}"))
}

pub(super) fn json_object_string(
    object: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Option<String> {
    object
        .and_then(|object| object.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn codex_base_url_from_config(config_toml: &str) -> Option<String> {
    let trimmed = config_toml.trim();
    if trimmed.is_empty() {
        return None;
    }
    let document = trimmed.parse::<DocumentMut>().ok()?;
    let root = document.as_table();
    let providers = root.get("model_providers")?.as_table()?;
    let selected_provider = root
        .get("model_provider")
        .and_then(Item::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(selected_provider) = selected_provider {
        if let Some(base_url) = providers
            .get(selected_provider)
            .and_then(Item::as_table)
            .and_then(|provider| provider.get("base_url"))
            .and_then(Item::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(base_url.to_string());
        }
    }

    let fallback = providers.iter().find_map(|(_, item)| {
        item.as_table()
            .and_then(|provider| provider.get("base_url"))
            .and_then(Item::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    });
    fallback
}
