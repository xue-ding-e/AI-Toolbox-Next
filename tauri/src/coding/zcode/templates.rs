//! Provider templates read from ZCode's installed built-in catalog.
//!
//! The catalog lives under a version-stamped directory
//! (`v2/runtime/provider/<platform>-<arch>/<appVersion>/endpoint-<hash>/`),
//! so it is discovered at call time. A bundled fallback keeps the picker
//! usable when ZCode is not installed on this machine.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::constants::ZCODE_PROVIDER_CONFIG_RELATIVE_PATH;
use super::types::ZcodeProviderTemplate;

/// Directory holding one catalog revision per installed ZCode version.
const CATALOG_RUNTIME_DIR: &str = "v2/runtime/provider";
const CATALOG_FILE_NAME: &str = "zcode-builtin.json";

/// Templates shipped with ZCode 3.14.x, used when the catalog cannot be read.
const FALLBACK_TEMPLATES: &[(&str, &str, &str, &str)] = &[
    ("zai-api", "Z.ai Coding Plan", "anthropic-messages", "https://api.z.ai/api/anthropic"),
    ("zai-standard-api", "Z.ai API", "openai-chat-completions", "https://api.z.ai/api/paas/v4"),
    ("bigmodel-api", "BigModel Coding Plan", "anthropic-messages", "https://open.bigmodel.cn/api/anthropic"),
    ("bigmodel-standard-api", "BigModel API", "openai-chat-completions", "https://open.bigmodel.cn/api/paas/v4"),
    ("deepseek", "DeepSeek", "anthropic-messages", "https://api.deepseek.com/anthropic"),
    ("moonshot-kimi", "Moonshot Kimi", "anthropic-messages", "https://api.moonshot.cn/anthropic"),
    ("minimax", "MiniMax", "anthropic-messages", "https://api.minimaxi.com/anthropic"),
    ("xiaomi-mimo", "Xiaomi MiMo", "openai-chat-completions", "https://api.xiaomimimo.com/v1"),
    ("openai", "OpenAI", "openai-responses", "https://api.openai.com"),
    ("anthropic", "Anthropic", "anthropic-messages", "https://api.anthropic.com"),
    ("xai", "xAI", "openai-responses", "https://api.x.ai"),
    ("openrouter", "OpenRouter", "openai-chat-completions", "https://openrouter.ai/api"),
    ("qwen-alibaba-model-studio-cn", "Alibaba Model Studio (CN)", "openai-chat-completions", "https://dashscope.aliyuncs.com/compatible-mode/v1"),
    ("qwen-alibaba-model-studio-intl", "Alibaba Model Studio (Intl)", "openai-chat-completions", "https://dashscope-intl.aliyuncs.com/compatible-mode/v1"),
];

fn fallback_templates() -> Vec<ZcodeProviderTemplate> {
    FALLBACK_TEMPLATES
        .iter()
        .map(|(id, name, api_type, base_url)| ZcodeProviderTemplate {
            template_id: (*id).to_string(),
            name: (*name).to_string(),
            api_type: Some((*api_type).to_string()),
            base_url: Some((*base_url).to_string()),
            logo_key: None,
        })
        .collect()
}

/// Finds the newest installed catalog file.
///
/// Version directory names sort lexicographically for the common `3.x.y`
/// shape; the `endpoint-*` segment is picked deterministically so the choice
/// is stable across runs.
fn find_catalog_file(root: &Path) -> Option<PathBuf> {
    let runtime_dir = root.join(CATALOG_RUNTIME_DIR);
    let platform_dirs = std::fs::read_dir(&runtime_dir).ok()?;
    let mut best: Option<(Vec<u32>, PathBuf)> = None;

    for platform_entry in platform_dirs.flatten() {
        let Ok(version_dirs) = std::fs::read_dir(platform_entry.path()) else {
            continue;
        };
        for version_entry in version_dirs.flatten() {
            let version = version_entry.file_name().to_string_lossy().to_string();
            let Ok(segments) = version
                .split('.')
                .map(str::parse::<u32>)
                .collect::<Result<Vec<_>, _>>()
            else {
                continue;
            };
            let Ok(endpoint_dirs) = std::fs::read_dir(version_entry.path()) else {
                continue;
            };
            let mut endpoints: Vec<PathBuf> = endpoint_dirs
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect();
            endpoints.sort();
            for endpoint in endpoints {
                let candidate = endpoint.join(CATALOG_FILE_NAME);
                if !candidate.is_file() {
                    continue;
                }
                if best.as_ref().is_none_or(|(current, _)| *current < segments) {
                    best = Some((segments.clone(), candidate));
                }
            }
        }
    }

    best.map(|(_, path)| path)
}

/// Parses `templateRules` into the flat template list the UI consumes.
fn parse_catalog(value: &Value) -> Vec<ZcodeProviderTemplate> {
    let rules = value
        .get("config")
        .and_then(|config| config.get("providerConfigRules"))
        .and_then(|rules| rules.get("templateRules"))
        .and_then(Value::as_array);
    let Some(rules) = rules else {
        return Vec::new();
    };

    rules
        .iter()
        .filter_map(|rule| {
            let template_id = rule.get("templateId").and_then(Value::as_str)?;
            let config = rule.get("config");
            let name = rule
                .get("templateNameMap")
                .and_then(|map| map.get("zh-CN").or_else(|| map.get("en-US")))
                .and_then(Value::as_str)
                .unwrap_or(template_id);
            Some(ZcodeProviderTemplate {
                template_id: template_id.to_string(),
                name: name.to_string(),
                api_type: config
                    .and_then(|config| config.get("api"))
                    .and_then(|api| api.get("type"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                base_url: config
                    .and_then(|config| config.get("api"))
                    .and_then(|api| api.get("baseUrl"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                logo_key: config
                    .and_then(|config| config.get("logo"))
                    .and_then(|logo| logo.get("key"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
        })
        .collect()
}

/// Reads templates from the installed catalog, falling back to the bundled
/// list when ZCode is absent or the catalog is unreadable.
pub fn read_provider_templates(root: &Path) -> Vec<ZcodeProviderTemplate> {
    let _ = ZCODE_PROVIDER_CONFIG_RELATIVE_PATH;
    let Some(catalog) = find_catalog_file(root) else {
        return fallback_templates();
    };
    let Ok(text) = std::fs::read_to_string(&catalog) else {
        return fallback_templates();
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return fallback_templates();
    };
    let parsed = parse_catalog(&value);
    if parsed.is_empty() {
        fallback_templates()
    } else {
        parsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_template_rules_into_flat_entries() {
        let catalog = json!({
            "config": {
                "providerConfigRules": {
                    "templateRules": [{
                        "templateId": "deepseek",
                        "templateNameMap": { "zh-CN": "深度求索", "en-US": "DeepSeek" },
                        "config": {
                            "access": { "type": "api-key" },
                            "api": { "type": "anthropic-messages", "baseUrl": "https://api.deepseek.com/anthropic" },
                            "logo": { "type": "builtin", "key": "deepseek" }
                        }
                    }]
                }
            }
        });
        let templates = parse_catalog(&catalog);
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].template_id, "deepseek");
        assert_eq!(templates[0].name, "深度求索");
        assert_eq!(templates[0].api_type.as_deref(), Some("anthropic-messages"));
        assert_eq!(templates[0].logo_key.as_deref(), Some("deepseek"));
    }

    #[test]
    fn missing_catalog_yields_fallback_list() {
        let templates = read_provider_templates(Path::new("Z:\\definitely-missing"));
        assert!(templates.iter().any(|item| item.template_id == "deepseek"));
        assert!(templates.iter().any(|item| item.template_id == "anthropic"));
    }
}
