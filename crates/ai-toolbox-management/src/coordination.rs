//! Read-only guards for copying or replacing trees containing owner artifacts.
//!
//! A scan rejects artifacts already observed. It is not a filesystem transaction
//! or a claim that an uncooperative remote writer cannot race a later deletion.
use std::path::Path;
use std::{fs, io};

use crate::transport::is_reserved_coordination_name;

const MAX_ENTRIES: usize = 100_000;
const MAX_DEPTH: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeGuardError {
    CoordinationArtifact,
    UnsafeRoot,
    Unreadable,
    ScanLimit,
}

impl std::fmt::Display for TreeGuardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::CoordinationArtifact => "OWNER_COORDINATION_ARTIFACT_PRESENT",
            Self::UnsafeRoot => "OWNER_REPLACEMENT_ROOT_UNSAFE",
            Self::Unreadable => "OWNER_REPLACEMENT_TREE_UNREADABLE",
            Self::ScanLimit => "OWNER_REPLACEMENT_SCAN_LIMIT",
        })
    }
}

impl std::error::Error for TreeGuardError {}

/// Archive/import paths are portable strings. Reject a reserved component on
/// either separator convention before extracting or overwriting any destination.
pub fn contains_coordination_component(path: &str) -> bool {
    path.split(['/', '\\'])
        .any(|part| is_reserved_coordination_name(std::ffi::OsStr::new(part)))
}

/// Never removes or repairs anything. Missing roots are allowed; existing roots
/// must be real directories. Descendant symlinks are not followed. Callers must
/// retain their admission/lease and repeat this check before destructive replace;
/// this alone is not proof against external TOCTOU or remote active owners.
pub fn ensure_replaceable_tree(root: &Path) -> Result<(), TreeGuardError> {
    if root.file_name().is_some_and(is_reserved_coordination_name) {
        return Err(TreeGuardError::CoordinationArtifact);
    }
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(TreeGuardError::Unreadable),
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(TreeGuardError::UnsafeRoot);
    }
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut entries = 0usize;
    while let Some((directory, depth)) = pending.pop() {
        if depth > MAX_DEPTH {
            return Err(TreeGuardError::ScanLimit);
        }
        for entry in fs::read_dir(&directory).map_err(|_| TreeGuardError::Unreadable)? {
            let entry = entry.map_err(|_| TreeGuardError::Unreadable)?;
            entries += 1;
            if entries > MAX_ENTRIES {
                return Err(TreeGuardError::ScanLimit);
            }
            if is_reserved_coordination_name(&entry.file_name()) {
                return Err(TreeGuardError::CoordinationArtifact);
            }
            let kind = entry.file_type().map_err(|_| TreeGuardError::Unreadable)?;
            if kind.is_dir() {
                pending.push((entry.path(), depth + 1));
            }
        }
    }
    Ok(())
}

/// Exact shell globs for source pruning. Keep the lengths tied to the same
/// fixed-width endpoint format as transport::is_reserved_coordination_name.
pub fn reserved_coordination_shell_globs() -> [String; 3] {
    let hex = "[0123456789abcdef]".repeat(16);
    [
        crate::owner::PROFILE_LEASE_FILE.to_string(),
        crate::owner::RESOURCE_LEASE_FILE.to_string(),
        format!("{}{hex}-{hex}{}", crate::transport::ENDPOINT_PREFIX, crate::transport::ENDPOINT_SUFFIX),
    ]
}

/// Quote a path as data while retaining only the documented leading home alias.
/// Never interpolate arbitrary path bytes as shell source.
pub fn quote_posix_sync_path(path: &str) -> String {
    let quote = |value: &str| format!("'{}'", value.replace('\'', "'\\''"));
    for prefix in ["~/", "$HOME/"] {
        if let Some(relative) = path.strip_prefix(prefix) {
            return format!("\"$HOME\"/{}", quote(relative));
        }
    }
    if path == "~" || path == "$HOME" {
        "\"$HOME\"".to_string()
    } else {
        quote(path)
    }
}

/// Shared POSIX preflight used by WSL and SSH. It has no filesystem writes,
/// does not follow descendant symlinks, and propagates find/read errors. Run it
/// before staging and again directly before rm; neither check is a remote lease
/// and the gap between the check and deletion remains a TOCTOU boundary.
pub fn posix_coordination_guard() -> String {
    let [profile, runtime, socket] = reserved_coordination_shell_globs();
    format!(r#"
ait_reserved_name() {{
    case "$1" in {profile}|{runtime}|{socket}) return 0;; *) return 1;; esac
}}
ait_guard_path() {{
    ait_remaining=$1
    while :; do
        ait_part=${{ait_remaining%%/*}}
        if ait_reserved_name "$ait_part"; then
            echo OWNER_COORDINATION_PATH_RESERVED >&2; return 1
        fi
        case "$ait_remaining" in */*) ait_remaining=${{ait_remaining#*/}};; *) break;; esac
    done
}}
ait_guard_tree() {{
    ait_guard_path "$1" || return 1
    # Reject symlink roots, dangling ancestors and inaccessible parents instead
    # of treating a failed existence probe as an absent destination. Ordinary
    # ancestor aliases are not an owner proof; descendant links are not followed.
    if [ -L "$1" ]; then echo OWNER_REPLACEMENT_ROOT_UNSAFE >&2; return 1; fi
    ait_parent=$1
    while :; do
        if [ -L "$ait_parent" ] && [ ! -e "$ait_parent" ]; then
            echo OWNER_REPLACEMENT_ROOT_UNSAFE >&2; return 1
        fi
        if [ -e "$ait_parent" ]; then
            if [ "$ait_parent" != "$1" ] && [ ! -d "$ait_parent" ]; then
                echo OWNER_REPLACEMENT_ROOT_UNSAFE >&2; return 1
            fi
            if [ -d "$ait_parent" ] && [ ! -x "$ait_parent" ]; then
                echo OWNER_REPLACEMENT_TREE_UNREADABLE >&2; return 1
            fi
        fi
        case "$ait_parent" in /|.|'') break;; esac
        ait_next=$(dirname -- "$ait_parent") || return 1
        [ "$ait_next" != "$ait_parent" ] || break
        ait_parent=$ait_next
    done
    if [ ! -e "$1" ]; then return 0; fi
    if [ ! -d "$1" ]; then echo OWNER_REPLACEMENT_ROOT_UNSAFE >&2; return 1; fi
    ait_found=$(find "$1" \( -name '{profile}' -o -name '{runtime}' -o -name '{socket}' \) -print -quit) || {{
        echo OWNER_REPLACEMENT_TREE_UNREADABLE >&2; return 1;
    }}
    if [ -n "$ait_found" ]; then echo OWNER_COORDINATION_ARTIFACT_PRESENT >&2; return 1; fi
}}
"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    mod shell_fixture {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/bounded_shell.rs"));
    }

    #[test]
    fn shell_globs_are_exact_and_paths_do_not_become_shell_source() {
        let patterns = reserved_coordination_shell_globs();
        assert_eq!(patterns[0], crate::owner::PROFILE_LEASE_FILE);
        assert_eq!(patterns[1], crate::owner::RESOURCE_LEASE_FILE);
        assert_eq!(patterns[2].matches("[0123456789abcdef]").count(), 32);
        assert!(!patterns[2].contains('*'));
        assert_eq!(quote_posix_sync_path("~/ordinary folder"), "\"$HOME\"/'ordinary folder'");
        assert_eq!(quote_posix_sync_path("/tmp/a'$(false)"), "'/tmp/a'\\''$(false)'");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn posix_guard_rejects_reserved_directories_dangling_links_and_sockets_without_mutation() {
        use std::os::unix::fs::MetadataExt;
        for kind in ["directory", "symlink", "socket"] {
            let fixture = tempfile::tempdir().unwrap();
            let target = fixture.path().join("target");
            fs::create_dir(&target).unwrap();
            let artifact = target.join(".ai-toolbox-owner.lock");
            let _socket = match kind {
                "directory" => { fs::create_dir(&artifact).unwrap(); None }
                "symlink" => { std::os::unix::fs::symlink("missing", &artifact).unwrap(); None }
                _ => Some(std::os::unix::net::UnixListener::bind(&artifact).unwrap()),
            };
            let inode = fs::symlink_metadata(&artifact).unwrap().ino();
            let marker = fixture.path().join("must-not-create");
            let command = format!("set -e; {}\nait_guard_tree {}; mkdir {}", posix_coordination_guard(), quote_posix_sync_path(target.to_str().unwrap()), quote_posix_sync_path(marker.to_str().unwrap()));
            let (_fixture, output) = shell_fixture::run("sh", &command, fixture);
            assert!(!output.status.success());
            assert!(!marker.exists());
            assert_eq!(fs::symlink_metadata(&artifact).unwrap().ino(), inode);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn posix_tree_guard_rejects_ordinary_file_root_without_mutation() {
        use std::os::unix::fs::MetadataExt;
        let fixture = tempfile::tempdir().unwrap();
        let target = fixture.path().join("ordinary-file");
        fs::write(&target, b"unchanged ordinary file").unwrap();
        let inode = fs::metadata(&target).unwrap().ino();
        let marker = fixture.path().join("must-not-create");
        let command = format!("set -e; {}\nait_guard_tree {}; mkdir {}", posix_coordination_guard(), quote_posix_sync_path(target.to_str().unwrap()), quote_posix_sync_path(marker.to_str().unwrap()));
        let (_fixture, output) = shell_fixture::run("sh", &command, fixture);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("OWNER_REPLACEMENT_ROOT_UNSAFE"));
        assert_eq!(fs::read(&target).unwrap(), b"unchanged ordinary file");
        assert_eq!(fs::metadata(&target).unwrap().ino(), inode);
        assert!(!marker.exists());
        assert_eq!(ensure_replaceable_tree(&target), Err(TreeGuardError::UnsafeRoot));
    }

    #[test]
    fn archive_components_reject_only_exact_reserved_shapes() {
        for path in ["codex/.ai-toolbox-runtime.lock", "profile\\.ai-toolbox-owner.lock",
            "x/.ai-toolbox-owner-0000000000000001-0000000000000002.sock/y"] {
            assert!(contains_coordination_component(path));
        }
        for path in [".config/config.toml", "AGENTS.md", "a/.ai-toolbox-owner-note.sock",
            ".ai-toolbox-runtime.lock.backup", ".ai-toolbox-owner-1-2.sock"] {
            assert!(!contains_coordination_component(path));
        }
    }

    #[test]
    fn existing_reserved_lock_is_rejected_without_mutation() {
        let directory = tempfile::tempdir().unwrap();
        let nested = directory.path().join("nested");
        fs::create_dir(&nested).unwrap();
        let lock = nested.join(".ai-toolbox-runtime.lock");
        fs::write(&lock, b"synthetic-coordination").unwrap();
        let before = fs::metadata(&lock).unwrap();
        assert_eq!(ensure_replaceable_tree(directory.path()), Err(TreeGuardError::CoordinationArtifact));
        assert_eq!(fs::read(&lock).unwrap(), b"synthetic-coordination");
        #[cfg(unix)] {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(before.ino(), fs::metadata(&lock).unwrap().ino());
        }
        assert!(directory.path().exists());
    }

    #[test]
    fn ordinary_tree_and_missing_target_are_read_only_controls() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("config.toml");
        fs::write(&file, b"synthetic = true\n").unwrap();
        assert_eq!(ensure_replaceable_tree(directory.path()), Ok(()));
        let missing = directory.path().join("missing");
        assert_eq!(ensure_replaceable_tree(&missing), Ok(()));
        assert!(!missing.exists());
        assert_eq!(fs::read(file).unwrap(), b"synthetic = true\n");
    }
}
