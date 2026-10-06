//! Preset model lookup for Kimi providers.
//!
//! Kimi Code CLI's `provider catalog add` imports provider + model entries from
//! the public models.dev catalog. ai-toolbox bundles the same catalog
//! (`tauri/resources/models.dev.json`, shared with the OpenCode module), so a
//! provider whose `base_url` matches a catalog entry can offer that entry's
//! models without any network round trip.

use serde::Serialize;
use serde_json::Value;

use crate::coding::open_code::free_models;

/// One catalog model, reduced to the fields the Kimi model catalog consumes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KimiPresetModel {
    /// Upstream model id (`models.<id>` in the catalog).
    pub id: String,
    /// Display name the catalog declares, when present.
    pub display_name: Option<String>,
    /// `limit.context` — feeds `max_context_size`.
    pub max_context_size: Option<i64>,
    /// `limit.output` — feeds `max_output_size`.
    pub max_output_size: Option<i64>,
    /// Whether the catalog marks the model as reasoning-capable.
    pub reasoning: bool,
    /// Input modalities, mapped to Kimi capability names by the frontend.
    pub input_modalities: Vec<String>,
    /// Output modalities, mapped to Kimi capability names by the frontend.
    pub output_modalities: Vec<String>,
}

/// Normalize a base URL for catalog comparison: lowercase, no scheme, no
/// trailing slash. The catalog stores bare hosts (`api.moonshot.ai/v1`).
fn normalize_base_url(raw: &str) -> String {
    let trimmed = raw.trim().to_ascii_lowercase();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(&trimmed);
    without_scheme.trim_end_matches('/').to_string()
}

/// Host portion of a normalized base URL, used as the fuzzy-match key.
fn host_of(normalized: &str) -> &str {
    normalized.split('/').next().unwrap_or(normalized)
}

fn read_modalities(value: &Value, key: &str) -> Vec<String> {
    value
        .get("modalities")
        .and_then(|modalities| modalities.get(key))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_preset_models(provider: &Value) -> Vec<KimiPresetModel> {
    let Some(models) = provider.get("models").and_then(Value::as_object) else {
        return Vec::new();
    };
    models
        .iter()
        .map(|(id, model)| KimiPresetModel {
            id: id.clone(),
            display_name: model
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string),
            max_context_size: model
                .pointer("/limit/context")
                .and_then(Value::as_i64)
                .filter(|size| *size > 0),
            max_output_size: model
                .pointer("/limit/output")
                .and_then(Value::as_i64)
                .filter(|size| *size > 0),
            reasoning: model.get("reasoning").and_then(Value::as_bool).unwrap_or(false),
            input_modalities: read_modalities(model, "input"),
            output_modalities: read_modalities(model, "output"),
        })
        .collect()
}

/// Look up the bundled catalog entry matching `base_url`.
///
/// Exact match on the normalized URL wins; otherwise the first entry sharing the
/// host is used (so `api.moonshot.cn/v1` still finds `moonshotai-cn`). Returns an
/// empty list when nothing matches — callers show no preset hint rather than a
/// wrong one, so there is deliberately no name-based fallback.
pub fn preset_models_for_base_url(base_url: &str) -> Vec<KimiPresetModel> {
    let normalized = normalize_base_url(base_url);
    if normalized.is_empty() {
        return Vec::new();
    }
    let Some(catalog) = free_models::bundled_models_dev_catalog() else {
        return Vec::new();
    };
    let Some(entries) = catalog.as_object() else {
        return Vec::new();
    };

    let host = host_of(&normalized);
    let mut host_match: Option<&Value> = None;
    for entry in entries.values() {
        let Some(api) = entry.get("api").and_then(Value::as_str) else {
            continue;
        };
        let candidate = normalize_base_url(api);
        if candidate == normalized {
            return parse_preset_models(entry);
        }
        if host_match.is_none() && host_of(&candidate) == host {
            host_match = Some(entry);
        }
    }
    host_match.map(parse_preset_models).unwrap_or_default()
}

/// Tauri command: preset catalog models for a provider base URL.
#[tauri::command]
pub async fn get_kimi_preset_models(base_url: String) -> Result<Vec<KimiPresetModel>, String> {
    Ok(preset_models_for_base_url(&base_url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_base_url_strips_scheme_and_trailing_slash() {
        assert_eq!(normalize_base_url("HTTPS://API.Moonshot.AI/v1/"), "api.moonshot.ai/v1");
        assert_eq!(normalize_base_url("  http://api.moonshot.cn/v1  "), "api.moonshot.cn/v1");
        assert_eq!(normalize_base_url("api.kimi.com/coding/v1"), "api.kimi.com/coding/v1");
        assert_eq!(normalize_base_url("   "), "");
    }

    #[test]
    fn preset_lookup_matches_moonshot_catalog_entry() {
        // The bundled catalog carries `moonshotai`; its models must come back
        // with the context/output limits the CLI projects.
        let models = preset_models_for_base_url("https://api.moonshot.ai/v1");
        assert!(!models.is_empty(), "expected bundled moonshotai models");
        assert!(models.iter().any(|model| model.id.starts_with("kimi-")));
        assert!(models.iter().all(|model| model.max_context_size.is_some()));
    }

    #[test]
    fn preset_lookup_falls_back_to_host_match() {
        // A path the catalog does not declare still resolves through the host.
        let models = preset_models_for_base_url("https://api.moonshot.ai/v1/extra");
        assert!(!models.is_empty());
    }

    #[test]
    fn preset_lookup_returns_empty_for_unknown_or_blank_base_url() {
        assert!(preset_models_for_base_url("https://relay.example.com/v1").is_empty());
        assert!(preset_models_for_base_url("").is_empty());
        assert!(preset_models_for_base_url("   ").is_empty());
    }
}
