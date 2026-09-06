//! Codex memories file management (issue #296).
//!
//! Codex stores its memory files under `<codex_root>/memories/` (`MEMORY.md`,
//! `memory_summary.md`, `raw_memories.md`, `rollout_summaries/`, `skills/`,
//! `extensions/ad_hoc/notes/`, plus a `.git/` consolidation workspace). The
//! on-disk layout differs between Codex versions, so this module treats the
//! directory as a generic scoped file browser instead of hardcoding filenames.
//!
//! Source switching mirrors the Codex history sync semantics: the current
//! runtime root when it is WSL Direct, otherwise the host root plus the
//! WSL-sync Codex home (`<distro>:<linux_home>/.codex`).
//!
//! All file I/O runs on the blocking pool with a wall-clock timeout because
//! WSL UNC / network roots can block `fs` calls for a long time.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use super::commands::{
    resolve_codex_history_source_candidates, CodexHistoryRuntimeSource, CodexHistorySourceCandidate,
};
use crate::db::SqliteDbState;

/// Wall-clock timeout for one memories file operation on a possibly-UNC path.
const MEMORIES_FILE_IO_TIMEOUT: Duration = Duration::from_secs(15);

/// Refuse to read memory files larger than this (memory files are markdown).
const MAX_MEMORY_FILE_READ_BYTES: u64 = 2 * 1024 * 1024;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexMemoriesSourceMode {
    Local,
    Wsl,
}

impl CodexMemoriesSourceMode {
    pub fn parse(raw: Option<&str>) -> Result<Self, String> {
        match raw
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("local")
        {
            "local" => Ok(Self::Local),
            "wsl" => Ok(Self::Wsl),
            value => Err(format!("Unsupported Codex memories source mode: {value}")),
        }
    }

    fn matches(self, source: CodexHistoryRuntimeSource) -> bool {
        matches!(
            (self, source),
            (Self::Local, CodexHistoryRuntimeSource::Local)
                | (Self::Wsl, CodexHistoryRuntimeSource::Wsl)
        )
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Wsl => "wsl",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexMemoriesSourceOption {
    pub source: String,
    pub distro: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexMemoriesEntry {
    pub name: String,
    pub relative_path: String,
    pub entry_type: String,
    pub size: u64,
    pub modified_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexMemoriesListResult {
    pub root_path: String,
    pub source: String,
    pub distro: Option<String>,
    pub available_sources: Vec<CodexMemoriesSourceOption>,
    pub entries: Vec<CodexMemoriesEntry>,
    /// True when the requested source does not exist (e.g. the host root is
    /// WSL Direct and `local` was requested). The list still succeeds so the
    /// UI can discover and switch to an available source.
    pub unavailable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexMemoryFileContent {
    pub content: String,
    pub size: u64,
    pub modified_at_ms: Option<i64>,
}

struct CodexMemoriesSource {
    codex_root: PathBuf,
    source: &'static str,
    distro: Option<String>,
}

// ============================================================================
// Source resolution
// ============================================================================

/// Resolve the memories source for `source_mode`.
///
/// Returns `None` (with the always-available source list) when the requested
/// source does not exist, so `list_codex_memories` can degrade gracefully
/// instead of failing the UI's initial load on WSL-only setups. Database
/// errors still propagate.
async fn resolve_codex_memories_context(
    db: &SqliteDbState,
    source_mode: CodexMemoriesSourceMode,
) -> Result<(Option<CodexMemoriesSource>, Vec<CodexMemoriesSourceOption>), String> {
    let candidates: Vec<CodexHistorySourceCandidate> =
        resolve_codex_history_source_candidates(db).await?;
    let available_sources = candidates
        .iter()
        .map(|candidate| CodexMemoriesSourceOption {
            source: candidate.source.as_str().to_string(),
            distro: candidate.distro.clone(),
        })
        .collect::<Vec<_>>();

    let source = candidates
        .iter()
        .find(|candidate| source_mode.matches(candidate.source))
        .map(|candidate| CodexMemoriesSource {
            codex_root: candidate.root_dir.clone(),
            source: candidate.source.as_str(),
            distro: candidate.distro.clone(),
        });
    Ok((source, available_sources))
}

fn unavailable_source_error(
    source_mode: CodexMemoriesSourceMode,
    available_sources: &[CodexMemoriesSourceOption],
) -> String {
    format!(
        "Codex memories source '{}' is not available (available: {})",
        source_mode.as_str(),
        available_sources
            .iter()
            .map(|option| option.source.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

// ============================================================================
// Path validation
// ============================================================================

/// Parse a memories-relative path into validated components.
///
/// Rejects absolute paths, `..`, and dot-hidden components so every operation
/// stays inside the memories root (mirrors the Codex memories backend's own
/// scoped-path semantics). Returns an empty vec for the memories root itself.
fn validate_relative_components(relative_path: &str) -> Result<Vec<String>, String> {
    let trimmed = relative_path.trim().trim_matches(['/', '\\']);
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let mut components = Vec::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::Normal(name) => {
                let name = name.to_string_lossy().to_string();
                if name.starts_with('.') {
                    return Err(format!(
                        "Memory paths must not contain hidden components: {relative_path}"
                    ));
                }
                // Windows drive prefixes and separators are not Prefix/root
                // components on other platforms; reject them explicitly so
                // validation behaves the same on every OS.
                if name.contains('\\') || name.contains(':') {
                    return Err(format!(
                        "Memory path must stay within the memories root: {relative_path}"
                    ));
                }
                components.push(name);
            }
            _ => {
                return Err(format!(
                    "Memory path must stay within the memories root: {relative_path}"
                ))
            }
        }
    }
    Ok(components)
}

/// Validate a user-provided file or directory name (rename target, new file).
fn validate_entry_name(entry_name: &str) -> Result<String, String> {
    let trimmed = entry_name.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return Err(format!("Invalid memory entry name: {entry_name}"));
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err(format!(
            "Memory entry name must not contain path separators: {entry_name}"
        ));
    }
    if trimmed.starts_with('.') {
        return Err(format!(
            "Memory entry name must not start with a dot: {entry_name}"
        ));
    }
    Ok(trimmed.to_string())
}

/// Walk the component list under `memories_root`, rejecting symlinks and
/// file-in-the-middle traversals. Must run on the blocking pool (UNC).
fn resolve_scoped_memory_path(
    memories_root: &Path,
    components: &[String],
) -> Result<PathBuf, String> {
    let mut current = memories_root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        current.push(component);
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "Memory paths must not traverse symlinks: {}",
                    current.display()
                ));
            }
            if index + 1 < components.len() && !metadata.is_dir() {
                return Err(format!(
                    "Memory path traverses a non-directory component: {}",
                    current.display()
                ));
            }
        }
    }
    Ok(current)
}

fn memory_display_relative_path(components: &[String]) -> String {
    components.join("/")
}

// ============================================================================
// Blocking file operations
// ============================================================================

fn modified_time_ms(metadata: &fs::Metadata) -> Option<i64> {
    metadata.modified().ok().map(|time| {
        time.duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0)
    })
}

fn list_memories_blocking(
    memories_root: &Path,
    relative_path: &str,
) -> Result<Vec<CodexMemoriesEntry>, String> {
    let components = validate_relative_components(relative_path)?;
    let target = resolve_scoped_memory_path(memories_root, &components)?;

    let metadata = fs::symlink_metadata(&target)
        .map_err(|error| format!("Memory path not accessible ({}): {error}", target.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "Memory paths must not traverse symlinks: {}",
            target.display()
        ));
    }

    let mut entries = Vec::new();
    if metadata.is_file() {
        entries.push(CodexMemoriesEntry {
            name: components
                .last()
                .cloned()
                .unwrap_or_else(|| memories_root.display().to_string()),
            relative_path: memory_display_relative_path(&components),
            entry_type: "file".to_string(),
            size: metadata.len(),
            modified_at_ms: modified_time_ms(&metadata),
        });
        return Ok(entries);
    }

    let dir_entries = fs::read_dir(&target).map_err(|error| {
        format!(
            "Failed to read memory directory ({}): {error}",
            target.display()
        )
    })?;
    for entry in dir_entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let Ok(entry_metadata) = entry.metadata() else {
            continue;
        };
        if entry_metadata.file_type().is_symlink() {
            continue;
        }
        let entry_type = if entry_metadata.is_dir() {
            "directory"
        } else if entry_metadata.is_file() {
            "file"
        } else {
            continue;
        };
        let mut child_components = components.clone();
        child_components.push(name.clone());
        entries.push(CodexMemoriesEntry {
            name,
            relative_path: memory_display_relative_path(&child_components),
            entry_type: entry_type.to_string(),
            size: entry_metadata.len(),
            modified_at_ms: modified_time_ms(&entry_metadata),
        });
    }

    entries.sort_by(|left, right| {
        let left_dir = left.entry_type == "directory";
        let right_dir = right.entry_type == "directory";
        right_dir
            .cmp(&left_dir)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(entries)
}

fn read_memory_file_blocking(
    memories_root: &Path,
    relative_path: &str,
) -> Result<CodexMemoryFileContent, String> {
    let components = validate_relative_components(relative_path)?;
    if components.is_empty() {
        return Err("Memory file path must point to a file".to_string());
    }
    let target = resolve_scoped_memory_path(memories_root, &components)?;

    let metadata = fs::symlink_metadata(&target)
        .map_err(|error| format!("Memory file not accessible ({}): {error}", target.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "Memory file must not be a symlink: {}",
            target.display()
        ));
    }
    if !metadata.is_file() {
        return Err(format!("Memory path is not a file: {}", target.display()));
    }
    if metadata.len() > MAX_MEMORY_FILE_READ_BYTES {
        return Err(format!(
            "Memory file is too large to open ({} > {} bytes): {}",
            metadata.len(),
            MAX_MEMORY_FILE_READ_BYTES,
            target.display()
        ));
    }

    let content = fs::read_to_string(&target)
        .map_err(|error| format!("Failed to read memory file ({}): {error}", target.display()))?;
    Ok(CodexMemoryFileContent {
        size: metadata.len(),
        modified_at_ms: modified_time_ms(&metadata),
        content,
    })
}

fn write_memory_file_blocking(
    memories_root: &Path,
    relative_path: &str,
    content: &str,
) -> Result<(), String> {
    let components = validate_relative_components(relative_path)?;
    if components.is_empty() {
        return Err("Memory file path must point to a file".to_string());
    }
    let target = resolve_scoped_memory_path(memories_root, &components)?;
    if let Ok(metadata) = fs::symlink_metadata(&target) {
        if metadata.file_type().is_symlink() || metadata.is_dir() {
            return Err(format!(
                "Memory file path is not a writable file: {}",
                target.display()
            ));
        }
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create memory directory ({}): {error}",
                parent.display()
            )
        })?;
    }

    fs::write(&target, content).map_err(|error| {
        format!(
            "Failed to write memory file ({}): {error}",
            target.display()
        )
    })?;
    Ok(())
}

fn rename_memory_entry_blocking(
    memories_root: &Path,
    relative_path: &str,
    new_name: &str,
) -> Result<(), String> {
    let components = validate_relative_components(relative_path)?;
    if components.is_empty() {
        return Err("Memory path must point to a file or directory".to_string());
    }
    let validated_name = validate_entry_name(new_name)?;
    let source = resolve_scoped_memory_path(memories_root, &components)?;

    let metadata = fs::symlink_metadata(&source)
        .map_err(|error| format!("Memory path not accessible ({}): {error}", source.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "Memory paths must not be symlinks: {}",
            source.display()
        ));
    }

    let mut target_components = components.clone();
    let last_index = target_components.len() - 1;
    target_components[last_index] = validated_name;
    let target = resolve_scoped_memory_path(memories_root, &target_components)?;
    if fs::symlink_metadata(&target).is_ok() {
        return Err(format!("Memory entry already exists: {}", target.display()));
    }

    fs::rename(&source, &target).map_err(|error| {
        format!(
            "Failed to rename memory entry ({} -> {}): {error}",
            source.display(),
            target.display()
        )
    })?;
    Ok(())
}

fn delete_memory_entries_blocking(
    memories_root: &Path,
    relative_paths: &[String],
) -> Result<(), String> {
    let mut errors = Vec::new();
    for relative_path in relative_paths {
        let operation = || -> Result<(), String> {
            let components = validate_relative_components(relative_path)?;
            if components.is_empty() {
                return Err("Memory path must point to a file or directory".to_string());
            }
            let target = resolve_scoped_memory_path(memories_root, &components)?;
            let metadata = match fs::symlink_metadata(&target) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(error) => {
                    return Err(format!(
                        "Memory path not accessible ({}): {error}",
                        target.display()
                    ))
                }
            };
            if metadata.file_type().is_symlink() {
                fs::remove_file(&target).map_err(|error| {
                    format!(
                        "Failed to remove memory symlink ({}): {error}",
                        target.display()
                    )
                })?;
            } else if metadata.is_dir() {
                fs::remove_dir_all(&target).map_err(|error| {
                    format!(
                        "Failed to remove memory directory ({}): {error}",
                        target.display()
                    )
                })?;
            } else {
                fs::remove_file(&target).map_err(|error| {
                    format!(
                        "Failed to remove memory file ({}): {error}",
                        target.display()
                    )
                })?;
            }
            Ok(())
        };
        if let Err(error) = operation() {
            errors.push(error);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

/// Clear the visible contents of one memory root.
///
/// This mirrors upstream `clear_memory_root_contents` (clear contents, keep
/// the directory itself, refuse symlinked roots) with one deliberate
/// deviation: dot-hidden top-level entries such as the consolidation
/// `.git/` workspace are kept, so Codex keeps its baseline for the next
/// consolidation diff instead of rebuilding the whole workspace history.
fn clear_memory_root_contents_blocking(memory_root: &Path) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(memory_root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "Memory root not accessible ({}): {error}",
                memory_root.display()
            ))
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "Refusing to clear symlinked memory root {}",
            memory_root.display()
        ));
    }

    let entries = fs::read_dir(memory_root).map_err(|error| {
        format!(
            "Failed to read memory root ({}): {error}",
            memory_root.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "Failed to read memory root ({}): {error}",
                memory_root.display()
            )
        })?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "Failed to inspect memory entry ({}): {error}",
                path.display()
            )
        })?;
        let result = if file_type.is_dir() && !file_type.is_symlink() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        result.map_err(|error| {
            format!("Failed to clear memory entry ({}): {error}", path.display())
        })?;
    }
    Ok(())
}

fn clear_memories_blocking(codex_root: &Path) -> Result<(), String> {
    let memories_root = codex_root.join("memories");
    let memories_extensions_root = codex_root.join("memories_extensions");
    clear_memory_root_contents_blocking(&memories_root)?;
    if fs::symlink_metadata(&memories_extensions_root).is_ok() {
        clear_memory_root_contents_blocking(&memories_extensions_root)?;
    }
    Ok(())
}

// ============================================================================
// Timed command wrapper
// ============================================================================

async fn run_memories_fs_operation<T, F>(
    operation_label: &str,
    display_path: &str,
    operation: F,
) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    let label_owned = operation_label.to_string();
    let display_path_owned = display_path.to_string();
    match tokio::time::timeout(
        MEMORIES_FILE_IO_TIMEOUT,
        tauri::async_runtime::spawn_blocking(operation),
    )
    .await
    {
        Ok(Ok(result)) => result,
        Ok(Err(join_error)) => Err(format!(
            "Failed to {label_owned} ({display_path_owned}): {join_error}"
        )),
        Err(_) => Err(format!(
            "Timed out after {}s while trying to {label_owned} ({display_path_owned}). If this is a WSL or network path, check that the distro/share is running and accessible.",
            MEMORIES_FILE_IO_TIMEOUT.as_secs()
        )),
    }
}

// ============================================================================
// Tauri commands
// ============================================================================

#[tauri::command]
pub async fn list_codex_memories(
    state: tauri::State<'_, SqliteDbState>,
    source_mode: Option<String>,
    relative_path: Option<String>,
) -> Result<CodexMemoriesListResult, String> {
    let db = state.db();
    let parsed_mode = CodexMemoriesSourceMode::parse(source_mode.as_deref())?;
    let (source, available_sources) = resolve_codex_memories_context(db, parsed_mode).await?;
    let Some(source) = source else {
        return Ok(CodexMemoriesListResult {
            root_path: String::new(),
            source: parsed_mode.as_str().to_string(),
            distro: None,
            available_sources,
            entries: Vec::new(),
            unavailable: true,
        });
    };
    let memories_root = source.codex_root.join("memories");
    let relative_path = relative_path.unwrap_or_default();
    let display_root = memories_root.to_string_lossy().to_string();

    let entries = run_memories_fs_operation("list Codex memories", &display_root, move || {
        list_memories_blocking(&memories_root, &relative_path)
    })
    .await?;

    Ok(CodexMemoriesListResult {
        root_path: display_root,
        source: source.source.to_string(),
        distro: source.distro,
        available_sources,
        entries,
        unavailable: false,
    })
}

#[tauri::command]
pub async fn read_codex_memory_file(
    state: tauri::State<'_, SqliteDbState>,
    source_mode: Option<String>,
    relative_path: String,
) -> Result<CodexMemoryFileContent, String> {
    let db = state.db();
    let parsed_mode = CodexMemoriesSourceMode::parse(source_mode.as_deref())?;
    let (source, available_sources) = resolve_codex_memories_context(db, parsed_mode).await?;
    let source = source.ok_or_else(|| unavailable_source_error(parsed_mode, &available_sources))?;
    let memories_root = source.codex_root.join("memories");
    let display_root = memories_root.to_string_lossy().to_string();

    run_memories_fs_operation("read Codex memory file", &display_root, move || {
        read_memory_file_blocking(&memories_root, &relative_path)
    })
    .await
}

#[tauri::command]
pub async fn write_codex_memory_file(
    state: tauri::State<'_, SqliteDbState>,
    source_mode: Option<String>,
    relative_path: String,
    content: String,
) -> Result<(), String> {
    let db = state.db();
    let parsed_mode = CodexMemoriesSourceMode::parse(source_mode.as_deref())?;
    let (source, available_sources) = resolve_codex_memories_context(db, parsed_mode).await?;
    let source = source.ok_or_else(|| unavailable_source_error(parsed_mode, &available_sources))?;
    let memories_root = source.codex_root.join("memories");
    let display_root = memories_root.to_string_lossy().to_string();

    run_memories_fs_operation("write Codex memory file", &display_root, move || {
        write_memory_file_blocking(&memories_root, &relative_path, &content)
    })
    .await
}

#[tauri::command]
pub async fn rename_codex_memory_entry(
    state: tauri::State<'_, SqliteDbState>,
    source_mode: Option<String>,
    relative_path: String,
    new_name: String,
) -> Result<(), String> {
    let db = state.db();
    let parsed_mode = CodexMemoriesSourceMode::parse(source_mode.as_deref())?;
    let (source, available_sources) = resolve_codex_memories_context(db, parsed_mode).await?;
    let source = source.ok_or_else(|| unavailable_source_error(parsed_mode, &available_sources))?;
    let memories_root = source.codex_root.join("memories");
    let display_root = memories_root.to_string_lossy().to_string();

    run_memories_fs_operation("rename Codex memory entry", &display_root, move || {
        rename_memory_entry_blocking(&memories_root, &relative_path, &new_name)
    })
    .await
}

#[tauri::command]
pub async fn delete_codex_memory_entries(
    state: tauri::State<'_, SqliteDbState>,
    source_mode: Option<String>,
    relative_paths: Vec<String>,
) -> Result<(), String> {
    let db = state.db();
    let parsed_mode = CodexMemoriesSourceMode::parse(source_mode.as_deref())?;
    let (source, available_sources) = resolve_codex_memories_context(db, parsed_mode).await?;
    let source = source.ok_or_else(|| unavailable_source_error(parsed_mode, &available_sources))?;
    let memories_root = source.codex_root.join("memories");
    let display_root = memories_root.to_string_lossy().to_string();

    run_memories_fs_operation("delete Codex memory entries", &display_root, move || {
        delete_memory_entries_blocking(&memories_root, &relative_paths)
    })
    .await
}

#[tauri::command]
pub async fn clear_codex_memories(
    state: tauri::State<'_, SqliteDbState>,
    source_mode: Option<String>,
) -> Result<(), String> {
    let db = state.db();
    let parsed_mode = CodexMemoriesSourceMode::parse(source_mode.as_deref())?;
    let (source, available_sources) = resolve_codex_memories_context(db, parsed_mode).await?;
    let source = source.ok_or_else(|| unavailable_source_error(parsed_mode, &available_sources))?;
    let codex_root = source.codex_root;
    let display_root = codex_root.to_string_lossy().to_string();

    run_memories_fs_operation("clear Codex memories", &display_root, move || {
        clear_memories_blocking(&codex_root)
    })
    .await
}

#[tauri::command]
pub async fn reveal_codex_memories_folder(
    state: tauri::State<'_, SqliteDbState>,
    source_mode: Option<String>,
) -> Result<(), String> {
    let db = state.db();
    let parsed_mode = CodexMemoriesSourceMode::parse(source_mode.as_deref())?;
    let (source, available_sources) = resolve_codex_memories_context(db, parsed_mode).await?;
    let source = source.ok_or_else(|| unavailable_source_error(parsed_mode, &available_sources))?;
    let memories_root = source.codex_root.join("memories");

    let display_root = memories_root.to_string_lossy().to_string();
    let ensure_root = memories_root.clone();
    run_memories_fs_operation("open Codex memories folder", &display_root, move || {
        fs::create_dir_all(&ensure_root)
            .map_err(|error| format!("Failed to create memories directory: {error}"))?;
        Ok(())
    })
    .await?;

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&memories_root)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&memories_root)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&memories_root)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn write_test_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn relative_path_validation_rejects_escape_and_hidden() {
        assert!(validate_relative_components("..").is_err());
        assert!(validate_relative_components("a/../b").is_err());
        assert!(validate_relative_components("../x").is_err());
        assert!(validate_relative_components("C:\\x").is_err());
        assert!(validate_relative_components("x:y").is_err());
        assert!(validate_relative_components(".git").is_err());
        assert!(validate_relative_components("skills/.hidden").is_err());
        assert!(validate_relative_components("").unwrap().is_empty());
        assert!(validate_relative_components("  ").unwrap().is_empty());
    }

    #[test]
    fn relative_path_validation_accepts_plain_paths() {
        assert!(
            validate_relative_components("rollout_summaries/abc.md")
                .unwrap()
                .join("/")
                == "rollout_summaries/abc.md"
        );
        assert!(
            validate_relative_components("extensions\\ad_hoc\\notes")
                .unwrap()
                .join("/")
                == "extensions/ad_hoc/notes"
        );
        assert!(validate_relative_components("MEMORY.md").unwrap().join("/") == "MEMORY.md");
        // Leading/trailing separators are normalized away; the result still
        // stays inside the memories root.
        assert_eq!(validate_relative_components("/x/").unwrap().join("/"), "x");
    }

    #[test]
    fn entry_name_validation_rejects_paths_and_hidden() {
        assert!(validate_entry_name("").is_err());
        assert!(validate_entry_name(".").is_err());
        assert!(validate_entry_name("..").is_err());
        assert!(validate_entry_name("a/b").is_err());
        assert!(validate_entry_name("a\\b").is_err());
        assert!(validate_entry_name(".git").is_err());
        assert_eq!(validate_entry_name(" note-1.md ").unwrap(), "note-1.md");
    }

    #[test]
    fn list_sorts_directories_first_and_skips_hidden() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        write_test_file(&root.join("memory_summary.md"), "v1");
        write_test_file(&root.join("raw_memories.md"), "raw");
        write_test_file(&root.join("rollout_summaries/session-a.md"), "a");
        write_test_file(&root.join(".git/HEAD"), "ref");
        fs::create_dir_all(root.join("skills")).unwrap();

        let entries = list_memories_blocking(root, "").unwrap();
        let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "rollout_summaries",
                "skills",
                "memory_summary.md",
                "raw_memories.md"
            ]
        );
        assert_eq!(entries[0].entry_type, "directory");
        assert_eq!(entries[2].entry_type, "file");
        assert_eq!(entries[2].relative_path, "memory_summary.md");
    }

    #[test]
    fn list_reports_missing_directory_as_error() {
        let temp_dir = tempfile::tempdir().unwrap();
        let missing = temp_dir.path().join("memories");
        assert!(list_memories_blocking(&missing, "").is_err());
    }

    #[test]
    fn read_and_write_round_trip_and_enforce_size_limit() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        write_memory_file_blocking(root, "extensions/ad_hoc/notes/note.md", "hello").unwrap();
        let content = read_memory_file_blocking(root, "extensions/ad_hoc/notes/note.md").unwrap();
        assert_eq!(content.content, "hello");
        assert_eq!(content.size, 5);
        assert!(content.modified_at_ms.unwrap() > 0);

        write_memory_file_blocking(root, "MEMORY.md", "long-term").unwrap();
        assert_eq!(
            read_memory_file_blocking(root, "MEMORY.md")
                .unwrap()
                .content,
            "long-term"
        );

        let oversized = "x".repeat(MAX_MEMORY_FILE_READ_BYTES as usize + 1);
        write_test_file(&root.join("big.md"), &oversized);
        let error = read_memory_file_blocking(root, "big.md").unwrap_err();
        assert!(error.contains("too large"), "unexpected error: {error}");
    }

    #[test]
    fn write_refuses_hidden_and_escape_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        assert!(write_memory_file_blocking(root, "../escape.md", "x").is_err());
        assert!(write_memory_file_blocking(root, ".git/config", "x").is_err());
        assert!(write_memory_file_blocking(root, "", "x").is_err());
    }

    #[test]
    fn rename_rejects_conflicts_and_cross_component_names() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        write_test_file(&root.join("a.md"), "a");
        write_test_file(&root.join("b.md"), "b");

        rename_memory_entry_blocking(root, "a.md", "b.md").unwrap_err();
        rename_memory_entry_blocking(root, "a.md", "sub/c.md").unwrap_err();
        rename_memory_entry_blocking(root, "a.md", ".hidden").unwrap_err();
        rename_memory_entry_blocking(root, "a.md", "renamed.md").unwrap();
        assert!(root.join("renamed.md").exists());
        assert!(!root.join("a.md").exists());
    }

    #[test]
    fn delete_removes_files_and_directories_and_tolerates_missing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        write_test_file(&root.join("MEMORY.md"), "m");
        write_test_file(&root.join("rollout_summaries/a.md"), "a");

        delete_memory_entries_blocking(
            root,
            &[
                "MEMORY.md".to_string(),
                "rollout_summaries".to_string(),
                "gone.md".to_string(),
            ],
        )
        .unwrap();
        assert!(!root.join("MEMORY.md").exists());
        assert!(!root.join("rollout_summaries").exists());

        assert!(delete_memory_entries_blocking(root, &["../outside.md".to_string()]).is_err());
    }

    #[test]
    fn clear_removes_visible_entries_and_keeps_hidden_and_extensions_root() {
        let temp_dir = tempfile::tempdir().unwrap();
        let codex_root = temp_dir.path();
        let memories_root = codex_root.join("memories");
        write_test_file(&memories_root.join("MEMORY.md"), "m");
        write_test_file(&memories_root.join("memory_summary.md"), "v1");
        write_test_file(&memories_root.join(".git/HEAD"), "ref");
        write_test_file(&memories_root.join("rollout_summaries/a.md"), "a");
        let extensions_root = codex_root.join("memories_extensions");
        write_test_file(&extensions_root.join("ad_hoc/note.md"), "note");

        clear_memories_blocking(codex_root).unwrap();

        assert!(!memories_root.join("MEMORY.md").exists());
        assert!(!memories_root.join("memory_summary.md").exists());
        assert!(!memories_root.join("rollout_summaries").exists());
        assert!(memories_root.join(".git/HEAD").exists());
        assert!(memories_root.exists(), "memories root directory is kept");
        assert!(!extensions_root.join("ad_hoc").exists());
        assert!(
            extensions_root.exists(),
            "extensions root directory is kept"
        );
    }

    #[test]
    fn clear_is_noop_when_roots_missing() {
        let temp_dir = tempfile::tempdir().unwrap();
        clear_memories_blocking(temp_dir.path()).unwrap();
    }

    #[test]
    fn modified_time_ms_maps_to_epoch_millis() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("t.md");
        write_test_file(&file_path, "x");
        let metadata = fs::metadata(&file_path).unwrap();
        let millis = modified_time_ms(&metadata).unwrap();
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0));
        assert!(millis <= now.as_millis() as i64 + 1000);
    }

    #[test]
    fn source_mode_parse_defaults_to_local() {
        assert_eq!(
            CodexMemoriesSourceMode::parse(None).unwrap(),
            CodexMemoriesSourceMode::Local
        );
        assert_eq!(
            CodexMemoriesSourceMode::parse(Some("wsl")).unwrap(),
            CodexMemoriesSourceMode::Wsl
        );
        assert!(CodexMemoriesSourceMode::parse(Some("all")).is_err());
    }
}
