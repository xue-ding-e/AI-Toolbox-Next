use ai_toolbox_lib::coding::oh_my_opencode_slim::adapter::{
    fallback_config_to_value, from_db_value, global_config_from_db_value, merge_fallback_values,
    parse_fallback_config_value, resolve_slim_agents_from_config_value,
    strip_legacy_fallback_models_from_agents,
};
use ai_toolbox_lib::coding::oh_my_opencode_slim::types::OhMyOpenCodeSlimFallbackConfig;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn from_db_value_ignores_legacy_agent_fallback_models_and_keeps_supported_fallback_sources() {
    let config = from_db_value(json!({
        "id": "oh_my_opencode_slim_profile:test-profile",
        "name": "Test Profile",
        "agents": {
            "oracle": {
                "model": "gpt-5.4",
                "fallback_models": ["legacy-oracle"]
            },
            "orchestrator": {
                "fallback_models": "legacy-orchestrator"
            }
        },
        "fallback": {
            "enabled": true,
            "timeoutMs": 1200,
            "retryDelayMs": 80,
            "retry_on_empty": true,
            "chains": {
                "oracle": ["top-oracle"]
            },
            "strategy": "prefer-top-level"
        },
        "other_fields": {
            "fallback": {
                "enabled": false,
                "chains": {
                    "fixer": ["other-fixer"],
                    "oracle": ["other-oracle"]
                }
            },
            "theme": "compact"
        }
    }));

    let fallback = config.fallback.expect("fallback should be extracted");
    assert_eq!(config.id, "test-profile");
    assert_eq!(fallback.enabled, Some(true));
    assert_eq!(fallback.timeout_ms, Some(1200));
    assert_eq!(fallback.retry_delay_ms, Some(80));
    assert_eq!(fallback.retry_on_empty, Some(true));
    assert_eq!(
        fallback.chains,
        Some(json!({
            "oracle": ["top-oracle"],
            "fixer": ["other-fixer"]
        }))
    );
    assert_eq!(
        fallback.other_fields.get("strategy"),
        Some(&json!("prefer-top-level"))
    );
    assert_eq!(
        config.agents,
        Some(json!({
            "oracle": {
                "model": "gpt-5.4"
            },
            "orchestrator": {}
        }))
    );
    assert_eq!(config.other_fields, Some(json!({ "theme": "compact" })));
}

#[test]
fn fallback_config_roundtrips_string_and_array_chain_shapes() {
    let fallback_value = fallback_config_to_value(&OhMyOpenCodeSlimFallbackConfig {
        enabled: Some(false),
        timeout_ms: Some(1500),
        retry_delay_ms: Some(90),
        retry_on_empty: Some(true),
        chains: Some(json!({
            "oracle": "gpt-5.4-mini",
            "fixer": ["gpt-5.4", "gpt-5.4-mini"]
        })),
        other_fields: BTreeMap::from([("strategy".to_string(), json!("aggressive"))]),
    })
    .expect("fallback should serialize");

    assert_eq!(
        fallback_value,
        json!({
            "enabled": false,
            "timeoutMs": 1500,
            "retryDelayMs": 90,
            "retry_on_empty": true,
            "chains": {
                "oracle": "gpt-5.4-mini",
                "fixer": ["gpt-5.4", "gpt-5.4-mini"]
            },
            "strategy": "aggressive"
        })
    );

    let parsed = parse_fallback_config_value(&fallback_value).expect("fallback should parse");
    assert_eq!(parsed.enabled, Some(false));
    assert_eq!(parsed.timeout_ms, Some(1500));
    assert_eq!(parsed.retry_delay_ms, Some(90));
    assert_eq!(parsed.retry_on_empty, Some(true));
    assert_eq!(
        parsed.chains,
        Some(json!({
            "oracle": "gpt-5.4-mini",
            "fixer": ["gpt-5.4", "gpt-5.4-mini"]
        }))
    );
    assert_eq!(
        parsed.other_fields.get("strategy"),
        Some(&json!("aggressive"))
    );
}

#[test]
fn global_config_keeps_fallback_inside_other_fields() {
    let config = global_config_from_db_value(json!({
        "id": "oh_my_opencode_slim_global:global",
        "other_fields": {
            "fallback": {
                "enabled": true,
                "chains": {
                    "oracle": ["gpt-5.4"]
                }
            },
            "theme": "compact"
        }
    }));

    assert_eq!(config.id, "global");
    assert_eq!(
        config.other_fields,
        Some(json!({
            "fallback": {
                "enabled": true,
                "chains": {
                    "oracle": ["gpt-5.4"]
                }
            },
            "theme": "compact"
        }))
    );
}

#[test]
fn merge_fallback_values_keeps_global_settings_when_profile_only_overrides_chains() {
    let merged_fallback = merge_fallback_values(
        Some(json!({
            "chains": {
                "oracle": ["profile-oracle"]
            }
        })),
        Some(json!({
            "enabled": true,
            "timeout_ms": 1500,
            "strategy": "shared",
            "chains": {
                "fixer": ["global-fixer"]
            }
        })),
    )
    .expect("fallback should merge");

    assert_eq!(
        merged_fallback,
        json!({
            "enabled": true,
            "timeoutMs": 1500,
            "strategy": "shared",
            "chains": {
                "oracle": ["profile-oracle"],
                "fixer": ["global-fixer"]
            }
        })
    );
}

#[test]
fn strip_legacy_fallback_models_from_agents_removes_only_legacy_field() {
    let stripped = strip_legacy_fallback_models_from_agents(json!({
        "oracle": {
            "model": "openai/gpt-5.4",
            "variant": "high",
            "fallback_models": ["openai/gpt-5.4-mini"],
            "skills": ["plan"]
        },
        "reviewer": {
            "model": "openai/gpt-5.4-mini",
            "fallback_models": "openai/gpt-4.1-mini",
            "prompt": "custom"
        }
    }));

    assert_eq!(
        stripped,
        json!({
            "oracle": {
                "model": "openai/gpt-5.4",
                "variant": "high",
                "skills": ["plan"]
            },
            "reviewer": {
                "model": "openai/gpt-5.4-mini",
                "prompt": "custom"
            }
        })
    );
}

#[test]
fn resolve_slim_agents_from_config_value_reads_active_preset_and_keeps_advanced_fields() {
    let agents = resolve_slim_agents_from_config_value(&json!({
        "preset": "openai",
        "presets": {
            "openai": {
                "orchestrator": {
                    "model": "openai/GPT-5.5",
                    "prompt": "plan first",
                    "orchestratorPrompt": "coordinate specialists",
                    "displayName": "Lead",
                    "skills": ["planning"],
                    "mcps": ["github"],
                    "options": {
                        "temperature": 0.2
                    }
                },
                "oracle": {
                    "model": "openai/GPT-5.4"
                }
            },
            "anthropic": {
                "orchestrator": {
                    "model": "anthropic/claude"
                }
            }
        },
        "agents": {
            "orchestrator": {
                "variant": "high",
                "options": {
                    "top_p": 0.9
                }
            }
        }
    }))
    .expect("agents should resolve");

    assert_eq!(
        agents,
        json!({
            "orchestrator": {
                "model": "openai/GPT-5.5",
                "prompt": "plan first",
                "orchestratorPrompt": "coordinate specialists",
                "displayName": "Lead",
                "skills": ["planning"],
                "mcps": ["github"],
                "options": {
                    "temperature": 0.2,
                    "top_p": 0.9
                },
                "variant": "high"
            },
            "oracle": {
                "model": "openai/GPT-5.4"
            }
        })
    );
}

#[test]
fn resolve_slim_agents_from_config_value_falls_back_to_single_preset_entry() {
    let agents = resolve_slim_agents_from_config_value(&json!({
        "preset": "missing",
        "presets": {
            "best": {
                "orchestrator": {
                    "model": "openai/GPT-5.5",
                    "prompt": "single preset"
                }
            }
        }
    }))
    .expect("single preset agents should resolve");

    assert_eq!(
        agents,
        json!({
            "orchestrator": {
                "model": "openai/GPT-5.5",
                "prompt": "single preset"
            }
        })
    );
}
