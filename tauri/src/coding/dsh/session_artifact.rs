//! dsh session artifact naming — the single source of truth for "which file is
//! a dsh session, and which generation of it is live".
//!
//! Layout: `<sessions root>/<project key>/<encoded session id>/session[.v<N>].jsonl[.zstd]`
//!
//! The filename encodes the on-disk **Session format generation**:
//! - `session.jsonl` is generation 0 (the only spelling for v0),
//! - `session.v<N>.jsonl` is generation `N` for `N > 0`,
//! - a `.zstd` suffix is the physical encoding, not part of the generation.
//!
//! dsh keeps every generation of one session side by side (a write-open
//! migration publishes a new file and never rewrites or deletes the old one),
//! so one session directory may hold several artifacts at once. The
//! numerically highest generation is the live one — mirroring upstream's
//! reader, which selects the highest canonical generation.
//!
//! Consumed by both `session_manager` (browsing) and `proxy_gateway` (usage
//! import); this module is the only place the naming rule may live.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The on-disk format generation a dsh session artifact filename encodes.
///
/// Returns `None` for anything that is not a canonical artifact name. A
/// leading zero (`session.v0.jsonl`, `session.v01.jsonl`) is deliberately not
/// canonical: generation 0 is spelled `session.jsonl`, so a zero-padded name is
/// either a foreign artifact or a bug, never a generation to select.
pub fn generation(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    let name = name.strip_suffix(".zstd").unwrap_or(name);
    if name == "session.jsonl" {
        return Some(0);
    }
    let version = name.strip_prefix("session.v")?.strip_suffix(".jsonl")?;
    if version.starts_with('0') {
        return None;
    }
    version.parse().ok()
}

/// Whether `path` is a dsh session artifact of any generation.
pub fn is_session_artifact(path: &Path) -> bool {
    generation(path).is_some()
}

/// Keep only the live artifact per session directory.
///
/// One artifact survives per parent directory: the numerically highest
/// generation. When two files in the same directory name the same generation,
/// the extension breaks the tie, so a compressed `.jsonl.zstd` wins over a
/// plain `.jsonl` of the same generation. The result keeps the input paths but
/// is ordered by directory, not by recency.
pub fn select_generations(files: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut selected = BTreeMap::<PathBuf, PathBuf>::new();
    for file in files {
        let directory = file.parent().unwrap_or(Path::new(".")).to_path_buf();
        let replace = selected.get(&directory).is_none_or(|old| {
            (generation(&file), file.extension()) > (generation(old), old.extension())
        });
        if replace {
            selected.insert(directory, file);
        }
    }
    selected.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_reads_the_bare_name_as_v0() {
        assert_eq!(generation(Path::new("/s/session.jsonl")), Some(0));
        assert_eq!(generation(Path::new("/s/session.jsonl.zstd")), Some(0));
    }

    #[test]
    fn generation_reads_versioned_names() {
        assert_eq!(generation(Path::new("/s/session.v3.jsonl")), Some(3));
        assert_eq!(generation(Path::new("/s/session.v3.jsonl.zstd")), Some(3));
        assert_eq!(generation(Path::new("/s/session.v12.jsonl.zstd")), Some(12));
    }

    #[test]
    fn generation_rejects_noncanonical_names() {
        // Generation 0 is spelled without a `.v` component.
        assert_eq!(generation(Path::new("/s/session.v0.jsonl")), None);
        assert_eq!(generation(Path::new("/s/session.v01.jsonl")), None);
        // Not a session artifact at all.
        assert_eq!(generation(Path::new("/s/other.jsonl")), None);
        assert_eq!(generation(Path::new("/s/session.jsonl.gz")), None);
        assert_eq!(generation(Path::new("/s/session.vX.jsonl")), None);
        assert!(!is_session_artifact(Path::new("/s/session_index.jsonl")));
        assert!(is_session_artifact(Path::new("/s/session.v3.jsonl.zstd")));
    }

    #[test]
    fn select_generations_prefers_the_highest_generation_per_directory() {
        let files = vec![
            PathBuf::from("/a/session.jsonl.zstd"),
            PathBuf::from("/a/session.v3.jsonl.zstd"),
            PathBuf::from("/b/session.jsonl"),
            PathBuf::from("/b/session.jsonl.zstd"),
        ];
        let selected = select_generations(files);
        assert_eq!(
            selected,
            vec![
                PathBuf::from("/a/session.v3.jsonl.zstd"),
                // Equal generation: the compressed artifact wins.
                PathBuf::from("/b/session.jsonl.zstd"),
            ]
        );
    }

    #[test]
    fn select_generations_keeps_directories_with_a_single_artifact() {
        let files = vec![
            PathBuf::from("/a/session.jsonl"),
            PathBuf::from("/b/session.v3.jsonl.zstd"),
        ];
        assert_eq!(select_generations(files.clone()), files);
        assert!(select_generations(vec![]).is_empty());
    }
}
