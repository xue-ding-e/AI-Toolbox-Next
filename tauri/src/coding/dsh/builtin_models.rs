//! Bundled pi-ai adapter default model catalog (dsh_builtin_models.json).
//!
//! Mirrors the official deepseek-harness `llm-pi-ai` behavior where a provider
//! route the adapter ships serves its installed catalog models when
//! `settings.yaml` declares no explicit `models` ("using the adapter's default
//! models"). The data is copied from `@earendil-works/pi-ai`'s built-in
//! provider catalog so the app can show and edit those defaults without the
//! deepseek-harness runtime present.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::{Map, Value};

const DSH_BUILTIN_MODELS_JSON: &str = include_str!("../../../resources/dsh_builtin_models.json");

/// Whether the bundled catalog describes this provider route.
pub fn has_builtin_models(provider: &str) -> bool {
    builtin_models_for(provider).is_some()
}

/// The installed-catalog default models for one provider route.
///
/// Matches the provider key exactly; a `-official` suffixed route (e.g. a
/// user-configured `deepseek-official`) falls back to its plain catalog id so it
/// still serves the adapter default models, mirroring the official DeepSeek
/// channel's route naming.
pub fn builtin_models_for(provider: &str) -> Option<&[Value]> {
    catalog()
        .get(provider)
        .or_else(|| {
            provider
                .strip_suffix("-official")
                .and_then(|plain| catalog().get(plain))
        })
        .map(Vec::as_slice)
}

/// Model ids of a bundled catalog slice, for view `model_ids`.
pub fn builtin_model_ids(models: &[Value]) -> Vec<String> {
    models
        .iter()
        .filter_map(|model| model.get("id").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

type Catalog = HashMap<String, Vec<Value>>;

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let Ok(map) = serde_json::from_str::<Map<String, Value>>(DSH_BUILTIN_MODELS_JSON) else {
            return Catalog::new();
        };
        map.into_iter()
            .filter_map(|(provider, models)| {
                models.as_array().map(|array| (provider, array.clone()))
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_parses_and_serves_deepseek() {
        assert!(has_builtin_models("deepseek"));
        let models = builtin_models_for("deepseek").expect("deepseek catalog");
        assert!(!models.is_empty());
        let ids = builtin_model_ids(models);
        assert!(ids.iter().any(|id| id == "deepseek-v4-flash"));
        assert!(ids.iter().any(|id| id == "deepseek-v4-pro"));
    }

    #[test]
    fn bundled_catalog_ignores_unknown_routes() {
        assert!(!has_builtin_models("not-a-catalog-route"));
        assert!(builtin_models_for("not-a-catalog-route").is_none());
    }
}
