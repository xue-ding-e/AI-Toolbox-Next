//! Shared ownership and replacement rules for WSL/SSH skill targets.

#[derive(Clone, Copy)]
pub enum RemoteSkillTargetAction {
    Link,
    Copy,
    Remove,
}

impl RemoteSkillTargetAction {
    pub fn sync_for_tool(tool_key: &str) -> Self {
        if crate::coding::tools::builtin_tool_forces_skill_copy(tool_key) {
            Self::Copy
        } else {
            Self::Link
        }
    }
}

const MANAGE_TARGET_SCRIPT: &str = r#"
set -eu
skill_source=$1
skill_target=$2
central_root=$3
action=$4
marker=.ai-toolbox-skill-source

# These paths are generated from runtime roots and one skill name. Reject
# ambiguous traversal before any filesystem mutation.
case "/$skill_source/$skill_target/" in */../*|*/./*) echo 'Skill paths contain dot segments' >&2; exit 1;; esac
resolve_parent() {
    ancestor=$1
    suffix=
    while [ ! -d "$ancestor" ]; do
        suffix="/$(basename -- "$ancestor")$suffix"
        next_ancestor=$(dirname -- "$ancestor")
        [ "$next_ancestor" != "$ancestor" ] || return 1
        ancestor=$next_ancestor
    done
    resolved_ancestor=$(cd -- "$ancestor" && pwd -P) || return 1
    printf '%s%s\n' "$resolved_ancestor" "$suffix"
}

# Resolve the parent, not a target symlink that we may need to replace.
target_parent=$(dirname -- "$skill_target")
if [ "$action" = remove ] && [ ! -d "$target_parent" ]; then
    printf 'unchanged\n'
    exit 0
fi
target_parent=$(resolve_parent "$target_parent")
skill_target="$target_parent/$(basename -- "$skill_target")"
central_root=$(cd -- "$central_root" && pwd -P)
skill_source="$central_root/$(basename -- "$skill_source")"
case "$skill_target/" in "$central_root/"*) echo 'Skill target overlaps central repository' >&2; exit 1;; esac
case "$central_root/" in "$skill_target/"*) echo 'Central repository overlaps skill target' >&2; exit 1;; esac

kind=missing
if [ -L "$skill_target" ]; then
    link_source=$(readlink -- "$skill_target")
    case "$link_source" in "$central_root"/*|"$1") kind=link;; *) kind=foreign;; esac
    case "/$link_source/" in */../*|*/./*) kind=foreign;; esac
elif [ -e "$skill_target" ]; then
    kind=foreign
    if [ -d "$skill_target" ] && [ ! -L "$skill_target/$marker" ] && [ -f "$skill_target/$marker" ]; then
        if [ "$(cat -- "$skill_target/$marker")" = "$skill_source" ]; then kind=copy; fi
    fi
fi
if [ "$kind" = foreign ]; then printf 'foreign\n'; exit 0; fi

if [ "$action" = remove ]; then
    if [ "$kind" = missing ]; then printf 'unchanged\n'; exit 0; fi
    rm -rf -- "$skill_target"
    printf 'updated\n'
    exit 0
fi

# Finish preparing the new target before moving the previous one aside.
test -d "$skill_source"
if [ "$action" = link ] && [ "$kind" = link ] && [ "$link_source" = "$skill_source" ]; then
    printf 'unchanged\n'; exit 0
fi
if [ "$action" = copy ] && [ "$kind" = copy ] && [ -f "$skill_source/.synced_hash" ] &&
    cmp -s -- "$skill_source/.synced_hash" "$skill_target/.synced_hash"; then
    printf 'unchanged\n'; exit 0
fi
mkdir -p -- "$target_parent"
staging=$(mktemp -d "$target_parent/.ai-toolbox-skill.XXXXXX")
trap 'rm -rf -- "$staging"' EXIT
if [ "$action" = copy ]; then
    mkdir -- "$staging/next"
    cp -R -P -- "$skill_source"/. "$staging/next"/
    rm -f -- "$staging/next/$marker"
    printf '%s\n' "$skill_source" > "$staging/next/$marker"
else
    ln -s -- "$skill_source" "$staging/next"
fi
if [ "$kind" != missing ]; then mv -- "$skill_target" "$staging/previous"; fi
if ! mv -- "$staging/next" "$skill_target"; then
    if [ "$kind" != missing ] && ! mv -- "$staging/previous" "$skill_target"; then
        trap - EXIT
        echo "Previous skill target preserved at $staging/previous" >&2
    fi
    exit 1
fi
printf 'updated\n'
"#;

fn quote_shell(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn remote_path_argument(path: &str) -> String {
    match path.strip_prefix("~/") {
        Some(relative) => format!("\"$HOME\"/{}", quote_shell(relative)),
        None => quote_shell(path),
    }
}

pub fn build_remote_skill_target_command(
    source: &str,
    target: &str,
    central_root: &str,
    action: RemoteSkillTargetAction,
) -> String {
    let action = match action {
        RemoteSkillTargetAction::Link => "link",
        RemoteSkillTargetAction::Copy => "copy",
        RemoteSkillTargetAction::Remove => "remove",
    };
    format!(
        "sh -c {} ai-toolbox-skills {} {} {} {}",
        quote_shell(MANAGE_TARGET_SCRIPT),
        remote_path_argument(source),
        remote_path_argument(target),
        remote_path_argument(central_root),
        action,
    )
}

/// False means a user-owned target was preserved, not a successful sync.
pub fn parse_remote_skill_target_result(output: &str) -> Result<bool, String> {
    match output.trim() {
        "updated" | "unchanged" => Ok(true),
        "foreign" => Ok(false),
        output => Err(format!("Unexpected skill target result: {output}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_path_arguments_only_expand_the_home_prefix() {
        assert_eq!(
            remote_path_argument("~/skills/a'b $name"),
            "\"$HOME\"/'skills/a'\\''b $name'"
        );
        assert_eq!(
            remote_path_argument("/tmp/$(touch unwanted)"),
            "'/tmp/$(touch unwanted)'"
        );
        assert_eq!(parse_remote_skill_target_result("foreign\n"), Ok(false));
        assert!(parse_remote_skill_target_result("").is_err());
    }

    #[cfg(unix)]
    fn run_target(
        source: &std::path::Path,
        target: &std::path::Path,
        central: &std::path::Path,
        action: RemoteSkillTargetAction,
    ) -> Result<bool, String> {
        let command = build_remote_skill_target_command(
            source.to_str().unwrap(),
            target.to_str().unwrap(),
            central.to_str().unwrap(),
            action,
        );
        let output = std::process::Command::new("sh")
            .args(["-c", &command])
            .output()
            .unwrap();
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        parse_remote_skill_target_result(&String::from_utf8_lossy(&output.stdout))
    }

    #[cfg(unix)]
    #[test]
    fn remote_copy_updates_and_removes_only_owned_targets() {
        let directory = tempfile::tempdir().unwrap();
        let central = directory.path().join("central");
        let source = central.join("test ' $skill");
        let target = directory.path().join("tool/test ' $skill");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("SKILL.md"), "first").unwrap();
        std::fs::write(source.join("removed.txt"), "old").unwrap();
        assert_eq!(
            run_target(&source, &target, &central, RemoteSkillTargetAction::Copy),
            Ok(true)
        );
        assert!(std::fs::read_link(&target).is_err());
        std::fs::write(source.join("SKILL.md"), "second").unwrap();
        std::fs::remove_file(source.join("removed.txt")).unwrap();
        assert_eq!(
            run_target(&source, &target, &central, RemoteSkillTargetAction::Copy),
            Ok(true)
        );
        assert_eq!(
            std::fs::read_to_string(target.join("SKILL.md")).unwrap(),
            "second"
        );
        assert!(!target.join("removed.txt").exists());
        std::fs::remove_dir_all(&source).unwrap();
        assert!(run_target(&source, &target, &central, RemoteSkillTargetAction::Copy).is_err());
        assert_eq!(
            std::fs::read_to_string(target.join("SKILL.md")).unwrap(),
            "second"
        );
        assert_eq!(
            run_target(&source, &target, &central, RemoteSkillTargetAction::Remove),
            Ok(true)
        );
        assert!(!target.exists());
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("SKILL.md"), "user").unwrap();
        assert_eq!(
            run_target(&source, &target, &central, RemoteSkillTargetAction::Remove),
            Ok(false)
        );
        assert_eq!(
            std::fs::read_to_string(target.join("SKILL.md")).unwrap(),
            "user"
        );
    }

    #[cfg(unix)]
    #[test]
    fn remote_copy_migrates_managed_links_and_protects_foreign_links_and_overlap() {
        let directory = tempfile::tempdir().unwrap();
        let central = directory.path().join("central");
        let source = central.join("skill");
        let target = directory.path().join("tool/skill");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("SKILL.md"), "content").unwrap();
        assert_eq!(
            run_target(&source, &target, &central, RemoteSkillTargetAction::Link),
            Ok(true)
        );
        assert_eq!(
            run_target(&source, &target, &central, RemoteSkillTargetAction::Copy),
            Ok(true)
        );
        assert!(std::fs::read_link(&target).is_err());
        assert_eq!(
            run_target(&source, &target, &central, RemoteSkillTargetAction::Remove),
            Ok(true)
        );
        std::os::unix::fs::symlink(directory.path(), &target).unwrap();
        for action in [
            RemoteSkillTargetAction::Copy,
            RemoteSkillTargetAction::Remove,
        ] {
            assert_eq!(run_target(&source, &target, &central, action), Ok(false));
        }
        assert!(run_target(&source, &source, &central, RemoteSkillTargetAction::Copy).is_err());
        let nested = source.join("missing/child/skill");
        assert!(run_target(&source, &nested, &central, RemoteSkillTargetAction::Copy).is_err());
        assert!(!source.join("missing").exists());
        let alias = directory.path().join("central-alias");
        std::os::unix::fs::symlink(&central, &alias).unwrap();
        assert!(run_target(
            &source,
            &alias.join("skill"),
            &central,
            RemoteSkillTargetAction::Copy
        )
        .is_err());
        assert!(source.join("SKILL.md").exists());
    }
}
