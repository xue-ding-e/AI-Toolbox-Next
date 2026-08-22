use std::collections::HashSet;

use ai_toolbox_lib::coding::skills::tool_adapters::{
    adapter_by_key, default_tool_adapters, runtime_adapter_by_key, CustomTool,
};
use ai_toolbox_lib::coding::tools::BUILTIN_TOOLS;

#[test]
fn default_tool_adapters_cover_all_builtin_skill_tools() {
    let actual_keys: HashSet<&'static str> = default_tool_adapters()
        .into_iter()
        .map(|adapter| adapter.key)
        .collect();
    let expected_keys: HashSet<&'static str> = BUILTIN_TOOLS
        .iter()
        .filter(|tool| tool.relative_skills_dir.is_some())
        .map(|tool| tool.key)
        .collect();

    assert_eq!(actual_keys, expected_keys);
}

#[test]
fn adapter_by_key_returns_qoder_variants() {
    let qoder = adapter_by_key("qoder").expect("qoder should be available in skills adapters");
    assert_eq!(qoder.display_name, "Qoder");
    assert_eq!(qoder.relative_skills_dir, "~/.qoder/skills");

    let qoder_work =
        adapter_by_key("qoder_work").expect("qoder_work should be available in skills adapters");
    assert_eq!(qoder_work.display_name, "QoderWork");
    assert_eq!(qoder_work.relative_skills_dir, "~/.qoderwork/skills");
}

#[test]
fn adapter_by_key_returns_trae_variants_and_openclaw_family() {
    let trae = adapter_by_key("trae").expect("trae should be available in skills adapters");
    assert_eq!(trae.display_name, "TRAE IDE");
    assert_eq!(trae.relative_skills_dir, "~/.trae/skills");

    let trae_cn =
        adapter_by_key("trae_cn").expect("trae_cn should be available in skills adapters");
    assert_eq!(trae_cn.display_name, "TRAE CN");
    assert_eq!(trae_cn.relative_skills_dir, "~/.trae-cn/skills");

    let qclaw = adapter_by_key("qclaw").expect("qclaw should be available in skills adapters");
    assert_eq!(qclaw.display_name, "QClaw");
    assert_eq!(qclaw.relative_skills_dir, "~/.qclaw/skills");

    let easyclaw =
        adapter_by_key("easyclaw").expect("easyclaw should be available in skills adapters");
    assert_eq!(easyclaw.display_name, "EasyClaw");
    assert_eq!(easyclaw.relative_skills_dir, "~/.easyclaw/skills");

    let autoclaw =
        adapter_by_key("autoclaw").expect("autoclaw should be available in skills adapters");
    assert_eq!(autoclaw.display_name, "AutoClaw");
    // AutoClaw's home dir is `.openclaw-autoclaw`, not `.autoclaw`.
    assert_eq!(autoclaw.relative_skills_dir, "~/.openclaw-autoclaw/skills");
}

#[test]
fn new_skill_tools_are_skills_only_without_mcp_config() {
    for key in ["trae", "trae_cn", "qclaw", "easyclaw", "autoclaw"] {
        let tool = ai_toolbox_lib::coding::tools::builtin_tool_by_key(key)
            .unwrap_or_else(|| panic!("{key} should exist in BUILTIN_TOOLS"));
        assert!(
            tool.relative_skills_dir.is_some(),
            "{key} must support skills"
        );
        assert!(
            tool.mcp_config_path.is_none(),
            "{key} is intentionally skills-only; add MCP config only after verifying the real path"
        );
    }
}

#[test]
fn runtime_adapter_by_key_prefers_builtin_tool_without_custom_entry() {
    let custom_tools: Vec<CustomTool> = Vec::new();
    let runtime_adapter = runtime_adapter_by_key("qoder_work", &custom_tools)
        .expect("qoder_work runtime adapter should resolve");

    assert_eq!(runtime_adapter.key, "qoder_work");
    assert_eq!(runtime_adapter.display_name, "QoderWork");
    assert!(!runtime_adapter.is_custom);
}
