use ai_toolbox_lib::coding::skills::sync_engine::sync_dir_for_tool_with_overwrite;
use ai_toolbox_lib::coding::skills::types::SyncMode;

/// Tools whose skills dir cannot follow symlinks (cursor, antigravity_cli) must
/// always land a real copied directory, even when no explicit force_copy flag
/// is passed by the caller.
#[test]
fn forced_copy_tools_always_copy_without_explicit_force_copy_flag() {
    for tool_key in ["cursor", "antigravity_cli"] {
        let temp = tempfile::tempdir().expect("temp dir");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        std::fs::create_dir(&source).expect("create source");
        std::fs::write(source.join("SKILL.md"), "---\nname: test\n---\n").expect("write source");

        let outcome = sync_dir_for_tool_with_overwrite(tool_key, &source, &target, false, false)
            .unwrap_or_else(|err| panic!("{tool_key} sync should succeed: {err}"));

        assert!(
            matches!(outcome.mode_used, SyncMode::Copy),
            "{tool_key} must sync in copy mode, got {:?}",
            outcome.mode_used
        );
        // A real directory must exist, not a symlink/junction pointing at the
        // central repo (read_link fails for real dirs on all platforms).
        assert!(
            std::fs::read_link(&target).is_err(),
            "{tool_key} target must not be a link"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("SKILL.md")).expect("read copied skill"),
            "---\nname: test\n---\n"
        );
    }
}
