#[path = "../../../src/coding/claude_code/plugin_metadata_sync.rs"]
mod plugin_metadata_sync_impl;

#[test]
fn rewrites_known_marketplace_install_location_to_target_root() {
    let source_root = r"C:\Users\Tester\.claude\plugins";
    let target_root = "/home/tester/.claude/plugins";
    let raw_content = r#"{
  "claude-plugins-official": {
    "source": {
      "source": "github",
      "repo": "anthropics/claude-plugins-official"
    },
    "installLocation": "C:\\Users\\Tester\\.claude\\plugins\\marketplaces\\claude-plugins-official"
  }
}"#;

    let rewritten = plugin_metadata_sync_impl::rewrite_claude_plugin_metadata_if_needed(
        "known_marketplaces.json",
        raw_content,
        source_root,
        target_root,
    )
    .expect("rewrite known marketplaces")
    .expect("expected rewritten content");

    assert!(rewritten.contains(
        r#""installLocation": "/home/tester/.claude/plugins/marketplaces/claude-plugins-official""#
    ));
}

#[test]
fn rewrites_installed_plugin_install_paths_to_target_root() {
    let source_root = r"C:\Users\Tester\.claude\plugins";
    let target_root = "/home/tester/.claude/plugins";
    let raw_content = r#"{
  "version": 2,
  "plugins": {
    "typescript-lsp@claude-plugins-official": [
      {
        "scope": "user",
        "installPath": "C:\\Users\\Tester\\.claude\\plugins\\cache\\claude-plugins-official\\typescript-lsp\\1.0.0",
        "version": "1.0.0"
      }
    ]
  }
}"#;

    let rewritten = plugin_metadata_sync_impl::rewrite_claude_plugin_metadata_if_needed(
        "installed_plugins.json",
        raw_content,
        source_root,
        target_root,
    )
    .expect("rewrite installed plugins")
    .expect("expected rewritten content");

    assert!(rewritten.contains(
        r#""installPath": "/home/tester/.claude/plugins/cache/claude-plugins-official/typescript-lsp/1.0.0""#
    ));
}

#[test]
fn leaves_non_plugin_paths_unchanged() {
    let source_root = r"C:\Users\Tester\.claude\plugins";
    let target_root = "/home/tester/.claude/plugins";
    let raw_content = r#"{
  "claude-plugins-official": {
    "installLocation": "D:\\Elsewhere\\claude-plugins-official"
  }
}"#;

    let rewritten = plugin_metadata_sync_impl::rewrite_claude_plugin_metadata_if_needed(
        "known_marketplaces.json",
        raw_content,
        source_root,
        target_root,
    )
    .expect("rewrite known marketplaces");

    assert!(rewritten.is_none());
}
