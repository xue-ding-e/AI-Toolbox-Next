//! Projection of AI Toolbox-managed providers into ZCode's
//! `v2/provider_config.json`.
//!
//! The file is shared: ZCode itself writes it, users may hand-edit it, and
//! other tools touch it. AI Toolbox owns only the provider entries whose
//! `providerId` it manages, so every write is a read-modify-write that removes
//! exactly the managed entries and preserves everything else verbatim.

use std::path::Path;

use serde_json::{json, Map, Value};

use super::constants::{
    ZCODE_ACCOUNT_PROVIDER_ID_PREFIX, ZCODE_BUILTIN_PROVIDER_ID_PREFIX,
    ZCODE_PERSONAL_PROVIDER_GROUP,
};
use super::types::{
    ZcodeModelRow, ZcodeModelRuleKind, ZcodeProviderConfig, ZcodeSettingsConfig,
};

/// Marker key ZCode expects for the personal provider group.
const GROUP_KEY: &str = "group";
const PROVIDER_RULES_KEY: &str = "providerRules";
const PROVIDER_MODEL_RULES_KEY: &str = "providerModelRules";
const MANUAL_PROVIDER_MODEL_RULES_KEY: &str = "manualProviderModelRules";
const PROVIDER_ORDER_KEY: &str = "providerOrder";
const DEFAULT_MODEL_SELECTION_KEY: &str = "defaultModelSelection";

/// A managed provider id that ZCode reserves and AI Toolbox must never rewrite.
pub fn is_reserved_provider_id(provider_id: &str) -> bool {
    provider_id.starts_with(ZCODE_ACCOUNT_PROVIDER_ID_PREFIX)
        || provider_id.starts_with(ZCODE_BUILTIN_PROVIDER_ID_PREFIX)
}

/// Reads the provider registry, returning a minimal skeleton when the file is
/// missing or malformed.
///
/// A malformed file is deliberately *not* treated as an error: ZCode recovers
/// from its own backups, and refusing to read would make the page unusable.
/// The skeleton keeps the version marker so a later write stays valid.
pub fn read_provider_config_base(path: &Path) -> Value {
    let Ok(text) = std::fs::read_to_string(path) else {
        return empty_provider_config();
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(value) if value.is_object() => value,
        _ => empty_provider_config(),
    }
}

pub fn empty_provider_config() -> Value {
    json!({
        "schemaVersion": 1,
        "config": {
            "providerOrder": [],
            "providerConfigRules": { "providerRules": [] },
            "modelConfigRules": {
                "providerModelRules": [],
                "manualProviderModelRules": [],
            },
        },
    })
}

/// Whether the new-generation registry exists. When it does not, ZCode is
/// still reading the legacy `v2/config.json` provider map and writing the new
/// file would have no effect.
pub fn provider_config_exists(path: &Path) -> bool {
    path.exists()
}

fn config_mut(base: &mut Value) -> Option<&mut Map<String, Value>> {
    base.get_mut("config")?.as_object_mut()
}

fn rules_array<'a>(config: &'a mut Map<String, Value>, key: &str) -> Option<&'a mut Vec<Value>> {
    config.get_mut(key)?.as_array_mut()
}

/// Removes every trace of `provider_id` from the registry.
///
/// Callers must invoke this before inserting the new entries so a model row
/// that moved between the smart and manual arrays does not end up in both.
pub fn remove_managed_provider(base: &mut Value, provider_id: &str) {
    let Some(config) = config_mut(base) else {
        return;
    };

    if let Some(rules) = config
        .get_mut("providerConfigRules")
        .and_then(|v| v.as_object_mut())
        .and_then(|v| rules_array(v, PROVIDER_RULES_KEY))
    {
        rules.retain(|rule| rule.get("providerId").and_then(Value::as_str) != Some(provider_id));
    }

    for key in [PROVIDER_MODEL_RULES_KEY, MANUAL_PROVIDER_MODEL_RULES_KEY] {
        if let Some(models) = config
            .get_mut("modelConfigRules")
            .and_then(|v| v.as_object_mut())
            .and_then(|v| rules_array(v, key))
        {
            models
                .retain(|rule| rule.get("providerId").and_then(Value::as_str) != Some(provider_id));
        }
    }

    if let Some(order) = rules_array(config, PROVIDER_ORDER_KEY) {
        order.retain(|entry| entry.as_str() != Some(provider_id));
    }
}

/// Builds the `providerRules[]` entry for a managed provider.
///
/// Only fields the user explicitly set are emitted. When `template_id` is
/// present the template supplies the rest, so expanding inherited values here
/// would freeze the provider against future catalog updates.
pub fn build_provider_rule(settings: &ZcodeSettingsConfig) -> Value {
    let mut rule = Map::new();
    rule.insert(
        "providerId".to_string(),
        Value::String(settings.provider_id.clone()),
    );
    if let Some(template_id) = settings.template_id.as_deref().filter(|v| !v.is_empty()) {
        rule.insert("templateId".to_string(), Value::String(template_id.to_string()));
    }
    if let Some(name) = settings.provider_name.as_deref().filter(|v| !v.is_empty()) {
        rule.insert("providerName".to_string(), Value::String(name.to_string()));
    }

    let mut config = Map::new();
    if let Some(source) = settings.config.as_ref() {
        // A personal provider must declare itself in the personal group;
        // `account:`/`builtin:` providers are rejected by ZCode when they do.
        config.insert(
            GROUP_KEY.to_string(),
            Value::String(ZCODE_PERSONAL_PROVIDER_GROUP.to_string()),
        );
        if let Some(logo) = source.logo.as_ref().filter(|logo| logo.key.is_some()) {
            if let Ok(value) = serde_json::to_value(logo) {
                config.insert("logo".to_string(), value);
            }
        }
        if let Some(access) = source.access.as_ref() {
            if let Ok(value) = serde_json::to_value(access) {
                config.insert("access".to_string(), value);
            }
        }
        if let Some(api) = source.api.as_ref() {
            if let Ok(value) = serde_json::to_value(api) {
                config.insert("api".to_string(), value);
            }
        }
        if let Some(visibility) = source.visibility.as_deref().filter(|v| !v.is_empty()) {
            config.insert(
                "visibility".to_string(),
                Value::String(visibility.to_string()),
            );
        }
        if let Some(models) = source
            .personal_model_ids
            .as_ref()
            .filter(|ids| !ids.is_empty())
        {
            config.insert(
                "personalModelIds".to_string(),
                Value::Array(models.iter().cloned().map(Value::String).collect()),
            );
        }
        if let Some(order) = source.model_order.as_ref().filter(|ids| !ids.is_empty()) {
            config.insert(
                "modelOrder".to_string(),
                Value::Array(order.iter().cloned().map(Value::String).collect()),
            );
        }
    }

    if !config.is_empty() {
        rule.insert("config".to_string(), Value::Object(config));
    }
    Value::Object(rule)
}

/// Builds the `modelConfigRules` entry for one model row.
pub fn build_model_rule(provider_id: &str, row: &ZcodeModelRow) -> Value {
    let mut config = Map::new();
    if let Some(enabled) = row.enabled {
        config.insert("enabled".to_string(), Value::Bool(enabled));
    }
    if let Some(properties) = row.properties.as_ref() {
        if let Ok(value) = serde_json::to_value(properties) {
            if value.as_object().is_some_and(|map| !map.is_empty()) {
                config.insert("properties".to_string(), value);
            }
        }
    }
    if let Some(option_specs) = row.option_specs.as_ref() {
        if let Ok(value) = serde_json::to_value(option_specs) {
            if value.as_object().is_some_and(|map| !map.is_empty()) {
                config.insert("optionSpecs".to_string(), value);
            }
        }
    }

    json!({
        "providerId": provider_id,
        "modelId": row.model_id,
        "config": Value::Object(config),
    })
}

/// Inserts a provider rule, replacing any existing entry with the same id.
pub fn upsert_provider_rule(base: &mut Value, rule: Value) {
    let Some(config) = config_mut(base) else {
        return;
    };
    let provider_id = rule.get("providerId").and_then(Value::as_str).map(str::to_string);
    if let Some(rules) = config
        .get_mut("providerConfigRules")
        .and_then(|v| v.as_object_mut())
        .and_then(|v| rules_array(v, PROVIDER_RULES_KEY))
    {
        rules.push(rule);
    }
    if let Some(provider_id) = provider_id {
        ensure_provider_order_entry(config, &provider_id);
    }
}

fn ensure_provider_order_entry(config: &mut Map<String, Value>, provider_id: &str) {
    let Some(order) = rules_array(config, PROVIDER_ORDER_KEY) else {
        return;
    };
    if !order.iter().any(|entry| entry.as_str() == Some(provider_id)) {
        order.push(Value::String(provider_id.to_string()));
    }
}

/// Appends model rules to the array matching each row's `ruleKind`.
///
/// Rows are assumed to have already been removed from both arrays by
/// [`remove_managed_provider`].
pub fn push_model_rules(base: &mut Value, provider_id: &str, rows: &[ZcodeModelRow]) {
    let Some(config) = config_mut(base) else {
        return;
    };
    let Some(model_config_rules) = config.get_mut("modelConfigRules").and_then(|v| v.as_object_mut())
    else {
        return;
    };

    for (key, kind) in [
        (PROVIDER_MODEL_RULES_KEY, ZcodeModelRuleKind::Smart),
        (MANUAL_PROVIDER_MODEL_RULES_KEY, ZcodeModelRuleKind::Manual),
    ] {
        let Some(array) = rules_array(model_config_rules, key) else {
            continue;
        };
        for row in rows.iter().filter(|row| row.rule_kind == kind) {
            array.push(build_model_rule(provider_id, row));
        }
    }
}

/// Points `defaultModelSelection` at `provider_id` / `model_id`.
pub fn set_default_model_selection(base: &mut Value, provider_id: &str, model_id: &str) {
    let Some(config) = config_mut(base) else {
        return;
    };
    config.insert(
        DEFAULT_MODEL_SELECTION_KEY.to_string(),
        json!({ "providerId": provider_id, "modelId": model_id }),
    );
}

/// Reads the currently selected provider id, if any.
pub fn read_default_provider_id(base: &Value) -> Option<String> {
    base.get("config")?
        .get(DEFAULT_MODEL_SELECTION_KEY)?
        .get("providerId")?
        .as_str()
        .map(str::to_string)
}

/// Reads the currently selected model id, if any.
pub fn read_default_model_id(base: &Value) -> Option<String> {
    base.get("config")?
        .get(DEFAULT_MODEL_SELECTION_KEY)?
        .get("modelId")?
        .as_str()
        .map(str::to_string)
}

/// Returns the ids of every provider present in the registry.
pub fn list_provider_ids(base: &Value) -> Vec<String> {
    base.get("config")
        .and_then(|config| config.get("providerConfigRules"))
        .and_then(|rules| rules.get(PROVIDER_RULES_KEY))
        .and_then(Value::as_array)
        .map(|rules| {
            rules
                .iter()
                .filter_map(|rule| rule.get("providerId").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Serializes and writes the registry atomically.
///
/// ZCode polls this file, so a partially written document would be observed.
pub fn atomic_write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut body = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    body.push('\n');
    crate::coding::mcp::yaml_sync::atomic_write_bytes(path, body.as_bytes())
}

/// Fills in the `personalModelIds`/`modelOrder` projection for a provider.
///
/// ZCode only surfaces models listed in `personalModelIds` for personal
/// providers, so the projection is derived from the row list rather than left
/// to the caller.
pub fn apply_personal_model_ids(settings: &mut ZcodeSettingsConfig) {
    if is_reserved_provider_id(&settings.provider_id) {
        return;
    }
    let ids: Vec<String> = settings
        .models
        .iter()
        .map(|row| row.model_id.clone())
        .collect();
    if ids.is_empty() {
        return;
    }
    let config = settings.config.get_or_insert_with(ZcodeProviderConfig::default);
    config.personal_model_ids = Some(ids.clone());
    config.model_order = Some(ids);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding::zcode::types::{ZcodeModelProperties, ZcodeProviderApi, ZcodeProviderAccess};

    fn sample_settings(provider_id: &str, model_id: &str) -> ZcodeSettingsConfig {
        ZcodeSettingsConfig {
            provider_id: provider_id.to_string(),
            provider_name: Some("My Provider".to_string()),
            template_id: None,
            config: Some(ZcodeProviderConfig {
                access: Some(ZcodeProviderAccess {
                    r#type: Some("api-key".to_string()),
                    api_key: Some("sk-test".to_string()),
                    ..Default::default()
                }),
                api: Some(ZcodeProviderApi {
                    r#type: Some("openai-chat-completions".to_string()),
                    base_url: Some("https://api.example.com/v1".to_string()),
                    headers: None,
                }),
                ..Default::default()
            }),
            models: vec![ZcodeModelRow {
                model_id: model_id.to_string(),
                display_name: None,
                rule_kind: ZcodeModelRuleKind::Smart,
                enabled: Some(true),
                properties: Some(ZcodeModelProperties {
                    context_window: Some(1_000_000),
                    ..Default::default()
                }),
                option_specs: None,
                is_default: true,
            }],
            default_model_id: None,
        }
    }

    #[test]
    fn empty_config_has_expected_shape() {
        let base = empty_provider_config();
        assert_eq!(base["schemaVersion"], json!(1));
        assert!(list_provider_ids(&base).is_empty());
        assert_eq!(read_default_provider_id(&base), None);
    }

    #[test]
    fn write_preserves_unmanaged_providers_and_unknown_fields() {
        let mut base = empty_provider_config();
        // A provider written by ZCode itself, plus an unknown future field.
        base["config"]["providerConfigRules"]["providerRules"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "providerId": "account:zai-start-plan", "providerName": "Start Plan" }));
        base["config"]["providerOrder"] = json!(["account:zai-start-plan", "hand-written"]);
        base["config"]["defaultModelSelection"] =
            json!({ "providerId": "account:zai-start-plan", "modelId": "GLM-5.3" });
        base["futureTopLevelKey"] = json!({ "keep": true });

        let mut settings = sample_settings("custom:mine", "deepseek-chat");
        apply_personal_model_ids(&mut settings);

        remove_managed_provider(&mut base, &settings.provider_id);
        upsert_provider_rule(&mut base, build_provider_rule(&settings));
        push_model_rules(&mut base, &settings.provider_id, &settings.models);

        let ids = list_provider_ids(&base);
        assert!(ids.contains(&"account:zai-start-plan".to_string()));
        assert!(ids.contains(&"custom:mine".to_string()));
        assert_eq!(base["futureTopLevelKey"]["keep"], json!(true));
        // Untouched selection and hand-written ordering entry survive.
        assert_eq!(
            read_default_provider_id(&base).as_deref(),
            Some("account:zai-start-plan")
        );
        let order = base["config"]["providerOrder"].as_array().unwrap();
        assert!(order.contains(&json!("hand-written")));
        assert!(order.contains(&json!("custom:mine")));
    }

    #[test]
    fn removing_a_provider_clears_both_model_arrays() {
        let mut base = empty_provider_config();
        let mut settings = sample_settings("custom:mine", "model-a");
        settings.models.push(ZcodeModelRow {
            model_id: "model-b".to_string(),
            display_name: None,
            rule_kind: ZcodeModelRuleKind::Manual,
            enabled: None,
            properties: None,
            option_specs: None,
            is_default: false,
        });
        apply_personal_model_ids(&mut settings);

        remove_managed_provider(&mut base, &settings.provider_id);
        upsert_provider_rule(&mut base, build_provider_rule(&settings));
        push_model_rules(&mut base, &settings.provider_id, &settings.models);

        let smart = base["config"]["modelConfigRules"]["providerModelRules"]
            .as_array()
            .unwrap();
        let manual = base["config"]["modelConfigRules"]["manualProviderModelRules"]
            .as_array()
            .unwrap();
        assert_eq!(smart.len(), 1);
        assert_eq!(manual.len(), 1);
        assert_eq!(smart[0]["modelId"], json!("model-a"));
        assert_eq!(manual[0]["modelId"], json!("model-b"));

        // Re-projecting after a rule-kind switch must not duplicate the row.
        let mut switched = settings.clone();
        switched.models[0].rule_kind = ZcodeModelRuleKind::Manual;
        remove_managed_provider(&mut base, &switched.provider_id);
        push_model_rules(&mut base, &switched.provider_id, &switched.models);

        let smart = base["config"]["modelConfigRules"]["providerModelRules"]
            .as_array()
            .unwrap();
        let manual = base["config"]["modelConfigRules"]["manualProviderModelRules"]
            .as_array()
            .unwrap();
        assert!(smart.is_empty(), "smart array should no longer hold model-a");
        assert_eq!(manual.len(), 2);
    }

    #[test]
    fn provider_rule_omits_unset_optional_fields() {
        let mut settings = sample_settings("custom:mine", "model-a");
        settings.template_id = Some("deepseek".to_string());
        let rule = build_provider_rule(&settings);
        assert_eq!(rule["templateId"], json!("deepseek"));
        // Never emit builtinModelIds for a personal provider: ZCode rejects it.
        assert!(rule["config"].get("builtinModelIds").is_none());
    }

    #[test]
    fn model_rule_keeps_map_expression_verbatim() {
        let mut settings = sample_settings("custom:mine", "model-a");
        settings.models[0].option_specs = Some(super::super::types::ZcodeModelOptionSpecs {
            max_output_tokens: Some(super::super::types::ZcodeMaxOutputTokensSpec {
                max: Some(128_000),
                map: Some("{\"max_tokens\": maxOutputTokens}".to_string()),
            }),
            reasoning_level: Some(super::super::types::ZcodeReasoningLevelSpec {
                values: Some(vec!["low".to_string(), "high".to_string()]),
                map: Some("{\"reasoning_effort\": reasoningLevel}".to_string()),
            }),
        });
        let rule = build_model_rule(&settings.provider_id, &settings.models[0]);
        assert_eq!(
            rule["config"]["optionSpecs"]["maxOutputTokens"]["map"],
            json!("{\"max_tokens\": maxOutputTokens}")
        );
    }
}
