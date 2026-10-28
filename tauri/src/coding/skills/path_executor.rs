use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::sync_engine::{
    ensure_source_dir, ensure_source_target_not_overlapping, sync_dir_for_tool_with_overwrite,
    validate_sync_target_preflight,
};
use super::types::{SyncMode, SyncOutcome};
use crate::coding::runtime_location;
use crate::coding::wsl;
use crate::management_admission::{
    admit_non_codex_targets, reject_managed_operation, CrossTargetAdmission,
};

fn parse_wsl_target_path(target: &Path) -> Option<runtime_location::WslLocationInfo> {
    target
        .to_str()
        .and_then(runtime_location::parse_wsl_unc_path)
}

pub fn sync_skill_to_target(
    tool_key: &str,
    source: &Path,
    target: &Path,
    overwrite: bool,
    force_copy: bool,
) -> Result<SyncOutcome> {
    // Keep the capability in the actual synchronous worker, not only its
    // potentially cancelled async caller. Custom/copy keys can alias Codex.
    let admission = admit_non_codex_targets(Some(tool_key), &[target])
        .map_err(anyhow::Error::msg)?;
    // The first managed slice does not certify link creation/replacement.
    // Commands preflight the whole CRUD; retain the same fail-closed rule at
    // the final writer for direct callers, before source/target preparation.
    let _mode_admission = if !force_copy
        && !crate::coding::tools::builtin_tool_forces_skill_copy(tool_key)
    {
        reject_managed_operation().map_err(anyhow::Error::msg)?
    } else {
        None
    };
    ensure_source_dir(source)?;

    if let Some(wsl_target) = parse_wsl_target_path(target) {
        let source_path = source.to_string_lossy().to_string();
        let unc_target_path =
            runtime_location::build_windows_unc_path(&wsl_target.distro, &wsl_target.linux_path);

        if !overwrite && wsl::wsl_path_exists(&wsl_target.distro, &wsl_target.linux_path) {
            anyhow::bail!("target already exists: {:?}", target);
        }

        if overwrite {
            admission.ensure_current().map_err(anyhow::Error::msg)?;
            wsl::remove_wsl_path(&wsl_target.distro, &wsl_target.linux_path)
                .map_err(anyhow::Error::msg)
                .with_context(|| format!("remove existing WSL target {:?}", target))?;
        }

        admission.ensure_current().map_err(anyhow::Error::msg)?;
        wsl::sync_directory(&source_path, &wsl_target.linux_path, &wsl_target.distro)
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("sync directory {:?} -> {:?}", source, target))?;

        return Ok(SyncOutcome {
            mode_used: SyncMode::Copy,
            target_path: unc_target_path,
            replaced: overwrite,
        });
    }

    admission.ensure_current().map_err(anyhow::Error::msg)?;
    sync_dir_for_tool_with_overwrite(tool_key, source, target, overwrite, force_copy)
}

pub fn remove_skill_target(target_path: &str) -> Result<()> {
    let admission = admit_non_codex_targets(None, &[Path::new(target_path)])
        .map_err(anyhow::Error::msg)?;
    remove_skill_target_admitted(target_path, &admission)
}

fn remove_skill_target_admitted(target_path: &str, admission: &CrossTargetAdmission) -> Result<()> {
    admission.ensure_current().map_err(anyhow::Error::msg)?;
    if let Some(wsl_target) = runtime_location::parse_wsl_unc_path(target_path) {
        return wsl::remove_wsl_path(&wsl_target.distro, &wsl_target.linux_path)
            .map_err(anyhow::Error::msg);
    }

    super::sync_engine::remove_path(target_path).map_err(anyhow::Error::msg)
}

pub fn remove_skill_target_checked(source: &Path, target_path: &str) -> Result<()> {
    let admission = admit_non_codex_targets(None, &[Path::new(target_path)])
        .map_err(anyhow::Error::msg)?;
    if runtime_location::parse_wsl_unc_path(target_path).is_none() {
        let target = PathBuf::from(target_path);
        if target == source || target.starts_with(source) || source.starts_with(&target) {
            anyhow::bail!(
                "source and target paths overlap: source={:?}, target={:?}",
                source,
                target
            );
        }
        if !is_direct_link_target(&target) {
            if let Err(err) = ensure_source_target_not_overlapping(source, &target) {
                if source.exists() {
                    return Err(err);
                }
            }
        }
    }

    remove_skill_target_admitted(target_path, &admission)
}

fn is_direct_link_target(target: &Path) -> bool {
    if std::fs::symlink_metadata(target)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
    {
        return true;
    }

    #[cfg(windows)]
    {
        junction::exists(target).unwrap_or(false)
    }

    #[cfg(not(windows))]
    {
        false
    }
}

pub fn sync_copy_target_path(source: &Path, target_path: &str) -> Result<SyncOutcome> {
    let target = PathBuf::from(target_path);
    sync_skill_to_target("copy", source, &target, true, true)
}

pub fn validate_skill_sync_target(source: &Path, target: &Path, force_copy: bool) -> Result<()> {
    // Inventory calls this before updating desired state. Reject unsupported
    // targets here as well as rechecking in the eventual filesystem worker.
    let _admission = admit_non_codex_targets(None, &[target]).map_err(anyhow::Error::msg)?;
    let _mode_admission = if !force_copy {
        reject_managed_operation().map_err(anyhow::Error::msg)?
    } else {
        None
    };
    if parse_wsl_target_path(target).is_some() {
        ensure_source_dir(source)?;
        return Ok(());
    }

    validate_sync_target_preflight(source, target, force_copy)
}

pub fn target_path_changed(previous_target_path: &str, next_target: &Path) -> bool {
    let next_target_path = next_target.to_string_lossy();
    previous_target_path.trim().to_ascii_lowercase() != next_target_path.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    fn managed_skill_fixture() -> (
        tempfile::TempDir,
        std::sync::Arc<crate::management_admission::GuiProfileSession>,
    ) {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::Builder::new()
            .prefix("ai-toolbox-managed-skills-")
            .tempdir_in(std::env::temp_dir())
            .unwrap();
        for child in ["profile", "codex", "source", "claude"] {
            let path = directory.path().join(child);
            std::fs::create_dir(&path).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        std::fs::write(directory.path().join("source/SKILL.md"), "source sentinel\n").unwrap();
        let owner = ai_toolbox_management::owner::ProfileLeaseStartup::acquire(
            directory.path().join("profile"),
        ).unwrap().into_managed().unwrap();
        let runtime = ai_toolbox_management::owner::RuntimeResourceIdentity::existing(
            directory.path().join("codex"),
        ).unwrap();
        let session = crate::management_admission::GuiProfileSession::managed_with_runtime(
            owner, runtime,
        ).unwrap();
        (directory, session)
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn managed_skill_targets_reject_codex_and_aliases_before_replace_or_mkdir() {
        let (directory, session) = managed_skill_fixture();
        let _scope = crate::management_admission::test_session(&session);
        let source = directory.path().join("source");
        let target = directory.path().join("codex/skills/fixture");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("SKILL.md"), "target sentinel\n").unwrap();
        let alias = directory.path().join("custom-alias");
        std::os::unix::fs::symlink(directory.path().join("codex"), &alias).unwrap();
        for path in [target.clone(), alias.join("skills/fixture"), alias.join("new/fixture")] {
            assert!(sync_skill_to_target("custom_alias", &source, &path, true, true).is_err());
            assert!(sync_copy_target_path(&source, path.to_str().unwrap()).is_err());
            assert!(remove_skill_target(path.to_str().unwrap()).is_err());
            assert!(remove_skill_target_checked(&source, path.to_str().unwrap()).is_err());
            assert!(validate_skill_sync_target(&source, &path, true).is_err());
        }
        let otherwise_disjoint = directory.path().join("claude/new/fixture");
        assert!(sync_skill_to_target("codex", &source, &otherwise_disjoint, true, true).is_err());
        assert!(!otherwise_disjoint.parent().unwrap().exists());
        assert!(!directory.path().join("codex/new").exists());
        assert_eq!(std::fs::read_to_string(target.join("SKILL.md")).unwrap(), "target sentinel\n");
        assert_eq!(std::fs::read_to_string(source.join("SKILL.md")).unwrap(), "source sentinel\n");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn managed_skill_target_link_cannot_hide_its_protected_parent() {
        let (directory, session) = managed_skill_fixture();
        let _scope = crate::management_admission::test_session(&session);
        let source = directory.path().join("source");
        let target = directory.path().join("codex/fixture");
        std::os::unix::fs::symlink(&source, &target).unwrap();
        assert!(remove_skill_target_checked(&source, target.to_str().unwrap()).is_err());
        assert!(sync_skill_to_target("copy", &source, &target, true, true).is_err());
        assert!(std::fs::symlink_metadata(&target).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(source.join("SKILL.md")).unwrap(), "source sentinel\n");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn managed_skill_targets_keep_disjoint_claude_copy_and_cleanup_available() {
        let (directory, session) = managed_skill_fixture();
        let _scope = crate::management_admission::test_session(&session);
        let source = directory.path().join("source");
        let target = directory.path().join("claude/skills/fixture");
        validate_skill_sync_target(&source, &target, true).unwrap();
        assert!(sync_skill_to_target("claude_code", &source, &target, true, false).is_err());
        assert!(!target.parent().unwrap().exists());
        sync_skill_to_target("claude_code", &source, &target, true, true).unwrap();
        assert_eq!(std::fs::read_to_string(target.join("SKILL.md")).unwrap(), "source sentinel\n");
        remove_skill_target_checked(&source, target.to_str().unwrap()).unwrap();
        assert!(!target.exists());
        assert!(source.join("SKILL.md").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn expired_skill_session_does_not_fall_back_to_an_ordinary_write() {
        let (directory, session) = managed_skill_fixture();
        let _scope = crate::management_admission::test_session(&session);
        let source = directory.path().join("source");
        let target = directory.path().join("claude/new/fixture");
        drop(session);
        let error = sync_skill_to_target("claude_code", &source, &target, true, true)
            .unwrap_err();
        assert_eq!(error.to_string(), "The GUI profile owner is unavailable");
        assert!(!target.parent().unwrap().exists());
    }

    #[cfg(unix)]
    #[test]
    fn checked_remove_deletes_direct_symlink_without_touching_source() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source = temp.path().join("source");
        let target = temp.path().join("target");
        std::fs::create_dir(&source).expect("create source");
        std::fs::write(source.join("SKILL.md"), "---\nname: valid\n---\n")
            .expect("write source file");
        std::os::unix::fs::symlink(&source, &target).expect("create target symlink");

        remove_skill_target_checked(&source, &target.to_string_lossy())
            .expect("remove direct link");

        assert!(source.exists());
        assert!(source.join("SKILL.md").exists());
        assert!(std::fs::symlink_metadata(&target).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn checked_remove_rejects_parent_symlink_resolving_to_source() {
        let temp = tempfile::tempdir().expect("temp dir");
        let central = temp.path().join("central");
        let runtime_skills = temp.path().join("runtime-skills");
        let source = central.join("drools-rule-dev");
        let target = runtime_skills.join("drools-rule-dev");

        std::fs::create_dir_all(&source).expect("create source");
        std::fs::write(source.join("SKILL.md"), "---\nname: drools-rule-dev\n---\n")
            .expect("write source file");
        std::os::unix::fs::symlink(&central, &runtime_skills)
            .expect("link runtime skills to central repo");

        let result = remove_skill_target_checked(&source, &target.to_string_lossy());

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("same path"));
        assert!(source.exists());
        assert_eq!(
            std::fs::read_to_string(source.join("SKILL.md")).expect("source survives"),
            "---\nname: drools-rule-dev\n---\n"
        );
    }

    #[test]
    fn checked_remove_allows_cleanup_when_source_is_missing() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source = temp.path().join("missing-source");
        let target = temp.path().join("target-copy");
        std::fs::create_dir(&target).expect("create target");
        std::fs::write(target.join("SKILL.md"), "---\nname: stale\n---\n")
            .expect("write target file");

        remove_skill_target_checked(&source, &target.to_string_lossy())
            .expect("remove stale target without source");

        assert!(!target.exists());
    }
}
