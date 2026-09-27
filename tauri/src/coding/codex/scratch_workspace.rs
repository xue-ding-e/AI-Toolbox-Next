//! Codex project-less ("scratch") chats: the workspace directory Codex creates
//! for them, and the `[projects]` trust entry it writes into `config.toml`.
//!
//! Starting a Codex chat without a project makes the Codex app create a scratch
//! workspace next to the user's documents and mark it trusted:
//!
//! ```text
//! <Documents>/Codex/<YYYY-MM-DD>/<slug>
//! ```
//!
//! ```toml
//! [projects."/Users/me/Documents/Codex/2026-09-25/xi"]
//! trust_level = "trusted"
//! ```
//!
//! Deleting such a chat in Codex archives and removes the rollout but leaves both
//! of those behind, so they pile up. This module owns the two facts that make
//! them cleanable — and, more importantly, the guards that keep a *real* project
//! directory and its trust entry out of reach.
//!
//! Fact freshness: the directory layout above is observed behavior of the Codex
//! desktop app (the CLI does not create these directories; it only records trust
//! for the cwd it runs in, see `codex-rs/core/src/config/mod.rs`), so it is a
//! heuristic with a single home here. Every removal additionally requires a
//! structural match, containment below the anchor, no symlinked component, and
//! no `.git` anywhere between the anchor and the target.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::coding::mcp::yaml_sync::atomic_write_bytes;

/// The segment that names the scratch root under the user's documents directory.
const SCRATCH_PARENT_SEGMENT: &str = "documents";
const SCRATCH_ROOT_SEGMENT: &str = "codex";

/// Upper bound on the entries `inspect_workspace` walks, so a runaway directory
/// cannot stall a cleanup dialog. Hitting it marks the result as truncated,
/// which is also a refusal to remove: a tree too large to inspect cannot be
/// shown to be a scratch workspace.
const WORKSPACE_WALK_LIMIT: usize = 20_000;

/// What a scratch workspace looks like on disk.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScratchWorkspaceInfo {
    pub path: String,
    pub exists: bool,
    /// Exists and holds nothing that was produced here: no files anywhere below
    /// it, and no linked entry. Codex's own empty `work/` and `outputs/`
    /// directories do not count as content, which is what makes a fresh chat's
    /// workspace safe to preselect.
    pub is_empty: bool,
    pub file_count: u64,
    pub total_bytes: u64,
    /// A real project was created inside: removal is refused, and the cleanup UI
    /// must not offer one.
    pub has_git: bool,
    /// The walk stopped at [`WORKSPACE_WALK_LIMIT`]; counts are lower bounds.
    pub truncated: bool,
}

/// A `[projects]` trust entry whose key names a scratch workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScratchTrustEntry {
    /// The key exactly as the TOML table holds it, i.e. removal-ready.
    pub key: String,
}

/// Whether `path` matches `<...>/Documents/Codex/<YYYY-MM-DD>/<at least one>`.
///
/// Segment names are compared case-insensitively: Windows writes the trust key
/// lowercased and the rollout `cwd` in its original case.
pub fn is_codex_scratch_workspace(path: &str) -> bool {
    scratch_parts(path).is_some()
}

/// The `<...>/Documents/Codex` prefix of a scratch path, in the input's own
/// separator style.
pub fn codex_scratch_anchor(path: &str) -> Option<String> {
    let parts = scratch_parts(path)?;
    let separator = if path.contains('\\') { "\\" } else { "/" };
    let mut segments = parts.leading_segments.clone();
    segments.extend(
        parts.segments[parts.anchor_index..=parts.anchor_index + 1]
            .iter()
            .cloned(),
    );

    Some(join_segments(&parts.prefix, &segments, separator))
}

/// Whether two path spellings name the same location.
///
/// Separators are unified, Win32 verbatim/UNC prefixes are folded away, and a
/// `\\wsl.localhost\<distro>\...` spelling is reduced to the Linux path Codex
/// itself would have written. Case is folded only for Windows drive paths: a
/// WSL-runtime `config.toml` holds case-sensitive Linux keys.
pub fn same_path_for_compare(left: &str, right: &str) -> bool {
    path_compare_key(left) == path_compare_key(right)
}

/// The comparison key behind [`same_path_for_compare`], usable as a map key.
///
/// Case is folded only when *either* spelling is a Windows drive path, so two
/// Linux paths stay case-sensitive even though the host is Windows.
pub fn path_compare_key(value: &str) -> String {
    let normalized = normalize_for_compare(value);
    if is_windows_drive_path(&normalized) {
        normalized.to_ascii_lowercase()
    } else {
        normalized
    }
}

/// The comparison keys a stored path can be recognized by.
///
/// A directory a rollout recorded, the same directory as Codex's trust map names
/// it, and the same directory reached through a redirected Documents folder can
/// all be spelled differently — and a scan may list one spelling while a rollout
/// recorded another. Every reference check therefore has to try *all* spellings,
/// not only the one that happens to be compared, or a directory a live session
/// still uses is reported as leftovers.
pub fn path_compare_candidates(value: &str) -> Vec<String> {
    let mut candidates: Vec<String> = scratch_trust_key_candidates(value)
        .iter()
        .map(|candidate| path_compare_key(candidate))
        .collect();
    candidates.sort();
    candidates.dedup();
    candidates
}

/// The trust-key spellings a scratch workspace can be stored under.
///
/// Codex writes the *canonicalized* path (`project_trust_key`), so a redirected
/// or symlinked Documents directory produces a different key than the `cwd` a
/// rollout recorded. On Windows that key is also lowercased. Under WSL the key is
/// the Linux spelling, while a host-side session context only knows the UNC path.
pub fn scratch_trust_key_candidates(project_dir: &str) -> Vec<String> {
    let mut candidates: Vec<String> = vec![project_dir.to_string()];

    if let Some(canonical) = canonicalize_lenient(project_dir) {
        candidates.push(canonical);
    }

    if let Some(linux) = wsl_unc_to_linux(&project_dir.trim().replace('\\', "/")) {
        candidates.push(linux);
    }

    let mut seen = HashSet::new();
    candidates.retain(|candidate| seen.insert(normalize_for_compare(candidate)));
    candidates
}

/// Inspect one scratch workspace. Never fails: an unreadable directory reports
/// what could be seen.
pub fn inspect_workspace(path: &str) -> ScratchWorkspaceInfo {
    let target = Path::new(path);
    let metadata = std::fs::symlink_metadata(target).ok();
    let exists = metadata
        .as_ref()
        .is_some_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink());

    if !exists {
        return ScratchWorkspaceInfo {
            path: path.to_string(),
            exists: false,
            is_empty: false,
            file_count: 0,
            total_bytes: 0,
            has_git: false,
            truncated: false,
        };
    }

    let mut info = ScratchWorkspaceInfo {
        path: path.to_string(),
        exists: true,
        is_empty: true,
        file_count: 0,
        total_bytes: 0,
        has_git: has_git_entry(target),
        truncated: false,
    };

    let mut pending: Vec<PathBuf> = vec![target.to_path_buf()];
    let mut visited = 0usize;
    while let Some(directory) = pending.pop() {
        // A repository anywhere below the workspace is a real project too: the
        // user may have cloned or created one inside the scratch directory, and
        // "contains a git repository" is what the cleanup UI promises to refuse.
        if !info.has_git && has_git_entry(&directory) {
            info.has_git = true;
        }

        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            // Every entry counts towards the bound, directories included: a tree
            // of empty directories has nothing to lose but can be just as endless
            // as a tree of files.
            visited += 1;
            if visited > WORKSPACE_WALK_LIMIT {
                info.truncated = true;
                return info;
            }

            let Ok(file_type) = entry.file_type() else {
                // An entry that cannot be classified is content that cannot be
                // ruled out.
                info.is_empty = false;
                continue;
            };
            if file_type.is_dir() {
                // An empty directory holds nothing to lose. A *linked* one does:
                // the workspace is not followed into it, and removing the
                // workspace would still drop something the user put there.
                if file_type.is_symlink() {
                    info.is_empty = false;
                } else {
                    pending.push(entry.path());
                }
                continue;
            }

            // "Empty" means nothing was produced here — Codex's own empty
            // `work/` and `outputs/` directories do not make a workspace
            // something to be careful about.
            info.is_empty = false;
            info.file_count += 1;
            info.total_bytes += entry
                .metadata()
                .map(|metadata| metadata.len())
                .unwrap_or_default();
        }
    }

    info
}

/// Remove a scratch workspace after every guard passes, then prune the empty
/// dated directories it left behind.
///
/// `Ok(())` means the directory is gone (removing one that is already gone is
/// convergence, not an error). Every refusal returns the reason instead of
/// deleting anything.
pub fn remove_workspace(path: &str) -> Result<(), String> {
    let Some(anchor) = codex_scratch_anchor(path) else {
        return Err(format!(
            "Refusing to remove {path}: not a Codex scratch workspace"
        ));
    };

    let target = Path::new(path);
    let anchor_path = Path::new(&anchor);

    let chain = path_chain_below_anchor(target, anchor_path)
        .ok_or_else(|| format!("Refusing to remove {path}: it is not below {anchor}"))?;

    let metadata = match std::fs::symlink_metadata(target) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(format!(
                "Failed to inspect Codex scratch workspace {}: {error}",
                target.display()
            ))
        }
    };

    if let Some(metadata) = metadata {
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "Refusing to remove symlinked Codex scratch workspace {}",
                target.display()
            ));
        }
        if !metadata.is_dir() {
            return Err(format!(
                "Refusing to remove {}: it is not a directory",
                target.display()
            ));
        }

        // A symlinked component anywhere below the anchor could redirect the
        // removal outside the scratch root, so it is refused rather than
        // resolved.
        for component in &chain {
            let is_link = std::fs::symlink_metadata(component)
                .map(|metadata| metadata.file_type().is_symlink())
                .unwrap_or(false);
            if is_link {
                return Err(format!(
                    "Refusing to remove {}: {} is a symlink",
                    target.display(),
                    component.display()
                ));
            }
        }

        // A `.git` in the target or between the dated directory and the target
        // means a real repository lives here; that is a project, not a scratch
        // workspace.
        for component in chain.iter().skip(1) {
            if has_git_entry(component) {
                return Err(format!(
                    "Refusing to remove {}: it contains a git repository",
                    component.display()
                ));
            }
        }

        // The same question about everything *below* the target, where a user may
        // well have created a repository, and about the tree's size: a directory
        // too large to inspect cannot be shown to be a scratch workspace, so it is
        // refused rather than deleted blind.
        let info = inspect_workspace(path);
        if info.has_git {
            return Err(format!(
                "Refusing to remove {}: it contains a git repository",
                target.display()
            ));
        }
        if info.truncated {
            return Err(format!(
                "Refusing to remove {}: it holds more than {WORKSPACE_WALK_LIMIT} files, so it cannot be checked",
                target.display()
            ));
        }

        if !is_within_anchor_after_canonicalize(target, anchor_path) {
            return Err(format!(
                "Refusing to remove {}: its real path is outside {anchor}",
                target.display()
            ));
        }

        std::fs::remove_dir_all(target).map_err(|error| {
            format!(
                "Failed to remove Codex scratch workspace {}: {error}",
                target.display()
            )
        })?;
    }

    prune_empty_dated_ancestors(target, anchor_path);
    Ok(())
}

/// Every scratch workspace directly under a dated directory of `anchor`.
///
/// Only the immediate children are listed: a nested directory is part of the
/// workspace above it, and removing that one takes it along.
pub fn list_scratch_workspaces(anchor: &str) -> Vec<String> {
    let mut workspaces = Vec::new();

    for date_dir in list_dated_dirs(anchor) {
        let Ok(entries) = std::fs::read_dir(&date_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() || file_type.is_symlink() {
                continue;
            }
            workspaces.push(display_path(&path));
        }
    }

    workspaces.sort();
    workspaces
}

/// Dated directories under `anchor` that hold nothing at all.
pub fn list_empty_date_dirs(anchor: &str) -> Vec<String> {
    let mut empty = Vec::new();

    for date_dir in list_dated_dirs(anchor) {
        let is_empty = std::fs::read_dir(&date_dir)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if is_empty {
            empty.push(display_path(&date_dir));
        }
    }

    empty.sort();
    empty
}

/// Remove a directory that `list_empty_date_dirs` reported, re-checking that it
/// is still an empty dated directory directly under `anchor`.
pub fn remove_empty_date_dir(path: &str) -> Result<(), String> {
    let Some(anchor) = anchor_of_date_dir(path) else {
        return Err(format!(
            "Refusing to remove {path}: not a Codex scratch date directory"
        ));
    };

    let target = Path::new(path);
    let metadata = match std::fs::symlink_metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "Failed to inspect Codex date directory {}: {error}",
                target.display()
            ))
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "Refusing to remove {}: it is not a directory",
            target.display()
        ));
    }

    let is_empty = std::fs::read_dir(target)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false);
    if !is_empty {
        return Err(format!(
            "Refusing to remove {}: it is no longer empty",
            target.display()
        ));
    }

    if !is_within_anchor_after_canonicalize(target, Path::new(&anchor)) {
        return Err(format!(
            "Refusing to remove {}: its real path is outside {anchor}",
            target.display()
        ));
    }

    std::fs::remove_dir(target)
        .map_err(|error| format!("Failed to remove {}: {error}", target.display()))
}

/// Trust entries in `config.toml` whose key names a scratch workspace.
///
/// An unparsable `config.toml` is an error rather than an empty list: a caller
/// about to claim "nothing to clean" must be able to tell the difference.
pub fn list_scratch_trust_entries(config_toml: &str) -> Result<Vec<ScratchTrustEntry>, String> {
    Ok(project_table_keys(config_toml)?
        .into_iter()
        .filter(|key| is_codex_scratch_workspace(&key.replace('\\', "/")))
        .map(|key| ScratchTrustEntry { key })
        .collect())
}

/// The `[projects]` keys that name the given workspace.
///
/// Matching is by path, not by string: Codex stores its own canonical spelling
/// and lowercases it on Windows, while a rollout records the `cwd` as typed.
pub fn find_trust_keys(config_toml: &str, project_dir: &str) -> Result<Vec<String>, String> {
    let candidates = scratch_trust_key_candidates(project_dir);

    Ok(project_table_keys(config_toml)?
        .into_iter()
        .filter(|key| {
            candidates
                .iter()
                .any(|candidate| same_path_for_compare(candidate, key))
        })
        .collect())
}

/// Remove the given `[projects]` keys, dropping the table when it empties.
///
/// `keys` are matched verbatim, so callers pass keys obtained from
/// [`find_trust_keys`] or [`list_scratch_trust_entries`] for the same file. A
/// missing or unparsable `config.toml` is reported rather than silently treated
/// as "nothing to do": the caller's cleanup result must say why nothing changed.
///
/// The keys that were actually there are returned, so a caller reporting what it
/// cleaned up does not claim a key another writer had already removed.
pub fn remove_trust_keys(config_path: &Path, keys: &[String]) -> Result<Vec<String>, String> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }

    let raw = match std::fs::read_to_string(config_path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Failed to read {}: {error}", config_path.display())),
    };

    let mut document = raw
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| format!("Failed to parse {}: {error}", config_path.display()))?;

    let mut removed: Vec<String> = Vec::new();
    let mut projects_became_empty = false;

    if let Some(projects) = document
        .as_table_mut()
        .get_mut("projects")
        .and_then(|item| item.as_table_like_mut())
    {
        for key in keys {
            if projects.remove(key).is_some() {
                removed.push(key.clone());
            }
        }
        projects_became_empty = projects.iter().next().is_none();
    }

    if removed.is_empty() {
        return Ok(Vec::new());
    }

    if projects_became_empty {
        document.as_table_mut().remove("projects");
    }

    // Serialize the document as-is: the surrounding file is the user's, including
    // a possible `#:schema` line, and only the removed keys may change.
    atomic_write_bytes(config_path, document.to_string().as_bytes())?;
    Ok(removed)
}

// ---------------------------------------------------------------------------
// Path shape
// ---------------------------------------------------------------------------

/// The pieces of a scratch path: its leading prefix (`C:`, `//server/share`),
/// the segments above `Documents/Codex`, and the anchor position.
struct ScratchParts {
    prefix: String,
    leading_segments: Vec<String>,
    segments: Vec<String>,
    /// Index into `segments` of the `Documents` segment.
    anchor_index: usize,
}

fn scratch_parts(path: &str) -> Option<ScratchParts> {
    let (prefix, rest) = split_prefix(path);
    let segments: Vec<String> = rest
        .split(['/', '\\'])
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .map(str::to_string)
        .collect();

    // Reject `..` outright: a traversal is never a scratch workspace.
    if rest.split(['/', '\\']).any(|segment| segment == "..") {
        return None;
    }

    // The anchor is the *last* `Documents/Codex` pair that still leaves a dated
    // directory and at least one segment below it, so a scratch path nested
    // inside a scratch path anchors at its outermost match.
    let mut anchor_index = None;
    for index in (0..segments.len()).rev() {
        if !segments[index].eq_ignore_ascii_case(SCRATCH_PARENT_SEGMENT) {
            continue;
        }
        let Some(codex_segment) = segments.get(index + 1) else {
            continue;
        };
        if !codex_segment.eq_ignore_ascii_case(SCRATCH_ROOT_SEGMENT) {
            continue;
        }
        let Some(date_segment) = segments.get(index + 2) else {
            continue;
        };
        if !is_date_segment(date_segment) {
            continue;
        }
        if segments.len() < index + 4 {
            continue;
        }
        anchor_index = Some(index);
        break;
    }
    let anchor_index = anchor_index?;

    Some(ScratchParts {
        prefix,
        leading_segments: segments[..anchor_index].to_vec(),
        segments,
        anchor_index,
    })
}

/// Split a path's root (`C:` or `//server/share`) from the rest.
///
/// The prefix keeps the input's own separators, so a rebuilt path stays in the
/// spelling the caller handed us (a Windows UNC string must not come back with
/// forward slashes).
fn split_prefix(path: &str) -> (String, String) {
    let normalized = path.trim();
    let separator = if normalized.contains('\\') { "\\" } else { "/" };

    if normalized.starts_with("//") || normalized.starts_with("\\\\") {
        let rest = normalized.trim_start_matches(['/', '\\']);
        let mut parts = rest.split(['/', '\\']);
        let first = parts.next().unwrap_or_default();
        let second = parts.next().unwrap_or_default();

        // Verbatim prefixes carry the drive or share in the third segment:
        // `//?/C:/...` and `//?/UNC/server/share/...`.
        if first == "?" || first == "." {
            if second.eq_ignore_ascii_case("UNC") {
                let server = parts.next().unwrap_or_default();
                let share = parts.next().unwrap_or_default();
                let remainder = parts.collect::<Vec<_>>().join(separator);
                return (format!("//{server}{separator}{share}"), remainder);
            }
            let remainder = parts.collect::<Vec<_>>().join(separator);
            return (format!("//{second}"), remainder);
        }

        let remainder = parts.collect::<Vec<_>>().join(separator);
        return (format!("//{first}{separator}{second}"), remainder);
    }

    let bytes = normalized.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return (normalized[..2].to_string(), normalized[2..].to_string());
    }

    (String::new(), normalized.to_string())
}

fn join_segments(prefix: &str, segments: &[String], separator: &str) -> String {
    let body = segments.join(separator);
    if prefix.is_empty() {
        return format!("{separator}{body}");
    }
    if prefix.starts_with("//") {
        let trimmed = prefix.trim_start_matches(['/', '\\']);
        let lead = separator.repeat(2);
        return format!("{lead}{trimmed}{separator}{body}");
    }
    format!("{prefix}{separator}{body}")
}

fn is_date_segment(value: &str) -> bool {
    value.len() == 10
        && value.chars().enumerate().all(|(index, value)| match index {
            4 | 7 => value == '-',
            _ => value.is_ascii_digit(),
        })
}

/// The chain of directories from (and including) the anchor down to `path`.
fn path_chain_below_anchor(path: &Path, anchor: &Path) -> Option<Vec<PathBuf>> {
    let anchor_display = display_path(anchor);
    let anchor_display = anchor_display.trim_end_matches(['/', '\\']).to_string();

    let mut chain: Vec<PathBuf> = Vec::new();
    for ancestor in path.ancestors() {
        if ancestor.as_os_str().is_empty() {
            return None;
        }
        let display = display_path(ancestor);
        let display = display.trim_end_matches(['/', '\\']).to_string();
        if same_path_for_compare(&display, &anchor_display) {
            chain.push(ancestor.to_path_buf());
            chain.reverse();
            return Some(chain);
        }
        chain.push(ancestor.to_path_buf());
    }

    None
}

fn is_within_anchor_after_canonicalize(target: &Path, anchor: &Path) -> bool {
    let Some(target) = canonicalize_lenient_path(target) else {
        // The target is gone (or unreachable): nothing to remove, nothing to
        // escape.
        return true;
    };
    let Some(anchor) = canonicalize_lenient_path(anchor) else {
        return false;
    };

    let target = display_path(&target);
    let anchor = display_path(&anchor);
    let anchor = anchor.trim_end_matches(['/', '\\']);

    let target = normalize_for_compare(&target);
    let anchor = normalize_for_compare(anchor);
    let fold = is_windows_drive_path(&target) || is_windows_drive_path(&anchor);

    let matches = |prefix: &str, value: &str| {
        if fold {
            value
                .to_ascii_lowercase()
                .starts_with(&format!("{}/", prefix.to_ascii_lowercase()))
        } else {
            value.starts_with(&format!("{prefix}/"))
        }
    };

    matches(&anchor, &target)
}

fn has_git_entry(directory: &Path) -> bool {
    std::fs::symlink_metadata(directory.join(".git")).is_ok()
}

/// Remove empty dated directories the removed workspace left behind.
///
/// Stops at the anchor: `<Documents>/Codex` is Codex's own directory and is never
/// deleted, only the dated directories it creates go away when they empty.
fn prune_empty_dated_ancestors(target: &Path, anchor: &Path) {
    let anchor_display = display_path(anchor);
    let anchor_display = anchor_display.trim_end_matches(['/', '\\']).to_string();

    let mut current = target.parent().map(Path::to_path_buf);
    while let Some(directory) = current {
        let display = display_path(&directory);
        let display = display.trim_end_matches(['/', '\\']).to_string();
        if same_path_for_compare(&display, &anchor_display) {
            return;
        }

        let Some(name) = directory.file_name().and_then(|name| name.to_str()) else {
            return;
        };
        // Only dated directories are pruned; anything else was not created by
        // the scratch flow and is left alone.
        if !is_date_segment(name) {
            return;
        }

        let is_link = std::fs::symlink_metadata(&directory)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(true);
        if is_link {
            return;
        }

        let is_empty = std::fs::read_dir(&directory)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if !is_empty {
            return;
        }

        if std::fs::remove_dir(&directory).is_err() {
            return;
        }
        current = directory.parent().map(Path::to_path_buf);
    }
}

fn list_dated_dirs(anchor: &str) -> Vec<PathBuf> {
    let mut dated = Vec::new();
    let Ok(entries) = std::fs::read_dir(anchor) else {
        return dated;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !is_date_segment(&name) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() && !file_type.is_symlink() {
            dated.push(entry.path());
        }
    }

    dated.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
    dated
}

/// The `<Documents>/Codex` anchor of a dated directory.
///
/// A dated directory has no workspace below it, so it is *not* a scratch
/// workspace; this is the placing check for the empty-directory cleanup.
pub fn codex_scratch_date_anchor(path: &str) -> Option<String> {
    anchor_of_date_dir(path)
}

fn anchor_of_date_dir(path: &str) -> Option<String> {
    let (prefix, rest) = split_prefix(path);
    let segments: Vec<String> = rest
        .split(['/', '\\'])
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect();
    if segments.len() < 3 {
        return None;
    }
    let last = segments.last()?;
    if !is_date_segment(last) {
        return None;
    }
    let root = segments.get(segments.len() - 2)?;
    let parent = segments.get(segments.len() - 3)?;
    if !root.eq_ignore_ascii_case(SCRATCH_ROOT_SEGMENT)
        || !parent.eq_ignore_ascii_case(SCRATCH_PARENT_SEGMENT)
    {
        return None;
    }

    let separator = if path.contains('\\') { "\\" } else { "/" };
    Some(join_segments(
        &prefix,
        &segments[..segments.len() - 1],
        separator,
    ))
}

// ---------------------------------------------------------------------------
// Path comparison helpers
// ---------------------------------------------------------------------------

fn normalize_for_compare(value: &str) -> String {
    let mut normalized = value.trim().replace('\\', "/");

    // Verbatim prefixes name the same place as their plain form.
    if let Some(rest) = normalized.strip_prefix("//?/UNC/") {
        normalized = format!("//{rest}");
    } else if let Some(rest) = normalized.strip_prefix("//?/") {
        normalized = rest.to_string();
    }

    if let Some(linux) = wsl_unc_to_linux(&normalized) {
        return linux;
    }

    while normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }
    normalized
}

fn wsl_unc_to_linux(value: &str) -> Option<String> {
    let rest = value
        .strip_prefix("//wsl.localhost/")
        .or_else(|| value.strip_prefix("//wsl$/"))?;
    let (_distro, linux_path) = rest.split_once('/')?;
    Some(format!("/{linux_path}"))
}

fn is_windows_drive_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

/// A host path in the platform's exact spelling, or `None` when nothing along it
/// exists (a WSL-runtime Linux path on a Windows host, for instance).
fn canonicalize_lenient(value: &str) -> Option<String> {
    canonicalize_lenient_path(Path::new(value)).map(|path| display_path(&path))
}

/// Canonicalize the deepest existing ancestor and re-append the rest, so a path
/// whose own directory is already gone still resolves for comparison.
fn canonicalize_lenient_path(path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return Some(strip_verbatim_prefix(canonical));
    }

    let mut trailing: Vec<std::ffi::OsString> = Vec::new();
    let mut current = path.to_path_buf();
    loop {
        let parent = current.parent()?.to_path_buf();
        let name = current.file_name().map(|name| name.to_os_string())?;
        trailing.push(name);

        if let Ok(canonical) = std::fs::canonicalize(&parent) {
            let mut resolved = strip_verbatim_prefix(canonical);
            for name in trailing.iter().rev() {
                resolved.push(name);
            }
            return Some(resolved);
        }
        current = parent;
    }
}

fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    let display = path.to_string_lossy().to_string();
    if let Some(rest) = display.strip_prefix("\\\\?\\UNC\\") {
        return PathBuf::from(format!("\\\\{rest}"));
    }
    if let Some(rest) = display.strip_prefix("\\\\?\\") {
        return PathBuf::from(rest);
    }
    path
}

// ---------------------------------------------------------------------------
// config.toml `[projects]`
// ---------------------------------------------------------------------------

/// The `[projects]` keys of a config.toml, decoded.
///
/// `TableLike::iter` yields the key's decoded value, which is also what
/// `TableLike::remove` matches on — unlike `Key`'s `Display`, which re-quotes a
/// key that needs escaping.
fn project_table_keys(config_toml: &str) -> Result<Vec<String>, String> {
    let document = config_toml
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| format!("Failed to parse config.toml: {error}"))?;

    let Some(projects) = document
        .get("projects")
        .and_then(|item| item.as_table_like())
    else {
        return Ok(Vec::new());
    };

    let mut keys: Vec<String> = projects.iter().map(|(key, _)| key.to_string()).collect();
    keys.sort();
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "ai-toolbox-codex-scratch-{label}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&path).expect("failed to create test directory");
        path
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            Self {
                path: temp_dir(label),
            }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn scratch_workspace(root: &Path, date: &str, slug: &str) -> PathBuf {
        let path = root.join("Documents").join("Codex").join(date).join(slug);
        std::fs::create_dir_all(&path).expect("failed to create scratch workspace");
        path
    }

    #[test]
    fn recognizes_the_observed_scratch_layout() {
        assert!(is_codex_scratch_workspace(
            r"c:\users\me\documents\codex\2026-08-31\new-chat"
        ));
        assert!(is_codex_scratch_workspace(
            "/Users/me/Documents/Codex/2026-09-25/xi"
        ));
        assert!(is_codex_scratch_workspace(
            r"\\wsl.localhost\Ubuntu\home\me\Documents\Codex\2026-09-25\xi"
        ));
        assert!(is_codex_scratch_workspace(
            r"\\?\C:\Users\me\Documents\Codex\2026-09-25\xi\nested"
        ));
    }

    #[test]
    fn rejects_real_projects_and_malformed_shapes() {
        assert!(!is_codex_scratch_workspace(r"d:\github\ai-toolbox"));
        assert!(!is_codex_scratch_workspace("/Users/me/projects/app"));
        // A dated directory with nothing below it is not a workspace.
        assert!(!is_codex_scratch_workspace(
            "/Users/me/Documents/Codex/2026-09-25"
        ));
        // `Codex` without the `Documents` parent is some other directory.
        assert!(!is_codex_scratch_workspace("/opt/Codex/2026-09-25/xi"));
        // A date-shaped slug above the dated directory must not shift the anchor.
        assert!(!is_codex_scratch_workspace(
            "/Users/me/Documents/Codex/notes/2026-09-25/xi"
        ));
        assert!(!is_codex_scratch_workspace(
            "/Users/me/Documents/Codex/2026-9-25/xi"
        ));
        assert!(!is_codex_scratch_workspace(
            "/Users/me/Documents/Codex/2026-09-25/../secret"
        ));
    }

    #[test]
    fn anchor_keeps_the_input_separator_style() {
        assert_eq!(
            codex_scratch_anchor(r"c:\users\me\documents\codex\2026-08-31\new-chat").as_deref(),
            Some(r"c:\users\me\documents\codex")
        );
        assert_eq!(
            codex_scratch_anchor("/Users/me/Documents/Codex/2026-09-25/xi").as_deref(),
            Some("/Users/me/Documents/Codex")
        );
        assert_eq!(
            codex_scratch_anchor(r"\\wsl.localhost\Ubuntu\home\me\Documents\Codex\2026-09-25\xi")
                .as_deref(),
            Some(r"\\wsl.localhost\Ubuntu\home\me\Documents\Codex")
        );
        assert_eq!(codex_scratch_anchor("/Users/me/other/2026-09-25/xi"), None);
    }

    #[test]
    fn path_comparison_folds_case_only_for_windows_shapes() {
        assert!(same_path_for_compare(
            r"C:\Users\me\Documents\Codex\2026-08-31\new-chat",
            r"c:\users\me\documents\codex\2026-08-31\new-chat"
        ));
        // A WSL key is a case-sensitive Linux path.
        assert!(!same_path_for_compare(
            "/home/me/Documents/Codex/2026-09-25/xi",
            "/home/me/documents/codex/2026-09-25/xi"
        ));
        // A host-side UNC spelling of a WSL path compares as that Linux path.
        assert!(same_path_for_compare(
            r"\\wsl.localhost\Ubuntu\home\me\Documents\Codex\2026-09-25\xi",
            "/home/me/Documents/Codex/2026-09-25/xi"
        ));
        assert!(same_path_for_compare(
            r"\\?\C:\Users\me\Documents\Codex\2026-09-25\xi",
            r"C:\Users\me\Documents\Codex\2026-09-25\xi"
        ));
        assert!(!same_path_for_compare(
            "/home/me/Documents/Codex/2026-09-25/xi",
            "/home/me/Documents/Codex/2026-09-25/other"
        ));
    }

    #[test]
    fn trust_key_candidates_include_the_canonical_spelling() {
        let test_dir = TestDir::new("candidates");
        let workspace = scratch_workspace(test_dir.path(), "2026-09-25", "xi");

        let candidates = scratch_trust_key_candidates(&workspace.to_string_lossy());
        let canonical = std::fs::canonicalize(&workspace).expect("workspace should exist");
        let canonical = strip_verbatim_prefix(canonical);

        assert!(candidates
            .iter()
            .any(|candidate| { same_path_for_compare(candidate, &canonical.to_string_lossy()) }));
    }

    #[test]
    fn inspect_reports_emptiness_and_contents() {
        let test_dir = TestDir::new("inspect");
        let workspace = scratch_workspace(test_dir.path(), "2026-09-25", "xi");

        let empty = inspect_workspace(&workspace.to_string_lossy());
        assert!(empty.exists);
        assert!(empty.is_empty);
        assert_eq!(empty.file_count, 0);
        assert!(!empty.has_git);

        std::fs::write(workspace.join("main.py"), b"print('hi')").expect("write should succeed");
        std::fs::create_dir_all(workspace.join("src")).expect("nested dir should be created");
        std::fs::write(workspace.join("src").join("a.py"), b"x").expect("write should succeed");

        let filled = inspect_workspace(&workspace.to_string_lossy());
        assert!(filled.exists);
        assert!(!filled.is_empty);
        assert_eq!(filled.file_count, 2);
        assert_eq!(filled.total_bytes, 12);

        let missing = inspect_workspace(&test_dir.path().join("nope").to_string_lossy());
        assert!(!missing.exists);
    }

    /// A workspace holding only empty directories — which is exactly what Codex
    /// leaves in a chat that produced nothing — has nothing to lose, so it counts
    /// as empty and is preselected. A linked entry is content: deleting the
    /// workspace drops the link.
    #[test]
    fn only_empty_directories_still_count_as_empty() {
        let test_dir = TestDir::new("inspect-empty-dirs");
        let workspace = scratch_workspace(test_dir.path(), "2026-09-25", "xi");
        std::fs::create_dir_all(workspace.join("work").join("nested"))
            .expect("nested empty directories should be created");
        std::fs::create_dir_all(workspace.join("outputs")).expect("outputs should be created");

        let info = inspect_workspace(&workspace.to_string_lossy());
        assert!(info.exists);
        assert!(info.is_empty, "empty directories are not content: {info:?}");
        assert_eq!(info.file_count, 0);
        assert!(!info.truncated);

        // A file anywhere below makes it something to be careful about.
        std::fs::write(workspace.join("work").join("nested").join("out.txt"), b"x")
            .expect("write should succeed");
        let filled = inspect_workspace(&workspace.to_string_lossy());
        assert!(!filled.is_empty);
        assert_eq!(filled.file_count, 1);
    }

    #[test]
    fn removal_deletes_the_workspace_and_its_empty_date_directory() {
        let test_dir = TestDir::new("remove");
        let workspace = scratch_workspace(test_dir.path(), "2026-09-25", "xi");
        std::fs::write(workspace.join("note.md"), b"hello").expect("write should succeed");

        remove_workspace(&workspace.to_string_lossy()).expect("removal should succeed");

        assert!(!workspace.exists());
        assert!(!test_dir
            .path()
            .join("Documents")
            .join("Codex")
            .join("2026-09-25")
            .exists());
        // The anchor itself stays.
        assert!(test_dir.path().join("Documents").join("Codex").exists());
    }

    #[test]
    fn removal_keeps_a_date_directory_that_still_has_other_workspaces() {
        let test_dir = TestDir::new("keep-date-dir");
        let first = scratch_workspace(test_dir.path(), "2026-09-25", "xi");
        let second = scratch_workspace(test_dir.path(), "2026-09-25", "other");

        remove_workspace(&first.to_string_lossy()).expect("removal should succeed");

        assert!(second.exists());
        assert!(test_dir
            .path()
            .join("Documents")
            .join("Codex")
            .join("2026-09-25")
            .exists());
    }

    #[test]
    fn removal_refuses_git_repositories_and_foreign_paths() {
        let test_dir = TestDir::new("refuse");
        let workspace = scratch_workspace(test_dir.path(), "2026-09-25", "xi");
        std::fs::create_dir_all(workspace.join(".git")).expect("git dir should be created");

        let error = remove_workspace(&workspace.to_string_lossy())
            .expect_err("a git repository must be refused");
        assert!(
            error.contains("git repository"),
            "unexpected error: {error}"
        );
        assert!(workspace.exists());

        let real_project = test_dir.path().join("projects").join("app");
        std::fs::create_dir_all(&real_project).expect("project should be created");
        let error = remove_workspace(&real_project.to_string_lossy())
            .expect_err("a real project path must be refused");
        assert!(
            error.contains("not a Codex scratch workspace"),
            "unexpected error: {error}"
        );
        assert!(real_project.exists());
    }

    #[test]
    fn removal_refuses_a_nested_git_repository() {
        let test_dir = TestDir::new("refuse-nested");
        let workspace = scratch_workspace(test_dir.path(), "2026-09-25", "xi");
        std::fs::write(workspace.join(".git"), b"gitdir: elsewhere").expect("write should succeed");

        let error = remove_workspace(&workspace.to_string_lossy())
            .expect_err("a git worktree file must be refused");
        assert!(
            error.contains("git repository"),
            "unexpected error: {error}"
        );
        assert!(workspace.exists());
    }

    /// A repository the user created *inside* the scratch directory — a clone,
    /// say — is still a real project, and removal must refuse it. The guard
    /// cannot only look at the workspace root.
    #[test]
    fn removal_refuses_a_repository_below_the_workspace() {
        let test_dir = TestDir::new("refuse-below");
        let workspace = scratch_workspace(test_dir.path(), "2026-09-25", "xi");
        let clone = workspace.join("repo");
        std::fs::create_dir_all(clone.join(".git")).expect("clone should be created");
        std::fs::write(clone.join("main.rs"), b"fn main() {}").expect("write should succeed");

        let error = remove_workspace(&workspace.to_string_lossy())
            .expect_err("a repository inside the workspace must be refused");
        assert!(
            error.contains("git repository"),
            "unexpected error: {error}"
        );
        assert!(clone.join("main.rs").exists());

        // The same question is answered for the cleanup UI before it offers the
        // directory at all.
        let info = inspect_workspace(&workspace.to_string_lossy());
        assert!(info.has_git, "a nested repository must be reported");
        assert!(!info.is_empty);
    }

    #[test]
    fn removal_of_a_missing_workspace_converges() {
        let test_dir = TestDir::new("missing");
        let workspace = scratch_workspace(test_dir.path(), "2026-09-25", "xi");
        std::fs::remove_dir_all(&workspace).expect("cleanup should succeed");

        remove_workspace(&workspace.to_string_lossy()).expect("removal should converge");
    }

    #[test]
    fn lists_workspaces_and_empty_date_dirs() {
        let test_dir = TestDir::new("list");
        let workspace = scratch_workspace(test_dir.path(), "2026-09-25", "xi");
        scratch_workspace(test_dir.path(), "2026-09-24", "other");
        let empty_date = test_dir
            .path()
            .join("Documents")
            .join("Codex")
            .join("2026-09-23");
        std::fs::create_dir_all(&empty_date).expect("empty date dir should be created");
        // Not a dated directory: never listed.
        std::fs::create_dir_all(test_dir.path().join("Documents").join("Codex").join("logs"))
            .expect("logs dir should be created");

        let anchor = test_dir
            .path()
            .join("Documents")
            .join("Codex")
            .to_string_lossy()
            .to_string();

        let workspaces = list_scratch_workspaces(&anchor);
        assert_eq!(workspaces.len(), 2);
        assert!(workspaces
            .iter()
            .any(|path| same_path_for_compare(path, &workspace.to_string_lossy())));

        let empty_dirs = list_empty_date_dirs(&anchor);
        assert_eq!(empty_dirs.len(), 1);
        assert!(empty_dirs[0].ends_with("2026-09-23"));

        remove_empty_date_dir(&empty_dirs[0]).expect("empty date dir removal should succeed");
        assert!(!empty_date.exists());
    }

    #[test]
    fn empty_date_dir_removal_refuses_a_non_empty_directory() {
        let test_dir = TestDir::new("date-dir-non-empty");
        scratch_workspace(test_dir.path(), "2026-09-25", "xi");
        let date_dir = test_dir
            .path()
            .join("Documents")
            .join("Codex")
            .join("2026-09-25");

        let error = remove_empty_date_dir(&date_dir.to_string_lossy())
            .expect_err("a non-empty date dir must be refused");
        assert!(
            error.contains("no longer empty"),
            "unexpected error: {error}"
        );
        assert!(date_dir.exists());
    }

    #[test]
    fn lists_and_removes_only_scratch_trust_entries() {
        let config = r#"#:schema none
model = "gpt-5.5"

[projects."/Users/me/Documents/Codex/2026-09-25/xi"]
trust_level = "trusted"

[projects.'c:\users\me\documents\codex\2026-08-31\new-chat']
trust_level = "trusted"

[projects."/Users/me/projects/app"]
trust_level = "trusted"

[projects."/Users/me"]
trust_level = "trusted"
"#;

        let entries = list_scratch_trust_entries(config).expect("config should list");
        assert_eq!(entries.len(), 2);
        let keys: Vec<&str> = entries.iter().map(|entry| entry.key.as_str()).collect();
        assert!(keys.contains(&"/Users/me/Documents/Codex/2026-09-25/xi"));
        assert!(keys.contains(&r"c:\users\me\documents\codex\2026-08-31\new-chat"));
    }

    #[test]
    fn finds_trust_keys_by_path_not_by_spelling() {
        let config = "[projects.'c:\\users\\me\\documents\\codex\\2026-08-31\\new-chat']\ntrust_level = \"trusted\"\n";

        let keys = find_trust_keys(config, r"C:\Users\me\Documents\Codex\2026-08-31\new-chat")
            .expect("config should list");
        assert_eq!(keys.len(), 1);

        // A different workspace in the same tree must not match.
        let keys = find_trust_keys(config, r"C:\Users\me\Documents\Codex\2026-08-31\other")
            .expect("config should list");
        assert!(keys.is_empty());
    }

    #[test]
    fn removes_scratch_trust_entries_and_preserves_the_rest_of_the_file() {
        let test_dir = TestDir::new("trust-remove");
        let config_path = test_dir.path().join("config.toml");
        let config = r#"#:schema none
# kept comment
model = "gpt-5.5"

[projects."/Users/me/Documents/Codex/2026-09-25/xi"]
trust_level = "trusted"

# kept project comment
[projects."/Users/me/projects/app"]
trust_level = "trusted"
"#;
        std::fs::write(&config_path, config).expect("write should succeed");

        let keys = list_scratch_trust_entries(config)
            .expect("config should list")
            .into_iter()
            .map(|entry| entry.key)
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 1);

        let removed = remove_trust_keys(&config_path, &keys).expect("removal should succeed");
        assert_eq!(removed, keys);

        let written = std::fs::read_to_string(&config_path).expect("read should succeed");
        assert!(!written.contains("2026-09-25"));
        assert!(written.contains(r#"[projects."/Users/me/projects/app"]"#));
        assert!(written.contains("# kept comment"));
        assert!(written.contains("# kept project comment"));
        assert!(written.starts_with("#:schema none\n"));
        assert!(written.contains("model = \"gpt-5.5\""));
    }

    #[test]
    fn removes_the_projects_table_when_it_empties() {
        let test_dir = TestDir::new("trust-empty");
        let config_path = test_dir.path().join("config.toml");
        let config =
            "[projects.\"/Users/me/Documents/Codex/2026-09-25/xi\"]\ntrust_level = \"trusted\"\n";
        std::fs::write(&config_path, config).expect("write should succeed");

        let keys = list_scratch_trust_entries(config)
            .expect("config should list")
            .into_iter()
            .map(|entry| entry.key)
            .collect::<Vec<_>>();
        assert_eq!(
            remove_trust_keys(&config_path, &keys).expect("removal should succeed"),
            keys
        );

        let written = std::fs::read_to_string(&config_path).expect("read should succeed");
        assert!(!written.contains("projects"));
    }

    #[test]
    fn trust_removal_is_a_noop_without_a_match_or_a_file() {
        let test_dir = TestDir::new("trust-noop");
        let config_path = test_dir.path().join("config.toml");
        std::fs::write(&config_path, "model = \"gpt-5.5\"\n").expect("write should succeed");

        assert!(remove_trust_keys(
            &config_path,
            &["/Users/me/Documents/Codex/2026-09-25/xi".to_string()]
        )
        .expect("removal should succeed")
        .is_empty());
        assert_eq!(
            std::fs::read_to_string(&config_path).expect("read should succeed"),
            "model = \"gpt-5.5\"\n"
        );

        let missing = test_dir.path().join("missing.toml");
        assert!(remove_trust_keys(&missing, &["any".to_string()])
            .expect("removal should converge")
            .is_empty());
    }
}
