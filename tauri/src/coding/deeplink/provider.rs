//! Per-app `settings_config` builders + dispatch to the refactored
//! `create_*_provider_inner` functions.
//!
//! Each builder produces the JSON **string** that the tool's
//! `*ProviderInput::settings_config` field expects — the same shape that
//! `cc_switch::extract_*_candidate` produces (see `cc_switch.rs:175-477`).
//! `config`/`extra` URL params act as escape hatches that override the
//! builder's output verbatim.

use serde::Serialize;
use serde_json::{json, Map, Value};
use tauri::AppHandle;

use crate::coding::claude_code::commands::create_claude_provider_inner;
use crate::coding::claude_code::types::ClaudeCodeProviderInput;
use crate::coding::codex::commands::create_codex_provider_inner;
use crate::coding::codex::types::CodexProviderInput;
use crate::coding::gemini_cli::commands::create_gemini_cli_provider_inner;
use crate::coding::gemini_cli::types::GeminiCliProviderInput;
use crate::db::SqliteDbState;

use super::parser::DeepLinkImportRequest;

/// Answer returned to the frontend after a successful import. The frontend
/// uses `app` to dispatch a per-tool page refresh and `id` for any follow-up.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepLinkImportResult {
    #[serde(rename = "type")]
    pub kind: String,
    pub app: String,
    pub id: String,
}

/// Build the provider's `settings_config` (and Claude's
/// `extra_settings_config`) and persist via the matching
/// `create_*_provider_inner`. Dispatches by `request.app`.
pub async fn build_and_create_provider(
    state: &SqliteDbState,
    app: &AppHandle,
    request: &DeepLinkImportRequest,
) -> Result<DeepLinkImportResult, String> {
    match request.app.as_str() {
        "claude" => {
            let (settings_config, extra_settings_config) = build_claude_settings(request)?;
            let input = ClaudeCodeProviderInput {
                id: None,
                name: request.name.clone(),
                category: request.category.clone(),
                settings_config,
                extra_settings_config,
                extra_settings_merge_strategy: None,
                source_provider_id: request.source_provider_id.clone(),
                website_url: request.homepage.clone(),
                notes: request.notes.clone(),
                icon: request.icon.clone(),
                icon_color: request.icon_color.clone(),
                sort_index: None,
                meta: None,
            };
            let provider = create_claude_provider_inner(state, app, input).await?;
            Ok(DeepLinkImportResult {
                kind: "provider".to_string(),
                app: "claude".to_string(),
                id: provider.id,
            })
        }
        "codex" => {
            let settings_config = build_codex_settings(request)?;
            let input = CodexProviderInput {
                id: None,
                name: request.name.clone(),
                category: request.category.clone(),
                settings_config,
                source_provider_id: request.source_provider_id.clone(),
                website_url: request.homepage.clone(),
                notes: request.notes.clone(),
                icon: request.icon.clone(),
                icon_color: request.icon_color.clone(),
                sort_index: None,
                meta: None,
                is_disabled: None,
            };
            let provider = create_codex_provider_inner(state, app, input).await?;
            Ok(DeepLinkImportResult {
                kind: "provider".to_string(),
                app: "codex".to_string(),
                id: provider.id,
            })
        }
        "gemini" => {
            let settings_config = build_gemini_settings(request)?;
            let input = GeminiCliProviderInput {
                id: None,
                name: request.name.clone(),
                category: request.category.clone(),
                settings_config,
                source_provider_id: request.source_provider_id.clone(),
                website_url: request.homepage.clone(),
                notes: request.notes.clone(),
                icon: request.icon.clone(),
                icon_color: request.icon_color.clone(),
                sort_index: None,
                meta: None,
                is_disabled: None,
            };
            let provider = create_gemini_cli_provider_inner(state, app, input).await?;
            Ok(DeepLinkImportResult {
                kind: "provider".to_string(),
                app: "gemini".to_string(),
                id: provider.id,
            })
        }
        // `grok` is deferred — its settings shape (defaultModelKey +
        // modelCatalog) is materially more complex and has no precedent in
        // cc_switch.rs. The parser already rejects it as `UnsupportedApp`,
        // so this arm is unreachable in v1; kept for completeness.
        other => Err(format!("deep-link: app '{other}' is not supported in v1")),
    }
}

/// `{"env": {ANTHROPIC_AUTH_TOKEN?, ANTHROPIC_BASE_URL?, ANTHROPIC_MODEL?}}`.
/// If `config` is provided, it overrides the built `settings_config` verbatim.
/// `extra` (decoded) becomes `extra_settings_config`, defaulting to `"{}"`.
fn build_claude_settings(
    request: &DeepLinkImportRequest,
) -> Result<(String, Option<String>), String> {
    let settings_config = match request.config.as_deref() {
        Some(cfg) if !cfg.trim().is_empty() => cfg.to_string(),
        _ => {
            let mut env = Map::new();
            if let Some(api_key) = &request.api_key {
                env.insert(
                    "ANTHROPIC_AUTH_TOKEN".to_string(),
                    Value::String(api_key.clone()),
                );
            }
            if let Some(base_url) = &request.base_url {
                env.insert(
                    "ANTHROPIC_BASE_URL".to_string(),
                    Value::String(base_url.clone()),
                );
            }
            if let Some(model) = &request.model {
                env.insert("ANTHROPIC_MODEL".to_string(), Value::String(model.clone()));
            }
            serde_json::to_string(&json!({ "env": Value::Object(env) }))
                .map_err(|e| format!("deep-link: failed to serialize claude settings: {e}"))?
        }
    };

    let extra_settings_config = match request.extra.as_deref() {
        Some(extra) if !extra.trim().is_empty() => Some(extra.to_string()),
        _ => Some("{}".to_string()),
    };

    Ok((settings_config, extra_settings_config))
}

/// `{"auth": {"OPENAI_API_KEY"?}, "config": "<TOML string>"}`.
/// The TOML contains `model_provider = "<slug>"`, optional `model`, and a
/// `[model_providers.<slug>]` table with `name`/`base_url`. If `config` is
/// provided, it is used as the TOML `config` string verbatim.
fn build_codex_settings(request: &DeepLinkImportRequest) -> Result<String, String> {
    let slug = slugify(&request.name);

    let config_toml = match request.config.as_deref() {
        Some(cfg) if !cfg.trim().is_empty() => cfg.to_string(),
        _ => {
            let mut root = toml::map::Map::new();
            root.insert(
                "model_provider".to_string(),
                toml::Value::String(slug.clone()),
            );
            if let Some(model) = &request.model {
                root.insert("model".to_string(), toml::Value::String(model.clone()));
            }

            let mut provider_table = toml::map::Map::new();
            provider_table.insert(
                "name".to_string(),
                toml::Value::String(request.name.clone()),
            );
            if let Some(base_url) = &request.base_url {
                provider_table.insert(
                    "base_url".to_string(),
                    toml::Value::String(base_url.clone()),
                );
            }
            let mut model_providers = toml::map::Map::new();
            model_providers.insert(slug, toml::Value::Table(provider_table));
            root.insert(
                "model_providers".to_string(),
                toml::Value::Table(model_providers),
            );

            toml::to_string(&toml::Value::Table(root))
                .map_err(|e| format!("deep-link: failed to serialize codex TOML: {e}"))?
        }
    };

    let mut auth = Map::new();
    if let Some(api_key) = &request.api_key {
        auth.insert("OPENAI_API_KEY".to_string(), Value::String(api_key.clone()));
    }

    serde_json::to_string(&json!({
        "auth": Value::Object(auth),
        "config": config_toml,
    }))
    .map_err(|e| format!("deep-link: failed to serialize codex settings: {e}"))
}

/// `{"env": {GEMINI_API_KEY?, GOOGLE_GEMINI_BASE_URL?, GEMINI_MODEL?}, "config": {}}`.
/// If `config` is provided, it overrides the built `settings_config` verbatim.
fn build_gemini_settings(request: &DeepLinkImportRequest) -> Result<String, String> {
    let settings_config = match request.config.as_deref() {
        Some(cfg) if !cfg.trim().is_empty() => cfg.to_string(),
        _ => {
            let mut env = Map::new();
            if let Some(api_key) = &request.api_key {
                env.insert("GEMINI_API_KEY".to_string(), Value::String(api_key.clone()));
            }
            if let Some(base_url) = &request.base_url {
                env.insert(
                    "GOOGLE_GEMINI_BASE_URL".to_string(),
                    Value::String(base_url.clone()),
                );
            }
            if let Some(model) = &request.model {
                env.insert("GEMINI_MODEL".to_string(), Value::String(model.clone()));
            }
            serde_json::to_string(&json!({
                "env": Value::Object(env),
                "config": Value::Object(Map::new()),
            }))
            .map_err(|e| format!("deep-link: failed to serialize gemini settings: {e}"))?
        }
    };
    Ok(settings_config)
}

/// Lowercase, replace non-[a-z0-9] runs with `-`, trim leading/trailing `-`.
/// Used as the codex `model_provider` id and `[model_providers.<id>]` key.
fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = true; // suppress leading dashes
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "provider".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding::deeplink::parser::parse_deeplink_url;

    fn req_for(app: &str) -> DeepLinkImportRequest {
        parse_deeplink_url(&format!(
            "aitoolbox://v1/import?resource=provider&app={app}&name=My%20Provider&category=custom&apiKey=sk-x&baseUrl=https%3A%2F%2Fapi.example.com&model=m1"
        ))
        .unwrap()
    }

    #[test]
    fn claude_env_shape() {
        let req = req_for("claude");
        let (settings, extra) = build_claude_settings(&req).unwrap();
        let v: Value = serde_json::from_str(&settings).unwrap();
        assert_eq!(v["env"]["ANTHROPIC_AUTH_TOKEN"], "sk-x");
        assert_eq!(v["env"]["ANTHROPIC_BASE_URL"], "https://api.example.com");
        assert_eq!(v["env"]["ANTHROPIC_MODEL"], "m1");
        assert_eq!(extra.as_deref(), Some("{}"));
    }

    #[test]
    fn claude_config_override() {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine as _;
        let cfg = URL_SAFE_NO_PAD.encode(r#"{"env":{"X":"1"}}"#);
        let req = parse_deeplink_url(&format!(
            "aitoolbox://v1/import?resource=provider&app=claude&name=T&category=custom&config={cfg}"
        ))
        .unwrap();
        let (settings, _) = build_claude_settings(&req).unwrap();
        let v: Value = serde_json::from_str(&settings).unwrap();
        assert_eq!(v["env"]["X"], "1");
        assert!(v["env"].get("ANTHROPIC_AUTH_TOKEN").is_none());
    }

    #[test]
    fn codex_toml_shape() {
        let req = req_for("codex");
        let settings = build_codex_settings(&req).unwrap();
        let v: Value = serde_json::from_str(&settings).unwrap();
        assert_eq!(v["auth"]["OPENAI_API_KEY"], "sk-x");
        let config = v["config"].as_str().unwrap();
        assert!(config.contains(r#"model_provider = "my-provider""#));
        assert!(config.contains("model = \"m1\""));
        // `my-provider` is a valid TOML bare key (dashes allowed), so the
        // table header is emitted without quotes.
        assert!(config.contains("[model_providers.my-provider]"));
        assert!(config.contains("base_url = \"https://api.example.com\""));
        assert!(config.contains("name = \"My Provider\""));
    }

    #[test]
    fn gemini_env_shape() {
        let req = req_for("gemini");
        let settings = build_gemini_settings(&req).unwrap();
        let v: Value = serde_json::from_str(&settings).unwrap();
        assert_eq!(v["env"]["GEMINI_API_KEY"], "sk-x");
        assert_eq!(
            v["env"]["GOOGLE_GEMINI_BASE_URL"],
            "https://api.example.com"
        );
        assert_eq!(v["env"]["GEMINI_MODEL"], "m1");
        assert!(v["config"].is_object());
    }

    #[test]
    fn slugify_handles_names() {
        assert_eq!(slugify("OpenRouter"), "openrouter");
        assert_eq!(slugify("My Cool API!"), "my-cool-api");
        assert_eq!(slugify("---"), "provider");
        assert_eq!(slugify("  spaces  "), "spaces");
    }

    #[test]
    fn codex_config_override_uses_verbatim_toml() {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine as _;
        let custom_toml = r#"model_provider = "custom"
model = "override"
[model_providers.custom]
name = "Custom"
base_url = "https://override.example.com"
"#;
        let cfg = URL_SAFE_NO_PAD.encode(custom_toml);
        let req = parse_deeplink_url(&format!(
            "aitoolbox://v1/import?resource=provider&app=codex&name=X&category=custom&config={cfg}"
        ))
        .unwrap();
        let settings = build_codex_settings(&req).unwrap();
        let v: Value = serde_json::from_str(&settings).unwrap();
        // When config is provided it is used verbatim as the TOML `config`
        // string; apiKey (none here) still drives auth but no slug-derived
        // provider block is synthesized.
        let config = v["config"].as_str().unwrap();
        assert!(config.contains("model_provider = \"custom\""));
        assert!(config.contains("model = \"override\""));
        assert!(
            !config.contains("[model_providers.x]"),
            "slug block must NOT be synthesized when config overrides"
        );
        assert!(v["auth"].is_object());
    }

    #[test]
    fn codex_without_model_omits_model_line() {
        // No `model` param: the TOML must keep model_providers but drop `model`.
        let req = parse_deeplink_url(
            "aitoolbox://v1/import?resource=provider&app=codex&name=NoModel&category=custom&baseUrl=https%3A%2F%2Fx.com",
        )
        .unwrap();
        let settings = build_codex_settings(&req).unwrap();
        let v: Value = serde_json::from_str(&settings).unwrap();
        let config = v["config"].as_str().unwrap();
        assert!(config.contains(r#"model_provider = "nomodel""#));
        assert!(!config.contains("model ="));
        assert!(config.contains("[model_providers.nomodel]"));
    }

    #[test]
    fn gemini_config_override_replaces_settings() {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine as _;
        let cfg = URL_SAFE_NO_PAD.encode(r#"{"env":{"GEMINI_API_KEY":"custom-key"}}"#);
        let req = parse_deeplink_url(&format!(
            "aitoolbox://v1/import?resource=provider&app=gemini&name=X&category=custom&config={cfg}"
        ))
        .unwrap();
        let settings = build_gemini_settings(&req).unwrap();
        let v: Value = serde_json::from_str(&settings).unwrap();
        assert_eq!(v["env"]["GEMINI_API_KEY"], "custom-key");
        // The override replaces the whole settings_config; builder adds nothing.
        assert!(v.get("config").is_none());
    }

    #[test]
    fn claude_settings_omit_unset_fields() {
        // Only name+category: no apiKey/baseUrl/model, so env must be empty object.
        let req = parse_deeplink_url(
            "aitoolbox://v1/import?resource=provider&app=claude&name=Empty&category=official",
        )
        .unwrap();
        let (settings, extra) = build_claude_settings(&req).unwrap();
        let v: Value = serde_json::from_str(&settings).unwrap();
        assert!(v["env"].as_object().unwrap().is_empty());
        assert_eq!(extra.as_deref(), Some("{}"));
    }
}
