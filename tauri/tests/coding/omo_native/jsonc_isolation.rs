//! 端到端隔离验证：向真实的 `~/.omo/omo.jsonc` 结构写入 `[native]` 块，
//! 断言 `[opencode]` 块、共享键与注释逐字未变。
//!
//! 这是本模块与 opencode tab 的 OMO 区块共存的前提：同一个文件、两个 harness 块。

use ai_toolbox_lib::coding::omo_jsonc_patch::{
    patch_top_level_block, remove_top_level_key, top_level_key_names,
};
use serde_json::json;

/// 取自真实 `~/.omo/omo.jsonc` 的结构：插件版写入的 `[opencode]` 块 + 共享键 + 注释。
const REAL_SHAPE: &str = r#"{
  "_migrations": ["2026-07-opencode-config-unification","2026-08-reasoning-unification"],
  "$schema": "https://raw.githubusercontent.com/code-yeongyu/oh-my-openagent/dev/assets/omo.schema.json",
  "codegraph": { "daemon": true }, // 共享键：插件版与 Native 都会读
  "[opencode]": {
    "disabled_agents": [],
    "claude_code": { "commands": true, "skills": true },
    "agents": {
      "sisyphus": { "models": ["axonhub-chat/deepseek-v4-flash-0731"] },
      "hephaestus": { "models": ["axonhub-chat/glm-5.2"] }
    },
    "categories": {
      "deep-low": { "reasoning": "max", "model": "axonhub-chat/glm-5.2" }
    }
  }
}"#;

#[test]
fn native_write_isolates_opencode_block_and_comments() {
    let block = json!({
        "agents": {
            "plan-reviewer": { "model": "axonhub-chat/glm-5.2", "reasoning": "xhigh" }
        },
        "categories": {
            "deep-low": { "model": "axonhub-chat/deepseek-v4-flash-0731" }
        },
        "model_profile": "daily-normal"
    });
    let block_json = serde_json::to_string_pretty(&block).unwrap();

    let patched = patch_top_level_block(REAL_SHAPE, "[native]", &block_json, &[]);

    let before: serde_json::Value = json5::from_str(REAL_SHAPE).unwrap();
    let after: serde_json::Value = json5::from_str(&patched).unwrap();

    // 1) 新块写入正确。
    assert_eq!(
        after["[native]"]["agents"]["plan-reviewer"]["reasoning"],
        json!("xhigh")
    );
    assert_eq!(after["[native]"]["model_profile"], json!("daily-normal"));

    // 2) [opencode] 块逐字未变（结构相等）。
    assert_eq!(after["[opencode]"], before["[opencode]"]);

    // 3) 共享键与控制键未变。
    assert_eq!(after["codegraph"], before["codegraph"]);
    assert_eq!(after["_migrations"], before["_migrations"]);
    assert_eq!(after["$schema"], before["$schema"]);

    // 4) 注释保留。
    assert!(patched.contains("// 共享键：插件版与 Native 都会读"));

    // 5) 幂等：再写一次结果不变。
    let patched_again = patch_top_level_block(&patched, "[native]", &block_json, &[]);
    assert_eq!(patched, patched_again);
}

#[test]
fn native_clear_keeps_opencode_block_and_shared_keys() {
    let with_native = patch_top_level_block(
        REAL_SHAPE,
        "[native]",
        r#"{ "agents": { "explore": {} } }"#,
        &[],
    );

    let (cleared, found) = remove_top_level_key(&with_native, "[native]");
    assert!(found);

    let after: serde_json::Value = json5::from_str(&cleared).unwrap();
    assert!(after.get("[native]").is_none());

    let before: serde_json::Value = json5::from_str(REAL_SHAPE).unwrap();
    assert_eq!(after["[opencode]"], before["[opencode]"]);
    assert_eq!(after["codegraph"], before["codegraph"]);

    // 清理后不应留下任何 `[native]` 痕迹。
    assert!(!top_level_key_names(&cleared)
        .iter()
        .any(|name| name == "[native]"));
}

#[test]
fn native_write_does_not_touch_a_shared_base_agents_key() {
    // 顶层共享 `agents` 与 `[native].agents` 是两个不同的层。
    // 写 `[native]` 不能动顶层共享键（那是插件版与 Native 的共同 base）。
    let raw = r#"{
  "agents": { "shared-agent": { "model": "a/b" } },
  "[opencode]": { "plugin": true }
}"#;
    let patched = patch_top_level_block(raw, "[native]", r#"{ "agents": {} }"#, &[]);
    let after: serde_json::Value = json5::from_str(&patched).unwrap();

    assert_eq!(after["agents"]["shared-agent"]["model"], json!("a/b"));
    assert_eq!(after["[opencode]"]["plugin"], json!(true));
    assert_eq!(after["[native]"]["agents"], json!({}));
}
