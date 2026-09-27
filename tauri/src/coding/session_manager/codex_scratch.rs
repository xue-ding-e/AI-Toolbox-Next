//! Codex scratch-workspace cleanup plumbing.
//!
//! Codex creates a workspace directory and a `[projects]` trust entry for every
//! project-less chat and never reclaims either when the chat is deleted. Two
//! entry points clean them up:
//!
//! - **Delete-time**: deleting a Codex session can optionally take its own
//!   workspace and trust entry along ([`CodexCleanupOptions`]).
//! - **Residue scan**: [`scan_residue`] lists leftovers whose sessions are gone,
//!   and [`clean_residue`] removes the selected ones.
//!
//! Both funnel into [`run_cleanup`], so the guards are identical: nothing is
//! removed while another rollout — active *or* archived, plain *or* compressed —
//! still records that directory as its `cwd`, and nothing is removed when that
//! reference set could not be read in full. Every removal also re-checks the
//! target at execution time, because the confirmation dialog may be minutes old.
//!
//! The cleanup result is reported separately from the session deletion it
//! followed: a cleanup failure must never turn a successful delete into a
//! failure.
//!
//! Directory-shape and `config.toml` facts live in
//! `crate::coding::codex::scratch_workspace`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::coding::codex::scratch_workspace as scratch;
use crate::coding::codex::scratch_workspace::ScratchWorkspaceInfo;
use crate::coding::runtime_location;

use super::codex_rollout;
use super::{
    SessionContextEntry, SessionContextSet, SessionMeta, SessionRuntimeSource, SessionSourceMode,
    ToolSessionContext,
};

/// Whether a delete should also clean up the session's Codex scratch residue.
#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CodexCleanupOptions {
    #[serde(default)]
    pub remove_workspace: bool,
    #[serde(default)]
    pub remove_trust_entry: bool,
}

impl CodexCleanupOptions {
    pub(super) fn is_empty(self) -> bool {
        !self.remove_workspace && !self.remove_trust_entry
    }
}

/// One item the cleanup could not act on, with the reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCleanupSkip {
    pub target: String,
    pub reason: String,
}

/// One item the cleanup tried to remove and failed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCleanupFailure {
    pub target: String,
    pub error: String,
}

/// What an auxiliary Codex cleanup did.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CodexCleanupSummary {
    pub removed_workspaces: Vec<String>,
    pub removed_trust_keys: Vec<String>,
    pub removed_date_dirs: Vec<String>,
    pub skipped: Vec<CodexCleanupSkip>,
    pub failures: Vec<CodexCleanupFailure>,
}

impl CodexCleanupSummary {
    pub(super) fn is_empty(&self) -> bool {
        self.removed_workspaces.is_empty()
            && self.removed_trust_keys.is_empty()
            && self.removed_date_dirs.is_empty()
            && self.skipped.is_empty()
            && self.failures.is_empty()
    }

    fn push_skip(&mut self, target: &str, reason: &str) {
        self.skipped.push(CodexCleanupSkip {
            target: target.to_string(),
            reason: reason.to_string(),
        });
    }

    fn push_failure(&mut self, target: &str, error: String) {
        self.failures.push(CodexCleanupFailure {
            target: target.to_string(),
            error,
        });
    }
}

/// One session's cleanup candidates, for the delete confirmation dialog.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCleanupPreviewItem {
    pub source_path: String,
    /// The working directory the rollout recorded.
    pub project_dir: String,
    /// Present only when that directory matches the scratch layout.
    pub workspace: Option<ScratchWorkspaceInfo>,
    /// `[projects]` keys naming that directory.
    pub trust_keys: Vec<String>,
    pub config_path: String,
    pub runtime_source: String,
    pub runtime_distro: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCleanupPreview {
    pub items: Vec<CodexCleanupPreviewItem>,
}

/// One workspace the residue scan found unreferenced.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexScratchResidueWorkspace {
    pub info: ScratchWorkspaceInfo,
    pub runtime_source: String,
    pub runtime_distro: Option<String>,
}

/// One `[projects]` trust entry the residue scan found unreferenced.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexScratchResidueTrustEntry {
    pub key: String,
    pub config_path: String,
    pub dir_exists: bool,
    pub runtime_source: String,
    pub runtime_distro: Option<String>,
}

/// Everything the residue scan found, merged across the selected sources.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexScratchResidue {
    pub source: String,
    /// `false` when the requested source has no Codex home at all.
    pub unavailable: bool,
    /// `false` when a rollout scan stopped at its ceiling, so "unreferenced"
    /// could not be proven. The UI must offer no cleanup in that case.
    pub scan_complete: bool,
    pub config_paths: Vec<String>,
    pub workspaces: Vec<CodexScratchResidueWorkspace>,
    pub trust_entries: Vec<CodexScratchResidueTrustEntry>,
    pub empty_date_dirs: Vec<String>,
}

/// One cleanup target, resolved from a session or from the residue scan.
#[derive(Debug, Clone)]
pub(super) struct CodexCleanupRequest {
    /// What skip/failure messages name.
    pub display: String,
    /// Host path of the workspace (the UNC spelling for a WSL runtime).
    pub workspace_path: String,
    /// The working directory as Codex recorded it, used for reference matching.
    pub project_dir: String,
    pub config_path: PathBuf,
    /// The `sessions/` root whose rollouts decide whether this is leftovers.
    pub sessions_root: PathBuf,
    /// What this request authorizes.
    ///
    /// A delete asks about the workspace and the trust entry together, so both
    /// are open and the user's two checkboxes decide. The residue dialog selects
    /// each row on its own, so a row authorizes exactly what it names — a checked
    /// trust entry must never take an unchecked directory with it.
    pub scope: CodexCleanupScope,
}

/// The actions one cleanup request may perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CodexCleanupScope {
    pub workspace: bool,
    pub trust_entry: bool,
}

impl CodexCleanupScope {
    /// A request from the delete dialog, which offers both actions at once.
    pub(super) const BOTH: Self = Self {
        workspace: true,
        trust_entry: true,
    };
    /// A residue row for one worked directory.
    pub(super) const WORKSPACE_ONLY: Self = Self {
        workspace: true,
        trust_entry: false,
    };
    /// A residue row for one `[projects]` entry.
    pub(super) const TRUST_ENTRY_ONLY: Self = Self {
        workspace: false,
        trust_entry: true,
    };
}

/// A Codex context that can hold scratch residue.
struct ResidueSource {
    entry: SessionContextEntry,
    config_path: PathBuf,
    config_toml: Option<String>,
    /// `None` when the rollout scan could not be completed.
    references: Option<ReferenceKeys>,
    anchors: Vec<String>,
}

impl ResidueSource {
    fn source_label(&self) -> String {
        self.entry.source.as_str().to_string()
    }

    /// Whether a directory another rollout still uses is named by `path`.
    fn is_referenced(&self, path: &str) -> bool {
        self.references
            .as_ref()
            .is_some_and(|references| is_referenced_in(references, path))
    }
}

/// The working directories one sessions root still records, in comparison form.
///
/// Keyed once per root so the per-item question is a hash lookup instead of a
/// scan over every recorded directory — a residue scan asks it once per listed
/// workspace and trust key, against thousands of rollouts.
type ReferenceKeys = HashSet<String>;

fn reference_keys(scan: &codex_rollout::RolloutCwds) -> ReferenceKeys {
    scan.cwds
        .iter()
        .map(|cwd| scratch::path_compare_key(cwd))
        .collect()
}

/// Whether any rollout still records `path` as its working directory.
///
/// The comparison tries every spelling `path` can have, not just the one the
/// caller happens to hold: a redirected or symlinked documents directory makes
/// the same workspace reachable under two names, and a rollout may have recorded
/// either of them. Missing a spelling here would call a live workspace leftovers.
fn is_referenced_in(references: &ReferenceKeys, path: &str) -> bool {
    scratch::path_compare_candidates(path)
        .iter()
        .any(|candidate| references.contains(candidate))
}

/// The Codex home and config path of a Codex session context.
fn codex_context_paths(entry: &SessionContextEntry) -> Option<(PathBuf, PathBuf)> {
    let ToolSessionContext::Codex { codex_home, .. } = &entry.context else {
        return None;
    };
    let codex_home = codex_home.clone()?;
    let config_path = codex_home.join("config.toml");
    Some((codex_home, config_path))
}

fn sessions_root_of(entry: &SessionContextEntry) -> Option<PathBuf> {
    let ToolSessionContext::Codex { sessions_root, .. } = &entry.context else {
        return None;
    };
    Some(sessions_root.clone())
}

/// The host-side path of a recorded working directory.
///
/// A WSL session's rollout records the Linux path Codex saw inside the distro,
/// while every file operation from the host side needs the UNC spelling. The
/// conversion is idempotent: paths this scan already derived from an anchor are
/// UNC, and converting them again would nest a second distro prefix.
fn host_path_of(entry: &SessionContextEntry, path: &str) -> String {
    if entry.source != SessionRuntimeSource::Wsl {
        return path.to_string();
    }
    if runtime_location::parse_wsl_unc_path(path).is_some() {
        return path.to_string();
    }
    let Some(distro) = entry.distro.as_deref() else {
        return path.to_string();
    };
    runtime_location::build_windows_unc_path(distro, path)
        .to_string_lossy()
        .to_string()
}

/// Whether a host path is a Codex scratch workspace.
///
/// The shape check is spelling-agnostic, so a Linux path, a Windows path and a
/// WSL UNC path all answer the same way.
fn is_scratch_workspace(path: &str) -> bool {
    scratch::is_codex_scratch_workspace(path)
}

/// Resolve one session's cleanup target. `None` when the session has no Codex
/// context or no recorded working directory.
fn cleanup_request(
    entry: &SessionContextEntry,
    session: &SessionMeta,
) -> Option<CodexCleanupRequest> {
    let (_, config_path) = codex_context_paths(entry)?;
    let sessions_root = sessions_root_of(entry)?;
    let project_dir = session.project_dir.as_deref()?.trim();
    if project_dir.is_empty() {
        return None;
    }

    let workspace_path = host_path_of(entry, project_dir);
    Some(CodexCleanupRequest {
        display: workspace_path.clone(),
        workspace_path,
        project_dir: project_dir.to_string(),
        config_path,
        sessions_root,
        scope: CodexCleanupScope::BOTH,
    })
}

/// Resolve the requested sessions with one list per context.
///
/// The delete path resolves each session on its own (and must keep doing so: it
/// is the authority on which session a row means). Cleanup only needs the
/// sessions' working directories, and doing that per path would rescan the whole
/// Codex home once per selected row.
///
/// The list cache is good enough here — and keeps a delete from paying for an
/// extra full scan it never paid for before this feature: the cleanup re-checks
/// every target against the rollouts that are still on disk, so a cached
/// `project_dir` can only ever make the dialog offer *less* than it could.
fn resolve_sessions(
    contexts: &SessionContextSet,
    source_paths: &[String],
) -> Vec<(SessionContextEntry, SessionMeta)> {
    if source_paths.is_empty() {
        return Vec::new();
    }

    let mut resolved = Vec::new();
    let mut seen = HashSet::new();

    for entry in &contexts.entries {
        for session in super::get_cached_sessions(&entry.context, false) {
            if !source_paths.iter().any(|source_path| {
                super::matches_session_source(&entry.context, &session.source_path, source_path)
            }) {
                continue;
            }
            if !seen.insert((
                entry.source.as_str(),
                super::session_dedupe_key(&entry.context, &session.source_path),
            )) {
                continue;
            }

            let session = super::annotate_session_source(
                super::canonical_session_meta(entry, session),
                entry,
            );
            resolved.push((entry.clone(), session));
        }
    }

    resolved
}

/// The cleanup candidates of the sessions behind a delete confirmation dialog.
pub(super) fn preview(
    contexts: &SessionContextSet,
    source_paths: &[String],
) -> CodexCleanupPreview {
    let mut items = Vec::new();

    for (entry, session) in resolve_sessions(contexts, source_paths) {
        let Some(request) = cleanup_request(&entry, &session) else {
            continue;
        };
        if !is_scratch_workspace(&request.workspace_path) {
            continue;
        }

        let trust_keys = std::fs::read_to_string(&request.config_path)
            .ok()
            .and_then(|config_toml| {
                scratch::find_trust_keys(&config_toml, &request.project_dir).ok()
            })
            .unwrap_or_default();

        items.push(CodexCleanupPreviewItem {
            source_path: session.source_path.clone(),
            project_dir: request.project_dir.clone(),
            workspace: Some(scratch::inspect_workspace(&request.workspace_path)),
            trust_keys,
            config_path: request.config_path.to_string_lossy().to_string(),
            runtime_source: entry.source.as_str().to_string(),
            runtime_distro: entry.distro.clone(),
        });
    }

    CodexCleanupPreview { items }
}

/// The cleanup requests of the sessions being deleted.
///
/// Resolved before the rollouts disappear: the working directory being cleaned is
/// recorded in the rollout that is about to be removed.
///
/// Only project-less chats produce a request. A real project's session would be
/// refused by the cleanup's own shape guard anyway, but it would be *reported* as
/// a leftover that was skipped — a warning about a directory that was never a
/// cleanup target, shown right after a deletion that went perfectly.
pub(super) fn delete_requests(
    contexts: &SessionContextSet,
    source_paths: &[String],
) -> Vec<CodexCleanupRequest> {
    resolve_sessions(contexts, source_paths)
        .into_iter()
        .filter_map(|(entry, session)| cleanup_request(&entry, &session))
        .filter(|request| is_scratch_workspace(&request.workspace_path))
        .collect()
}

/// Execute a cleanup over already-resolved requests.
///
/// Each request is re-checked here instead of being trusted from the caller: a
/// workspace is only removed while no rollout still records it.
pub(super) fn run_cleanup(
    requests: &[CodexCleanupRequest],
    options: CodexCleanupOptions,
    summary: &mut CodexCleanupSummary,
) {
    run_cleanup_with_references(requests, options, summary, &HashMap::new());
}

/// [`run_cleanup`] with the rollout scans a caller has already performed.
///
/// A residue cleanup reads the same reference sets to decide what to offer, so it
/// hands them over rather than making the walk twice.
pub(super) fn run_cleanup_with_references(
    requests: &[CodexCleanupRequest],
    options: CodexCleanupOptions,
    summary: &mut CodexCleanupSummary,
    known_references: &HashMap<PathBuf, Option<ReferenceKeys>>,
) {
    if options.is_empty() {
        return;
    }

    let mut config_cache: HashMap<PathBuf, ConfigRead> = HashMap::new();
    let mut trust_keys_by_config: HashMap<PathBuf, Vec<String>> = HashMap::new();
    let mut seen_workspaces: HashSet<String> = HashSet::new();
    // One rollout scan per sessions root; it is the expensive part and several
    // requests share a root.
    let mut reference_cache: HashMap<PathBuf, Option<ReferenceKeys>> = known_references.clone();

    for request in requests {
        let references = reference_cache
            .entry(request.sessions_root.clone())
            .or_insert_with(|| {
                let scan = codex_rollout::rollout_cwds(&request.sessions_root);
                scan.complete.then(|| reference_keys(&scan))
            });

        let referenced = references.as_ref().is_some_and(|references| {
            is_referenced_in(references, &request.project_dir)
                || is_referenced_in(references, &request.workspace_path)
        });

        let scratch_shaped = is_scratch_workspace(&request.workspace_path);
        let skip_reason = if !scratch_shaped {
            Some("not a Codex scratch workspace")
        } else if references.is_none() {
            Some("the Codex rollout scan was incomplete")
        } else if referenced {
            Some("another Codex session still uses this directory")
        } else {
            None
        };

        if options.remove_workspace
            && request.scope.workspace
            && seen_workspaces.insert(scratch::path_compare_key(&request.workspace_path))
        {
            match skip_reason {
                Some(reason) => summary.push_skip(&request.display, reason),
                None => {
                    let info = scratch::inspect_workspace(&request.workspace_path);
                    if info.has_git {
                        summary
                            .push_skip(&request.display, "the directory contains a git repository");
                    } else if !info.exists {
                        // Already gone: there is nothing to remove and nothing to
                        // report — counting it as removed would overstate what the
                        // cleanup changed.
                    } else {
                        match scratch::remove_workspace(&request.workspace_path) {
                            Ok(()) => summary
                                .removed_workspaces
                                .push(request.workspace_path.clone()),
                            Err(error) => summary.push_failure(&request.display, error),
                        }
                    }
                }
            }
        }

        // Only project-less chats have trust residue: a real project's trust
        // entry is Codex's own setting and is left alone.
        if options.remove_trust_entry && request.scope.trust_entry && scratch_shaped {
            match skip_reason {
                Some(reason) => summary.push_skip(&request.display, reason),
                None => {
                    let config_toml = match read_config(&request.config_path, &mut config_cache) {
                        // No config file means no trust entry, which is already
                        // the outcome the user asked for.
                        ConfigRead::Missing => continue,
                        ConfigRead::Read(config_toml) => config_toml,
                        ConfigRead::Failed(error) => {
                            summary.push_skip(
                                &request.display,
                                &format!(
                                    "Codex config.toml could not be read ({}): {error}",
                                    request.config_path.display()
                                ),
                            );
                            continue;
                        }
                    };
                    let keys = match scratch::find_trust_keys(&config_toml, &request.project_dir) {
                        Ok(keys) => keys,
                        Err(error) => {
                            summary.push_failure(&request.display, error);
                            continue;
                        }
                    };
                    if !keys.is_empty() {
                        trust_keys_by_config
                            .entry(request.config_path.clone())
                            .or_default()
                            .extend(keys);
                    }
                }
            }
        }
    }

    for (config_path, mut keys) in trust_keys_by_config {
        keys.sort();
        keys.dedup();
        match scratch::remove_trust_keys(&config_path, &keys) {
            // Only the keys the file actually held: a key removed by another
            // writer in the meantime (Codex running) is not a removal to report.
            Ok(removed) => summary.removed_trust_keys.extend(removed),
            Err(error) => summary.push_failure(&config_path.to_string_lossy(), error),
        }
    }
}

/// Scan the selected sources for scratch residue.
///
/// Residue is anything of the scratch shape — a workspace directory or a
/// `[projects]` trust key — that no rollout still records as its `cwd`. Archived
/// rollouts count as references, so a chat archived in Codex keeps its workspace
/// even though our session list does not show it.
pub(super) fn scan_residue(
    contexts: &SessionContextSet,
    source_mode: SessionSourceMode,
) -> CodexScratchResidue {
    let sources = collect_residue_sources(contexts, source_mode);

    let mut report = CodexScratchResidue {
        source: source_mode.as_str().to_string(),
        unavailable: sources.is_empty(),
        scan_complete: sources.iter().all(|source| source.references.is_some()),
        config_paths: sources
            .iter()
            .map(|source| source.config_path.to_string_lossy().to_string())
            .collect(),
        workspaces: Vec::new(),
        trust_entries: Vec::new(),
        empty_date_dirs: Vec::new(),
    };

    for source in &sources {
        for anchor in &source.anchors {
            for workspace in scratch::list_scratch_workspaces(anchor) {
                // The host spelling still compares equal to the recorded cwd: a
                // WSL UNC path folds back to the Linux spelling Codex wrote.
                let host_path = host_path_of(&source.entry, &workspace);
                if source.is_referenced(&host_path) {
                    continue;
                }
                report.workspaces.push(CodexScratchResidueWorkspace {
                    info: scratch::inspect_workspace(&host_path),
                    runtime_source: source.source_label(),
                    runtime_distro: source.entry.distro.clone(),
                });
            }

            for date_dir in scratch::list_empty_date_dirs(anchor) {
                report
                    .empty_date_dirs
                    .push(host_path_of(&source.entry, &date_dir));
            }
        }

        let Some(config_toml) = source.config_toml.as_deref() else {
            continue;
        };
        let trust_entries = match scratch::list_scratch_trust_entries(config_toml) {
            Ok(entries) => entries,
            Err(_) => {
                // An unreadable trust map means nothing here can be proven to be
                // leftovers, so the cleanup must not be offered at all.
                report.scan_complete = false;
                continue;
            }
        };

        for entry in trust_entries {
            if source.is_referenced(&entry.key) {
                continue;
            }
            // Only presence is asked here: walking the tree to answer it would
            // stat every file of every trusted project on the machine, for a
            // line of text.
            let dir_exists =
                std::fs::symlink_metadata(Path::new(&host_path_of(&source.entry, &entry.key)))
                    .is_ok();
            report.trust_entries.push(CodexScratchResidueTrustEntry {
                key: entry.key,
                config_path: source.config_path.to_string_lossy().to_string(),
                dir_exists,
                runtime_source: source.source_label(),
                runtime_distro: source.entry.distro.clone(),
            });
        }
    }

    report
}

/// Remove the selected residue, re-validating every item first.
pub(super) fn clean_residue(
    contexts: &SessionContextSet,
    source_mode: SessionSourceMode,
    workspace_paths: &[String],
    trust_keys: &[String],
    date_dirs: &[String],
) -> CodexCleanupSummary {
    let sources = collect_residue_sources(contexts, source_mode);
    let mut summary = CodexCleanupSummary::default();

    // The scans just performed answer the eligibility question below, so the
    // cleanup does not walk the same rollout trees a second time.
    let reference_sets: HashMap<PathBuf, Option<ReferenceKeys>> = sources
        .iter()
        .filter_map(|source| {
            sessions_root_of(&source.entry)
                .map(|sessions_root| (sessions_root, source.references.clone()))
        })
        .collect();

    if sources.is_empty() {
        for path in workspace_paths.iter().chain(trust_keys).chain(date_dirs) {
            summary.push_skip(path, "the selected Codex source is not available");
        }
        return summary;
    }

    let mut requests: Vec<CodexCleanupRequest> = Vec::new();

    for path in workspace_paths {
        let Some(source) = sources.iter().find(|source| source.owns_path(path)) else {
            summary.push_skip(path, "not a Codex scratch path of the selected source");
            continue;
        };
        let Some(sessions_root) = sessions_root_of(&source.entry) else {
            summary.push_skip(path, "the selected Codex source is not available");
            continue;
        };

        if !is_scratch_workspace(path) {
            summary.push_skip(path, "not a Codex scratch workspace");
            continue;
        }
        if !source.anchors_match(path) {
            summary.push_skip(path, "not inside a Codex scratch root");
            continue;
        }

        requests.push(CodexCleanupRequest {
            display: path.clone(),
            workspace_path: path.clone(),
            // The host spelling compares against the recorded cwd directly;
            // `same_path_for_compare` folds the UNC form back to Linux.
            project_dir: path.clone(),
            config_path: source.config_path.clone(),
            sessions_root,
            scope: CodexCleanupScope::WORKSPACE_ONLY,
        });
    }

    // Each source's trust map is read once: a selection can name hundreds of
    // keys, and a config.toml holding every trusted project on the machine is
    // not something to parse once per key.
    let trust_keys_by_source: Vec<HashSet<String>> = sources
        .iter()
        .map(|source| {
            source
                .config_toml
                .as_deref()
                .and_then(|config| scratch::list_scratch_trust_entries(config).ok())
                .map(|entries| entries.into_iter().map(|entry| entry.key).collect())
                .unwrap_or_default()
        })
        .collect();

    for key in trust_keys {
        let Some(source) = sources
            .iter()
            .enumerate()
            .find(|(index, _)| trust_keys_by_source[*index].contains(key))
            .map(|(_, source)| source)
        else {
            summary.push_skip(key, "the trust entry no longer exists");
            continue;
        };
        let Some(sessions_root) = sessions_root_of(&source.entry) else {
            summary.push_skip(key, "the selected Codex source is not available");
            continue;
        };

        requests.push(CodexCleanupRequest {
            display: key.clone(),
            // The key is a spelling Codex wrote, which for a WSL source is the
            // distro's own Linux path; every filesystem call needs the host
            // spelling of it.
            workspace_path: host_path_of(&source.entry, key),
            project_dir: key.clone(),
            config_path: source.config_path.clone(),
            sessions_root,
            scope: CodexCleanupScope::TRUST_ENTRY_ONLY,
        });
    }

    // Workspaces first: removing one may empty a date directory the caller also
    // selected, and the empty-directory removal then converges on a missing path.
    run_cleanup_with_references(
        &requests,
        CodexCleanupOptions {
            remove_workspace: !workspace_paths.is_empty(),
            remove_trust_entry: !trust_keys.is_empty(),
        },
        &mut summary,
        &reference_sets,
    );

    for date_dir in date_dirs {
        let Some(anchor) = scratch::codex_scratch_date_anchor(date_dir) else {
            summary.push_skip(date_dir, "not a Codex scratch date directory");
            continue;
        };
        if !sources
            .iter()
            .any(|source| source.anchors_match_anchor(&anchor))
        {
            summary.push_skip(date_dir, "not inside a Codex scratch root");
            continue;
        }
        match scratch::remove_empty_date_dir(date_dir) {
            Ok(()) => summary.removed_date_dirs.push(date_dir.clone()),
            Err(error) => summary.push_failure(date_dir, error),
        }
    }

    summary
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

impl ResidueSource {
    /// Whether a host path sits under one of this source's scratch anchors.
    fn owns_path(&self, path: &str) -> bool {
        self.anchors_match(path)
    }

    fn anchors_match(&self, path: &str) -> bool {
        let Some(anchor) = scratch::codex_scratch_anchor(path) else {
            return false;
        };
        self.anchors_match_anchor(&anchor)
    }

    fn anchors_match_anchor(&self, anchor: &str) -> bool {
        self.anchors
            .iter()
            .any(|candidate| scratch::same_path_for_compare(candidate, anchor))
    }
}

/// What reading a `config.toml` produced.
///
/// The three outcomes stay apart because they mean different things to the user:
/// a missing file has no trust entry to remove and is silent, while a file that
/// is there but unreadable is a real failure worth reporting.
#[derive(Clone)]
enum ConfigRead {
    Missing,
    Read(String),
    Failed(String),
}

/// Read a `config.toml`, remembering the result for the rest of one run.
fn read_config(path: &Path, cache: &mut HashMap<PathBuf, ConfigRead>) -> ConfigRead {
    if let Some(cached) = cache.get(path) {
        return cached.clone();
    }

    let outcome = match std::fs::read_to_string(path) {
        Ok(content) => ConfigRead::Read(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => ConfigRead::Missing,
        Err(error) => ConfigRead::Failed(error.to_string()),
    };
    cache.insert(path.to_path_buf(), outcome.clone());
    outcome
}

/// The Codex sources a residue scan may look at, with their reference sets.
fn collect_residue_sources(
    contexts: &SessionContextSet,
    source_mode: SessionSourceMode,
) -> Vec<ResidueSource> {
    let mut sources = Vec::new();

    for entry in &contexts.entries {
        if !source_mode.accepts(entry.source) {
            continue;
        }
        let Some((_, config_path)) = codex_context_paths(entry) else {
            continue;
        };
        let Some(sessions_root) = sessions_root_of(entry) else {
            continue;
        };

        let config_toml = std::fs::read_to_string(&config_path).ok();
        // The scan answers whether anything is still using a directory; this path
        // only needs that answer, not the directories themselves.
        let scan = codex_rollout::rollout_cwds(&sessions_root);
        let references = scan.complete.then(|| reference_keys(&scan));

        let anchors = scratch_anchors(entry, config_toml.as_deref());
        sources.push(ResidueSource {
            entry: entry.clone(),
            config_path,
            config_toml,
            references,
            anchors,
        });
    }

    sources
}

/// The `<Documents>/Codex` roots a source can hold residue in.
///
/// Trust keys are the primary discovery source, since they name the directories
/// Codex actually used; the platform documents directory covers residue whose
/// trust entry is already gone. For a WSL runtime only keys are available — the
/// distro's own documents directory is not resolvable from the host.
fn scratch_anchors(entry: &SessionContextEntry, config_toml: Option<&str>) -> Vec<String> {
    let mut anchors: Vec<String> = Vec::new();

    if let Some(config_toml) = config_toml {
        if let Ok(trust_entries) = scratch::list_scratch_trust_entries(config_toml) {
            for trust_entry in trust_entries {
                // The key keeps its own separators, so a Windows key yields a
                // Windows anchor and the listed paths stay native.
                if let Some(anchor) = scratch::codex_scratch_anchor(&trust_entry.key) {
                    anchors.push(host_path_of(entry, &anchor));
                }
            }
        }
    }

    if entry.source == SessionRuntimeSource::Local {
        if let Some(documents) = dirs::document_dir() {
            anchors.push(documents.join("Codex").to_string_lossy().to_string());
        }
    }

    let mut seen = HashSet::new();
    anchors.retain(|anchor| seen.insert(scratch::path_compare_key(anchor)));
    anchors
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    use serde_json::json;

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "ai-toolbox-codex-scratch-{label}-{}",
                uuid::Uuid::new_v4().simple()
            ));
            fs::create_dir_all(&path).expect("failed to create test directory");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_text_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent directory");
        }
        fs::write(path, content).expect("failed to write file");
    }

    /// A `[projects]` trust entry, quoted the way Codex writes one.
    fn trust_entry(path: &Path) -> String {
        format!(
            "[projects.'{}']\ntrust_level = \"trusted\"\n\n",
            path.to_string_lossy()
        )
    }

    /// End-to-end smoke over a **real WSL Codex home**, reached through the UNC
    /// path the app itself uses.
    ///
    /// Point `CODEX_SMOKE_WSL_HOME` at a distro-side Codex home prepared as the
    /// fixture below and run `cargo test --lib -- --ignored codex_scratch_wsl_smoke`
    /// (the caller fills the home; this test consumes it):
    ///
    /// ```text
    /// <home>/config.toml                      [projects] for orphan, kept, archived, project
    /// <home>/sessions/2026/09/25/rollout-..-<a>.jsonl     cwd = <docs>/Codex/2026-09-25/kept
    /// <home>/archived_sessions/2026/09/24/rollout-..-<b>.jsonl  cwd = <docs>/Codex/2026-09-24/archived
    /// <docs>/Codex/2026-09-25/orphan/<file>   unreferenced, must be cleaned
    /// <docs>/Codex/2026-09-25/kept/           referenced, must survive
    /// <docs>/Codex/2026-09-24/archived/       referenced from the archive, must survive
    /// <docs>/Codex/2026-09-19/                empty dated directory, must be pruned
    /// <docs>/Codex/../project/main.rs         real project, never a target
    /// ```
    ///
    /// The unit tests cover this logic with Windows or synthetic paths; only a
    /// real share shows what 9p and the UNC spelling do to it — a nested distro
    /// prefix, a trust key that will not compare, an archived sibling that does
    /// not resolve.
    #[test]
    #[ignore]
    fn codex_scratch_wsl_smoke() {
        let Ok(home) = std::env::var("CODEX_SMOKE_WSL_HOME") else {
            return;
        };
        let home = PathBuf::from(home);
        let config_path = home.join("config.toml");
        let config = fs::read_to_string(&config_path).expect("the smoke home needs a config.toml");
        let documents = smoke_documents_root(&config);
        let documents = documents.to_string_lossy().to_string();

        let contexts = codex_context_from_unc(&home);
        let report = scan_residue(&contexts, SessionSourceMode::All);
        assert!(
            report.scan_complete,
            "a reachable WSL home must scan completely: {report:?}"
        );

        let orphan = format!("{documents}/Codex/2026-09-25/orphan");
        let kept = format!("{documents}/Codex/2026-09-25/kept");
        let archived = format!("{documents}/Codex/2026-09-24/archived");
        let empty_date = format!("{documents}/Codex/2026-09-19");

        let listed: Vec<&str> = report
            .workspaces
            .iter()
            .map(|workspace| workspace.info.path.as_str())
            .collect();
        assert!(
            listed
                .iter()
                .any(|path| scratch::same_path_for_compare(path, &orphan)),
            "the unreferenced workspace should be listed: {listed:?}"
        );
        for referenced in [&kept, &archived] {
            assert!(
                !listed
                    .iter()
                    .any(|path| scratch::same_path_for_compare(path, referenced)),
                "a referenced workspace must not be offered: {referenced} in {listed:?}"
            );
        }
        assert!(
            !listed.iter().any(|path| path.contains("project")),
            "a real project must never be listed: {listed:?}"
        );
        assert!(
            report
                .empty_date_dirs
                .iter()
                .any(|path| scratch::same_path_for_compare(path, &empty_date)),
            "the empty dated directory should be listed: {:?}",
            report.empty_date_dirs
        );

        // Clean exactly the residue, and only it.
        let summary = clean_residue(
            &contexts,
            SessionSourceMode::All,
            &report
                .workspaces
                .iter()
                .map(|workspace| workspace.info.path.clone())
                .collect::<Vec<_>>(),
            &report
                .trust_entries
                .iter()
                .map(|entry| entry.key.clone())
                .collect::<Vec<_>>(),
            &report.empty_date_dirs.clone(),
        );
        assert!(
            summary.failures.is_empty(),
            "unexpected failures: {:?}",
            summary.failures
        );

        // The distro side had better agree: the removal went through 9p.
        assert!(
            !Path::new(&host_path_of(&contexts.entries[0], &orphan)).exists(),
            "the orphan workspace should be gone"
        );
        assert!(
            !Path::new(&host_path_of(&contexts.entries[0], &empty_date)).exists(),
            "the emptied dated directory should be pruned"
        );
        for referenced in [&kept, &archived, &format!("{documents}/project")] {
            assert!(
                Path::new(&host_path_of(&contexts.entries[0], referenced)).exists(),
                "the referenced or real directory must survive: {referenced}"
            );
        }

        let written = fs::read_to_string(&config_path).expect("config should still be readable");
        assert!(
            !written.contains(&orphan),
            "the orphan trust entry should be gone: {written}"
        );
        for referenced in [&kept, &archived, &format!("{documents}/project")] {
            assert!(
                written.contains(referenced.as_str()),
                "the remaining trust entry must survive: {referenced} in {written}"
            );
        }

        let after = scan_residue(&contexts, SessionSourceMode::All);
        assert!(
            after.workspaces.is_empty()
                && after.trust_entries.is_empty()
                && after.empty_date_dirs.is_empty(),
            "the smoke home should be fully cleaned: {after:?}"
        );

        // Phase 2: the delete-time cleanup over the same share — the flow a user
        // actually takes. The session whose cwd is `kept` goes, taking its
        // workspace, its trust entry and its rollout with it.
        let rollout = home
            .join("sessions")
            .join("2026")
            .join("09")
            .join("25")
            .join("rollout-2026-09-25T10-00-00-01a08e7d-5f4b-7c31-9a20-6d3f11b91882.jsonl");
        let result = crate::coding::session_manager::delete_session_blocking(
            codex_context_from_unc(&home),
            rollout.to_string_lossy().to_string(),
            CodexCleanupOptions {
                remove_workspace: true,
                remove_trust_entry: true,
            },
        )
        .expect("the delete should succeed");

        assert!(!rollout.exists(), "the rollout should be gone");
        assert!(
            !Path::new(&host_path_of(&contexts.entries[0], &kept)).exists(),
            "the deleted chat's workspace should be gone"
        );
        let cleanup = result.cleanup.expect("cleanup should be reported");
        assert_eq!(cleanup.removed_workspaces.len(), 1, "{cleanup:?}");
        assert_eq!(cleanup.removed_trust_keys.len(), 1, "{cleanup:?}");
        assert!(
            cleanup.failures.is_empty(),
            "unexpected failures: {:?}",
            cleanup.failures
        );

        // What another session uses is untouched, on both sides of the share.
        let project = format!("{documents}/project");
        for survivor in [&archived, &project] {
            assert!(
                Path::new(&host_path_of(&contexts.entries[0], survivor)).exists(),
                "a directory another session uses must survive: {survivor}"
            );
        }
        let written = fs::read_to_string(&config_path).expect("config should still be readable");
        assert!(
            !written.contains(&kept),
            "the deleted chat's trust entry should be gone: {written}"
        );
        for survivor in [&archived, &project] {
            assert!(
                written.contains(survivor.as_str()),
                "the remaining trust entry must survive: {survivor} in {written}"
            );
        }
    }

    /// Smoke over the machine's **real local Codex home**, in two opt-in halves.
    ///
    /// `CODEX_SMOKE_LOCAL_HOME` names an existing home (e.g.
    /// `%USERPROFILE%\.codex`) and is only ever **read**: the scan must complete
    /// (or the cleanup would not be offered at all), no real trust entry may be
    /// mistaken for residue (the "never touch a real project" rule, on a config
    /// that holds every project the user ever trusted), and the real rollout
    /// `cwd`s must compare against the real trust keys Codex wrote — it
    /// canonicalizes and lowercases those, while the `cwd` keeps the user's case.
    ///
    /// `CODEX_SMOKE_LOCAL_FIXTURE` names a home the caller prepared like the WSL
    /// fixture (a scratch workspace with files, a referenced sibling, a real
    /// project, real-style trust entries — including a lowercased one — and a
    /// rollout per session) and *is emptied by this test*. Never point it at a
    /// real home; point it at a copy.
    ///
    /// Run with `cargo test --lib -- --ignored codex_scratch_local_smoke`.
    #[test]
    #[ignore]
    fn codex_scratch_local_smoke() {
        if let Ok(fixture) = std::env::var("CODEX_SMOKE_LOCAL_FIXTURE") {
            local_clean_smoke(&PathBuf::from(fixture));
        }

        let Ok(home) = std::env::var("CODEX_SMOKE_LOCAL_HOME") else {
            return;
        };
        let home = PathBuf::from(home);
        assert!(home.is_dir(), "the smoke home should exist: {home:?}");

        let contexts = codex_context(&home);
        let report = scan_residue(&contexts, SessionSourceMode::Local);

        let sources = collect_residue_sources(&contexts, SessionSourceMode::Local);
        for source in &sources {
            println!("anchors: {:?}", source.anchors);
            println!(
                "references: {:?}",
                source.references.as_ref().map(|keys| keys.len())
            );
            if let Some(config) = source.config_toml.as_deref() {
                let entries = scratch::list_scratch_trust_entries(config).unwrap_or_default();
                println!("scratch trust entries: {}", entries.len());
                for entry in &entries {
                    println!("  raw trust key: {}", entry.key);
                }
            }
            for anchor in &source.anchors {
                let listed = scratch::list_scratch_workspaces(anchor);
                println!("anchor {anchor} -> {} workspace(s)", listed.len());
                for path in &listed {
                    println!(
                        "  {path} referenced={:?}",
                        source
                            .references
                            .as_ref()
                            .map(|_| source.is_referenced(&host_path_of(&source.entry, path)))
                    );
                }
            }
        }

        println!("source: {:?}", report.source);
        println!("config: {:?}", report.config_paths);
        println!(
            "scan_complete={} unavailable={}",
            report.scan_complete, report.unavailable
        );
        for workspace in &report.workspaces {
            println!(
                "workspace: {} empty={} files={} git={}",
                workspace.info.path,
                workspace.info.is_empty,
                workspace.info.file_count,
                workspace.info.has_git
            );
        }
        for entry in &report.trust_entries {
            println!("trust: {} dir_exists={}", entry.key, entry.dir_exists);
        }
        for path in &report.empty_date_dirs {
            println!("empty date dir: {path}");
        }

        assert!(
            report.scan_complete,
            "a reachable local home must scan completely: {report:?}"
        );

        // Nothing outside the scratch shape may be offered, whatever the config
        // holds: this home's trust map names real projects.
        for workspace in &report.workspaces {
            assert!(
                is_scratch_workspace(&workspace.info.path),
                "a non-scratch directory was offered: {}",
                workspace.info.path
            );
        }
        for entry in &report.trust_entries {
            assert!(
                is_scratch_workspace(&entry.key.replace('\\', "/")),
                "a non-scratch trust entry was offered: {}",
                entry.key
            );
        }

        // The platform documents directory is one of the discovered roots, so
        // residue whose trust entry is already gone is still reachable.
        if let Some(documents) = dirs::document_dir() {
            let anchor = documents.join("Codex").to_string_lossy().to_string();
            let sources = collect_residue_sources(&contexts, SessionSourceMode::Local);
            assert!(
                sources
                    .iter()
                    .any(|source| source.anchors_match_anchor(&anchor)),
                "the platform documents root should be discovered: {anchor}"
            );
        }
    }

    /// The write half: delete one chat of the fixture and clean up after it, on real
    /// Windows I/O.
    ///
    /// Fixture layout, built by the caller from whatever real residue they want to
    /// rehearse on:
    ///
    /// ```text
    /// <fixture>/home/config.toml            [projects] for orphan, with-chat, kept, project
    /// <fixture>/home/sessions/2026/09/25/rollout-..-<a>.jsonl         cwd = with-chat
    /// <fixture>/home/archived_sessions/2026/09/24/rollout-..-<b>.jsonl  cwd = kept
    /// <fixture>/Documents/Codex/2026-09-25/orphan/<files>   no rollout at all: residue
    /// <fixture>/Documents/Codex/2026-09-23/with-chat/<files>  the chat deleted below
    /// <fixture>/Documents/Codex/2026-09-24/kept/<files>     referenced from the archive
    /// <fixture>/Documents/project/main.rs   never a target
    /// ```
    ///
    /// Only this fixture's own paths are asserted on or cleaned: a *local* source
    /// always also reads the platform documents directory, so a developer's own
    /// residue shows up in the same report (correctly — it is residue of their own
    /// home) and must not be acted on here.
    fn local_clean_smoke(fixture: &Path) {
        let home = fixture.join("home");
        let config_path = home.join("config.toml");
        let config = fs::read_to_string(&config_path).expect("the fixture needs a config.toml");
        let documents = smoke_documents_root(&config);
        let documents = documents.to_string_lossy().to_string();
        // The fixture's keys are lowercased the way Codex writes them, so the
        // prefix test folds case: `Path::starts_with` does not.
        let own = |path: &str| {
            path.to_ascii_lowercase()
                .starts_with(&documents.to_ascii_lowercase())
        };

        let contexts = codex_context(&home);
        let report = scan_residue(&contexts, SessionSourceMode::Local);
        assert!(report.scan_complete, "the fixture should scan: {report:?}");

        let orphan = format!("{documents}/Codex/2026-09-25/orphan");
        let kept = format!("{documents}/Codex/2026-09-24/kept");
        let project = format!("{documents}/project");

        let listed: Vec<&str> = report
            .workspaces
            .iter()
            .map(|workspace| workspace.info.path.as_str())
            .filter(|path| own(path))
            .collect();
        println!("listed workspaces: {listed:?}");
        assert!(
            listed
                .iter()
                .any(|path| scratch::same_path_for_compare(path, &orphan)),
            "the unreferenced workspace should be listed: {listed:?}"
        );
        assert!(
            !listed
                .iter()
                .any(|path| scratch::same_path_for_compare(path, &kept)),
            "a workspace an archived session still uses must not be offered: {listed:?}"
        );
        assert!(
            !listed.iter().any(|path| path.contains("project")),
            "a real project must never be listed: {listed:?}"
        );

        // The delete path, exactly as the app drives it.
        let rollout = home
            .join("sessions")
            .join("2026")
            .join("09")
            .join("25")
            .join("rollout-2026-09-25T10-00-00-01a08e7d-5f4b-7c31-9a20-6d3f11b91882.jsonl");
        let result = crate::coding::session_manager::delete_session_blocking(
            codex_context(&home),
            rollout.to_string_lossy().to_string(),
            CodexCleanupOptions {
                remove_workspace: true,
                remove_trust_entry: true,
            },
        )
        .expect("the delete should succeed");

        let with_chat = format!("{documents}/Codex/2026-09-23/with-chat");
        assert!(!rollout.exists(), "the rollout should be gone");
        assert!(
            !Path::new(&with_chat).exists(),
            "the deleted chat's workspace should be gone"
        );
        assert!(
            Path::new(&kept).exists(),
            "the workspace an archived session uses must survive"
        );
        assert!(Path::new(&project).exists(), "the project must survive");

        let cleanup = result.cleanup.expect("cleanup should be reported");
        println!("cleanup: {cleanup:?}");
        assert_eq!(cleanup.removed_workspaces.len(), 1, "{cleanup:?}");
        assert_eq!(cleanup.removed_trust_keys.len(), 1, "{cleanup:?}");
        assert!(
            cleanup.failures.is_empty(),
            "unexpected failures: {:?}",
            cleanup.failures
        );

        // Match the way the module does: the keys Codex wrote are canonicalized and
        // lowercased, so a plain substring test would prove nothing either way.
        let written = fs::read_to_string(&config_path).expect("config should stay readable");
        let has_entry = |path: &str| {
            scratch::find_trust_keys(&written, path).is_ok_and(|keys| !keys.is_empty())
        };
        assert!(
            !has_entry(&with_chat),
            "the entry should be gone: {written}"
        );
        for survivor in [&kept, &project, &orphan] {
            assert!(
                has_entry(survivor),
                "the remaining entry must survive: {survivor} in {written}"
            );
        }
    }

    /// The `<docs>` root of the smoke fixture, read from its own trust keys.
    fn smoke_documents_root(config: &str) -> PathBuf {
        let key = scratch::list_scratch_trust_entries(config)
            .expect("the smoke config should list")
            .into_iter()
            .next()
            .expect("the smoke config should hold at least one scratch trust entry")
            .key;
        let anchor = scratch::codex_scratch_anchor(&key).expect("the key should be scratch shaped");
        PathBuf::from(anchor)
            .parent()
            .expect("the anchor should have a parent")
            .to_path_buf()
    }

    /// A Codex context for a distro-side home named by its UNC path.
    fn codex_context_from_unc(home: &Path) -> SessionContextSet {
        crate::coding::session_manager::single_context_set(ToolSessionContext::Codex {
            sessions_root: home.join("sessions"),
            codex_home: Some(home.to_path_buf()),
        })
    }

    fn rollout_record(thread_id: &str, cwd: &str) -> String {
        json!({
            "timestamp": "2026-09-25T10:00:00Z",
            "type": "session_meta",
            "payload": {
                "id": thread_id,
                "timestamp": "2026-09-25T10:00:00Z",
                "cwd": cwd,
            }
        })
        .to_string()
    }

    fn write_compressed(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent directory");
        }
        let compressed =
            zstd::stream::encode_all(content.as_bytes(), 3).expect("failed to compress rollout");
        fs::write(path, compressed).expect("failed to write compressed rollout");
    }

    fn scratch_workspace(root: &Path, date: &str, slug: &str) -> PathBuf {
        let path = root.join("Documents").join("Codex").join(date).join(slug);
        fs::create_dir_all(&path).expect("failed to create scratch workspace");
        path
    }

    fn codex_context(codex_home: &Path) -> crate::coding::session_manager::SessionContextSet {
        crate::coding::session_manager::single_context_set(ToolSessionContext::Codex {
            sessions_root: codex_home.join("sessions"),
            codex_home: Some(codex_home.to_path_buf()),
        })
    }

    /// The report items that live under this test's own tree.
    ///
    /// The real prefix is discovered from the platform documents directory as
    /// well, so a developer machine with actual Codex residue reports it too —
    /// correctly, but it must not make these assertions machine-dependent.
    fn own_workspaces<'a>(
        report: &'a CodexScratchResidue,
        root: &Path,
    ) -> Vec<&'a CodexScratchResidueWorkspace> {
        let prefix = root.to_string_lossy().to_string();
        report
            .workspaces
            .iter()
            .filter(|workspace| workspace.info.path.starts_with(&prefix))
            .collect()
    }

    fn own_trust_entries<'a>(
        report: &'a CodexScratchResidue,
        root: &Path,
    ) -> Vec<&'a CodexScratchResidueTrustEntry> {
        let prefix = root.to_string_lossy().to_string();
        report
            .trust_entries
            .iter()
            .filter(|entry| entry.key.starts_with(&prefix))
            .collect()
    }

    /// Residue is a scratch shape nothing references any more: a workspace whose
    /// chat is gone, and a trust key whose chat is gone (with or without its
    /// directory). A real project's trust key is never residue.
    #[test]
    fn residue_lists_unreferenced_workspaces_and_trust_entries() {
        let test_dir = TestDir::new("residue-list");
        let codex_home = test_dir.path().join("codex-home");
        let workspace = scratch_workspace(test_dir.path(), "2026-09-25", "xi");
        fs::write(workspace.join("note.md"), b"scratch").expect("failed to write workspace file");
        let gone_workspace = test_dir
            .path()
            .join("Documents")
            .join("Codex")
            .join("2026-09-20")
            .join("gone");
        let project_dir = test_dir.path().join("real-project");
        fs::create_dir_all(&project_dir).expect("failed to create project dir");

        write_text_file(
            &codex_home.join("config.toml"),
            &format!(
                "{}{}{}",
                trust_entry(&workspace),
                trust_entry(&gone_workspace),
                trust_entry(&project_dir)
            ),
        );
        // A live session in an unrelated project: the reference set is non-empty,
        // but it does not cover the scratch tree.
        write_text_file(
            &codex_home
                .join("sessions")
                .join("2026")
                .join("09")
                .join("25")
                .join("rollout-2026-09-25T09-00-00-01a08e7d-5f4b-7c31-9a20-6d3f11b91882.jsonl"),
            &rollout_record(
                "01a08e7d-5f4b-7c31-9a20-6d3f11b91882",
                &project_dir.to_string_lossy(),
            ),
        );

        let report = scan_residue(&codex_context(&codex_home), SessionSourceMode::All);

        assert!(!report.unavailable);
        assert!(report.scan_complete);

        let workspaces = own_workspaces(&report, test_dir.path());
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].info.path, workspace.to_string_lossy());
        assert!(!workspaces[0].info.is_empty);

        let keys: Vec<String> = own_trust_entries(&report, test_dir.path())
            .iter()
            .map(|entry| entry.key.clone())
            .collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&workspace.to_string_lossy().to_string()));
        assert!(keys.contains(&gone_workspace.to_string_lossy().to_string()));
        assert!(
            !keys.contains(&project_dir.to_string_lossy().to_string()),
            "a real project's trust entry is not residue"
        );

        let missing = report
            .trust_entries
            .iter()
            .find(|entry| entry.key == gone_workspace.to_string_lossy())
            .expect("the orphaned key should be listed");
        assert!(!missing.dir_exists);
    }

    /// A chat archived in Codex still owns its workspace: the residue scan must
    /// count archived rollouts — including compressed ones — as references.
    #[test]
    fn residue_keeps_what_an_archived_session_still_uses() {
        let test_dir = TestDir::new("residue-archived");
        let codex_home = test_dir.path().join("codex-home");
        let workspace = scratch_workspace(test_dir.path(), "2026-09-25", "kept");

        write_text_file(&codex_home.join("config.toml"), &trust_entry(&workspace));
        write_compressed(
            &codex_home
                .join("archived_sessions")
                .join("rollout-2026-09-24T09-00-00-01a08e11-2c7d-7b55-8e10-4a9c77d20453.jsonl.zst"),
            &format!(
                "{}\n",
                rollout_record(
                    "01a08e11-2c7d-7b55-8e10-4a9c77d20453",
                    &workspace.to_string_lossy(),
                )
            ),
        );

        let report = scan_residue(&codex_context(&codex_home), SessionSourceMode::All);

        assert!(report.scan_complete);
        assert!(
            own_workspaces(&report, test_dir.path()).is_empty(),
            "an archived session keeps its workspace: {:?}",
            report.workspaces
        );
        assert!(
            own_trust_entries(&report, test_dir.path()).is_empty(),
            "its trust entry is not residue either"
        );
    }

    /// Cleaning re-validates every item: the selection is removed, a workspace
    /// that is still referenced is skipped, and a second scan converges.
    #[test]
    fn clean_residue_removes_the_selection_and_revalidates() {
        let test_dir = TestDir::new("residue-clean");
        let codex_home = test_dir.path().join("codex-home");
        let workspace = scratch_workspace(test_dir.path(), "2026-09-25", "xi");
        // A date directory with nothing in it at all.
        let empty_date = test_dir
            .path()
            .join("Documents")
            .join("Codex")
            .join("2026-09-19");
        fs::create_dir_all(&empty_date).expect("failed to create empty date dir");
        // A workspace another live session still uses.
        let busy_workspace = scratch_workspace(test_dir.path(), "2026-09-18", "busy");

        write_text_file(
            &codex_home.join("config.toml"),
            &format!(
                "{}{}",
                trust_entry(&workspace),
                trust_entry(&busy_workspace)
            ),
        );
        write_text_file(
            &codex_home
                .join("sessions")
                .join("2026")
                .join("09")
                .join("18")
                .join("rollout-2026-09-18T09-00-00-01a08e11-2c7d-7b55-8e10-4a9c77d20453.jsonl"),
            &rollout_record(
                "01a08e11-2c7d-7b55-8e10-4a9c77d20453",
                &busy_workspace.to_string_lossy(),
            ),
        );

        let contexts = codex_context(&codex_home);
        let summary = clean_residue(
            &contexts,
            SessionSourceMode::All,
            &[
                workspace.to_string_lossy().to_string(),
                busy_workspace.to_string_lossy().to_string(),
            ],
            &[workspace.to_string_lossy().to_string()],
            &[empty_date.to_string_lossy().to_string()],
        );

        assert_eq!(summary.removed_workspaces.len(), 1);
        assert!(!workspace.exists());
        assert!(busy_workspace.exists(), "a referenced workspace stays");
        assert!(
            summary
                .skipped
                .iter()
                .any(|skip| skip.reason.contains("another Codex session")),
            "unexpected skips: {:?}",
            summary.skipped
        );
        assert_eq!(summary.removed_trust_keys.len(), 1);
        assert_eq!(summary.removed_date_dirs.len(), 1);
        assert!(!empty_date.exists());
        assert!(
            summary.failures.is_empty(),
            "unexpected failures: {:?}",
            summary.failures
        );

        // The remaining trust entry names the busy workspace, so it is not
        // residue; nothing else is left under this tree.
        let report = scan_residue(&contexts, SessionSourceMode::All);
        assert!(own_workspaces(&report, test_dir.path()).is_empty());
        assert!(own_trust_entries(&report, test_dir.path()).is_empty());
        assert!(!report
            .empty_date_dirs
            .iter()
            .any(|path| path.starts_with(&test_dir.path().to_string_lossy().to_string())));
    }

    /// A residue row authorizes exactly what it names: a checked trust entry must
    /// never take an unchecked directory with it, and a checked directory must
    /// never take an unchecked trust entry.
    #[test]
    fn clean_residue_removes_only_what_each_row_names() {
        let test_dir = TestDir::new("residue-row-scope");
        let codex_home = test_dir.path().join("codex-home");
        let empty_workspace = scratch_workspace(test_dir.path(), "2026-09-25", "empty");
        let kept_workspace = scratch_workspace(test_dir.path(), "2026-09-24", "kept");
        fs::write(kept_workspace.join("note.md"), b"the user's files")
            .expect("failed to write a workspace file");

        let config_path = codex_home.join("config.toml");
        write_text_file(
            &config_path,
            &format!(
                "{}{}",
                trust_entry(&empty_workspace),
                trust_entry(&kept_workspace)
            ),
        );

        let summary = clean_residue(
            &codex_context(&codex_home),
            SessionSourceMode::All,
            &[empty_workspace.to_string_lossy().to_string()],
            &[kept_workspace.to_string_lossy().to_string()],
            &[],
        );

        assert!(!empty_workspace.exists(), "the selected workspace goes");
        assert!(
            kept_workspace.join("note.md").exists(),
            "a directory the user left unchecked must survive a checked trust entry"
        );
        assert_eq!(
            summary.removed_workspaces,
            vec![empty_workspace.to_string_lossy().to_string()]
        );
        assert_eq!(
            summary.removed_trust_keys,
            vec![kept_workspace.to_string_lossy().to_string()]
        );
        assert!(
            summary.failures.is_empty(),
            "unexpected failures: {:?}",
            summary.failures
        );

        let written = fs::read_to_string(&config_path).expect("config should be readable");
        assert!(
            written.contains(&empty_workspace.to_string_lossy().to_string()),
            "the unchecked trust entry stays: {written}"
        );
        assert!(
            !written.contains(&kept_workspace.to_string_lossy().to_string()),
            "the checked trust entry goes: {written}"
        );
    }

    /// The headline guarantee: a session that belongs to a real project offers no
    /// cleanup at all, and asking for one anyway leaves the project directory and
    /// its trust entry untouched. This is the layer the delete dialog reads, so it
    /// is what keeps a user's project out of the option list.
    #[test]
    fn preview_offers_nothing_for_a_real_project() {
        let test_dir = TestDir::new("preview-real-project");
        let codex_home = test_dir.path().join("codex-home");
        let project_dir = test_dir.path().join("real-project");
        fs::create_dir_all(&project_dir).expect("failed to create project dir");
        fs::write(project_dir.join("main.rs"), b"fn main() {}").expect("failed to write a file");
        let project_dir_display = project_dir.to_string_lossy().to_string();

        let config_path = codex_home.join("config.toml");
        write_text_file(&config_path, &trust_entry(&project_dir));

        let rollout = codex_home
            .join("sessions")
            .join("2026")
            .join("09")
            .join("25")
            .join("rollout-2026-09-25T10-00-00-01a08e7d-5f4b-7c31-9a20-6d3f11b91882.jsonl");
        write_text_file(
            &rollout,
            &rollout_record("01a08e7d-5f4b-7c31-9a20-6d3f11b91882", &project_dir_display),
        );
        let source_path = rollout.to_string_lossy().to_string();

        let contexts = codex_context(&codex_home);
        let preview = preview(&contexts, std::slice::from_ref(&source_path));
        assert!(
            preview.items.is_empty(),
            "a real project has no scratch residue to offer"
        );

        // It is not even asked about: the cleanup targets of a delete are read
        // from the same rollout metadata, and a project contributes none — so
        // nothing is reported as a "leftover that was skipped" either.
        let requests = delete_requests(&contexts, std::slice::from_ref(&source_path));
        assert!(
            requests.is_empty(),
            "a real project must not become a cleanup target"
        );

        let mut summary = CodexCleanupSummary::default();
        run_cleanup(
            &requests,
            CodexCleanupOptions {
                remove_workspace: true,
                remove_trust_entry: true,
            },
            &mut summary,
        );

        assert!(
            project_dir.join("main.rs").exists(),
            "the project directory must survive"
        );
        assert!(summary.removed_workspaces.is_empty());
        assert!(summary.removed_trust_keys.is_empty());
        assert!(summary.is_empty(), "{summary:?}");

        let written = fs::read_to_string(&config_path).expect("config should be readable");
        assert!(
            written.contains(&project_dir_display),
            "the project trust entry stays: {written}"
        );
    }

    /// Codex stores the trust key canonicalized and lowercased (on Windows), while
    /// the rollout records the `cwd` in the user's own spelling. Real homes hold
    /// exactly that pair — `c:\users\...\documents\codex\...` against
    /// `C:\Users\...\Documents\Codex\...` — and the cleanup has to match across
    /// it, or deleting the chat leaves its trust entry behind forever.
    #[cfg(windows)]
    #[test]
    fn cleanup_matches_a_lowercased_trust_key() {
        let test_dir = TestDir::new("cleanup-lowercase-key");
        let codex_home = test_dir.path().join("codex-home");
        let workspace = scratch_workspace(test_dir.path(), "2026-09-25", "new-chat");
        fs::write(workspace.join("note.md"), b"produced here").expect("failed to write a file");

        // The key as Codex writes it: the canonical path, lowercased.
        let config_path = codex_home.join("config.toml");
        let canonical = fs::canonicalize(&workspace).expect("the workspace should resolve");
        let lowercased = canonical.to_string_lossy().to_ascii_lowercase();
        assert_ne!(
            lowercased,
            workspace.to_string_lossy(),
            "the fixture should differ in case, or this proves nothing"
        );
        write_text_file(
            &config_path,
            &format!("[projects.'{lowercased}']\ntrust_level = \"trusted\"\n"),
        );

        let rollout = codex_home
            .join("sessions")
            .join("2026")
            .join("09")
            .join("25")
            .join("rollout-2026-09-25T10-00-00-01a08e7d-5f4b-7c31-9a20-6d3f11b91882.jsonl");
        write_text_file(
            &rollout,
            &rollout_record(
                "01a08e7d-5f4b-7c31-9a20-6d3f11b91882",
                &workspace.to_string_lossy(),
            ),
        );

        let contexts = codex_context(&codex_home);
        let requests = delete_requests(&contexts, &[rollout.to_string_lossy().to_string()]);
        assert_eq!(requests.len(), 1, "the session should be a cleanup target");

        // The real order: the session goes first, and its own rollout stops
        // referencing the workspace before the cleanup looks at it.
        let result = crate::coding::session_manager::delete_session_blocking(
            contexts,
            rollout.to_string_lossy().to_string(),
            CodexCleanupOptions {
                remove_workspace: true,
                remove_trust_entry: true,
            },
        )
        .expect("the delete should succeed");

        assert!(!workspace.exists(), "the workspace should be gone");
        let summary = result.cleanup.expect("cleanup should be reported");
        assert_eq!(summary.removed_workspaces.len(), 1, "{summary:?}");
        assert_eq!(summary.removed_trust_keys.len(), 1, "{summary:?}");
        let written = fs::read_to_string(&config_path).expect("config should be readable");
        assert!(
            !written.contains(&lowercased),
            "the lowercased trust entry should be gone: {written}"
        );
    }

    /// The shape guard is the last line of defence: a caller that hands the
    /// cleanup a real project's path gets a refusal, never a deletion — even
    /// though the delete path filters those out before they get here.
    #[test]
    fn run_cleanup_refuses_a_path_that_is_not_a_scratch_workspace() {
        let test_dir = TestDir::new("run-cleanup-refuse");
        let codex_home = test_dir.path().join("codex-home");
        let project = test_dir.path().join("real-project");
        fs::create_dir_all(&project).expect("failed to create project dir");
        fs::write(project.join("main.rs"), b"fn main() {}").expect("failed to write a file");
        let display = project.to_string_lossy().to_string();

        let mut summary = CodexCleanupSummary::default();
        run_cleanup(
            &[CodexCleanupRequest {
                display: display.clone(),
                workspace_path: display.clone(),
                project_dir: display,
                config_path: codex_home.join("config.toml"),
                sessions_root: codex_home.join("sessions"),
                scope: CodexCleanupScope::BOTH,
            }],
            CodexCleanupOptions {
                remove_workspace: true,
                remove_trust_entry: true,
            },
            &mut summary,
        );

        assert!(
            project.join("main.rs").exists(),
            "the project must survive a cleanup aimed at it"
        );
        assert!(summary.removed_workspaces.is_empty());
        assert!(summary.removed_trust_keys.is_empty());
        assert_eq!(summary.skipped.len(), 1, "{:?}", summary.skipped);
        assert!(
            summary.skipped[0]
                .reason
                .contains("not a Codex scratch workspace"),
            "unexpected reason: {}",
            summary.skipped[0].reason
        );
    }

    /// A WSL session records a Linux cwd, and everything this module hands to the
    /// filesystem must be the host spelling of it — exactly once.
    #[test]
    fn wsl_paths_are_converted_to_unc_once() {
        let distro = "Ubuntu";
        let entry =
            crate::coding::session_manager::session_context_entry(ToolSessionContext::Codex {
                sessions_root: runtime_location::build_windows_unc_path(
                    distro,
                    "/home/me/.codex/sessions",
                ),
                codex_home: Some(runtime_location::build_windows_unc_path(
                    distro,
                    "/home/me/.codex",
                )),
            });
        assert_eq!(entry.source, SessionRuntimeSource::Wsl);
        assert_eq!(entry.distro.as_deref(), Some(distro));

        let linux_workspace = "/home/me/Documents/Codex/2026-09-25/xi";
        let unc_workspace = host_path_of(&entry, linux_workspace);
        assert_eq!(
            unc_workspace,
            runtime_location::build_windows_unc_path(distro, linux_workspace)
                .to_string_lossy()
                .to_string()
        );
        // A path the scan already derived from an anchor is UNC: converting it
        // again would nest a second distro prefix.
        assert_eq!(host_path_of(&entry, &unc_workspace), unc_workspace);

        let linux_anchor = scratch::codex_scratch_anchor(linux_workspace)
            .expect("the linux workspace should match the shape");
        let unc_anchor = host_path_of(&entry, &linux_anchor);
        assert_eq!(
            host_path_of(&entry, &unc_anchor),
            unc_anchor,
            "the anchor stays in the host spelling"
        );
    }

    /// A selected path outside the scratch roots, or a trust key that no longer
    /// exists, is refused rather than acted on.
    #[test]
    fn clean_residue_refuses_paths_it_cannot_place() {
        let test_dir = TestDir::new("residue-refuse");
        let codex_home = test_dir.path().join("codex-home");
        fs::create_dir_all(&codex_home).expect("failed to create codex home");
        let project_dir = test_dir.path().join("real-project");
        fs::create_dir_all(&project_dir).expect("failed to create project dir");

        let contexts = codex_context(&codex_home);
        let summary = clean_residue(
            &contexts,
            SessionSourceMode::All,
            &[
                project_dir.to_string_lossy().to_string(),
                "/Users/me/Documents/Codex/2026-09-25/xi".to_string(),
            ],
            &["/Users/me/Documents/Codex/2026-09-25/xi".to_string()],
            &[],
        );

        assert!(summary.removed_workspaces.is_empty());
        assert!(summary.removed_trust_keys.is_empty());
        assert_eq!(summary.skipped.len(), 3);
        assert!(project_dir.exists());
    }

    /// A cleanup request for one scratch workspace, with both actions offered.
    fn cleanup_request(codex_home: &Path, workspace: &Path) -> CodexCleanupRequest {
        let workspace = workspace.to_string_lossy().to_string();
        CodexCleanupRequest {
            display: workspace.clone(),
            workspace_path: workspace.clone(),
            project_dir: workspace,
            config_path: codex_home.join("config.toml"),
            sessions_root: codex_home.join("sessions"),
            scope: CodexCleanupScope::BOTH,
        }
    }

    /// A Codex home without a `config.toml` has no trust entry to remove, so the
    /// cleanup stays silent instead of warning about the missing file.
    #[test]
    fn a_missing_config_is_not_reported_as_a_skip() {
        let test_dir = TestDir::new("residue-missing-config");
        let codex_home = test_dir.path().join("codex-home");
        let workspace = scratch_workspace(test_dir.path(), "2026-09-25", "xi");
        assert!(!codex_home.join("config.toml").exists());

        let mut summary = CodexCleanupSummary::default();
        run_cleanup(
            &[cleanup_request(&codex_home, &workspace)],
            CodexCleanupOptions {
                remove_workspace: true,
                remove_trust_entry: true,
            },
            &mut summary,
        );

        assert!(!workspace.exists(), "the workspace is still removed");
        assert_eq!(summary.removed_workspaces.len(), 1);
        assert!(summary.removed_trust_keys.is_empty());
        assert!(
            summary.skipped.is_empty(),
            "unexpected skips: {:?}",
            summary.skipped
        );
        assert!(
            summary.failures.is_empty(),
            "unexpected failures: {:?}",
            summary.failures
        );
    }

    /// A `config.toml` that exists but cannot be read is a real failure, and it is
    /// reported rather than passed over in silence.
    #[test]
    fn an_unreadable_config_is_reported() {
        let test_dir = TestDir::new("residue-unreadable-config");
        let codex_home = test_dir.path().join("codex-home");
        let workspace = scratch_workspace(test_dir.path(), "2026-09-25", "xi");
        // A directory where the config file should be: it exists, but reading it
        // as text cannot succeed on either platform.
        fs::create_dir_all(codex_home.join("config.toml")).expect("failed to create directory");

        let mut summary = CodexCleanupSummary::default();
        run_cleanup(
            &[cleanup_request(&codex_home, &workspace)],
            CodexCleanupOptions {
                remove_workspace: true,
                remove_trust_entry: true,
            },
            &mut summary,
        );

        assert_eq!(summary.removed_workspaces.len(), 1);
        assert!(summary.removed_trust_keys.is_empty());
        assert_eq!(summary.skipped.len(), 1, "{:?}", summary.skipped);
        assert!(
            summary.skipped[0].reason.contains("could not be read"),
            "unexpected reason: {}",
            summary.skipped[0].reason
        );
        assert!(summary.failures.is_empty());
    }
}
