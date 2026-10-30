// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared local path expansion and portable backup normalization.
//! Extracted from AI Toolbox; original authorship and license apply.
//! No target file, database, GUI or owner is opened by this service.
use std::path::{Path, PathBuf};

/// Only path-related host values are captured. Tests inject their own context.
#[derive(Clone, Debug, Default)]
pub struct PathContext {
    pub home_dir: Option<PathBuf>,
    pub config_dir: Option<PathBuf>,
    pub case_insensitive: bool,
    pub variables: Vec<(&'static str, Option<String>)>,
}
impl PathContext {
    pub fn current() -> Self {
        Self {
            home_dir: dirs::home_dir(),
            config_dir: dirs::config_dir(),
            case_insensitive: cfg!(windows),
            variables: vec![
                ("USERPROFILE", std::env::var("USERPROFILE").ok()),
                ("APPDATA", std::env::var("APPDATA").ok()),
                ("LOCALAPPDATA", std::env::var("LOCALAPPDATA").ok()),
                ("HOME", std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok()),
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathLimitExceeded;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathRequestError { InvalidInput, ExpansionLimitExceeded }
impl PathRequestError {
    pub fn message(self) -> &'static str {
        match self {
            Self::InvalidInput => "Backup path must contain no NUL bytes and fit within 32768 UTF-8 bytes.",
            Self::ExpansionLimitExceeded => "Backup path expansion exceeds the supported size.",
        }
    }
}
/// UTF-8 bytes of the decoded path field, not JSON frame or argv bytes.
pub const PROTOCOL_INPUT_LIMIT: usize = 32 * 1024;
pub const PROTOCOL_OUTPUT_LIMIT: usize = 1024 * 1024;

pub fn validate_backup_path_input(input: &str) -> Result<(), PathRequestError> {
    if input.len() > PROTOCOL_INPUT_LIMIT || input.contains('\0') { Err(PathRequestError::InvalidInput) } else { Ok(()) }
}

fn append_checked(out: &mut String, value: &str, limit: usize) -> Result<(), PathLimitExceeded> {
    out.len().checked_add(value.len()).filter(|n| *n <= limit).ok_or(PathLimitExceeded)?;
    out.push_str(value);
    Ok(())
}
fn replace_checked(input: &str, needle: &str, value: &str, limit: usize) -> Result<String, PathLimitExceeded> {
    let count = input.matches(needle).count();
    let len = input.len().checked_sub(count * needle.len())
        .and_then(|n| value.len().checked_mul(count).and_then(|v| n.checked_add(v)))
        .filter(|n| *n <= limit).ok_or(PathLimitExceeded)?;
    let mut out = String::with_capacity(len);
    let mut pieces = input.split(needle);
    out.push_str(pieces.next().unwrap_or_default());
    for piece in pieces { out.push_str(value); out.push_str(piece); }
    Ok(out)
}

/// Expand local path: `~`, `$HOME`, `%USERPROFILE%`, and other common env vars.
/// Supports Unix and Windows conventions regardless of host platform.
pub fn expand_local_path(path: &str) -> Result<String, String> {
    expand_local_path_with_context(path, &PathContext::current())
}
pub fn expand_local_path_with_context(path: &str, context: &PathContext) -> Result<String, String> {
    expand_with_limit(path, context, usize::MAX).map_err(|_| "Expanded path exceeds capacity".to_string())
}
fn expand_with_limit(path: &str, context: &PathContext, limit: usize) -> Result<String, PathLimitExceeded> {
    if path.len() > limit { return Err(PathLimitExceeded); }
    let mut result = path.to_string();
    // Expand only the leading tilde, preserving existing path semantics.
    if result.starts_with("~/") || result == "~" {
        if let Some(home) = &context.home_dir {
            let mut expanded = String::new();
            append_checked(&mut expanded, &home.to_string_lossy(), limit)?;
            append_checked(&mut expanded, &result[1..], limit)?;
            result = expanded;
        }
    }
    // Preserve ordered Windows then Unix expansion, including HOME fallback.
    for (var, value) in &context.variables {
        if let Some(value) = value {
            result = replace_checked(&result, &format!("%{var}%"), value, limit)?;
            result = replace_unix_with_limit(&result, var, value, limit)?;
        }
    }
    Ok(result)
}
fn replace_unix_with_limit(input: &str, name: &str, value: &str, limit: usize) -> Result<String, PathLimitExceeded> {
    let input = replace_checked(input, &format!("${{{name}}}"), value, limit)?;
    let reference = format!("${name}");
    let mut out = String::new();
    let mut remaining = input.as_str();
    while let Some(index) = remaining.find(&reference) {
        append_checked(&mut out, &remaining[..index], limit)?;
        let after = &remaining[index + reference.len()..];
        let continues = after.chars().next().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        append_checked(&mut out, if continues { &reference } else { value }, limit)?;
        remaining = after;
    }
    append_checked(&mut out, remaining, limit)?;
    Ok(out)
}

fn strip_base_path(candidate: &str, base: &Path, case_insensitive: bool) -> Option<String> {
    let candidate = candidate.replace('\\', "/");
    let base = base.to_string_lossy().replace('\\', "/");
    let base = base.trim_end_matches('/');
    let (candidate_cmp, base_cmp) = if case_insensitive {
        (candidate.to_lowercase(), base.to_lowercase())
    } else { (candidate.clone(), base.to_string()) };
    if candidate_cmp == base_cmp { return Some(String::new()); }
    if !candidate_cmp.starts_with(&format!("{base_cmp}/")) { return None; }
    // Case folding can change UTF-8 lengths. Locate the original separator
    // using folded byte counts rather than slicing with the base's byte length.
    let mut folded_bytes = 0;
    for (index, c) in candidate.char_indices() {
        if folded_bytes == base_cmp.len() && c == '/' { return Some(candidate[index + 1..].to_string()); }
        folded_bytes += if case_insensitive { c.to_lowercase().map(char::len_utf8).sum::<usize>() } else { c.len_utf8() };
        if folded_bytes > base_cmp.len() { break; }
    }
    None
}
fn with_storage_prefix(prefix: &str, relative: &str, limit: usize) -> Result<String, PathLimitExceeded> {
    let mut out = String::new();
    append_checked(&mut out, prefix, limit)?;
    if !relative.is_empty() {
        append_checked(&mut out, "/", limit)?;
        append_checked(&mut out, relative.trim_start_matches('/'), limit)?;
    }
    Ok(out)
}

/// Normalize user-entered backup paths for portable storage.
/// Config-relative paths prefer `%APPDATA%`, then home-relative paths `~`.
/// This is not canonicalization or a path-safety validator: `..` is preserved.
pub fn normalize_backup_storage_path(input: &str) -> String {
    normalize_backup_storage_path_with_context(input, &PathContext::current())
}
pub fn normalize_backup_storage_path_with_context(input: &str, context: &PathContext) -> String {
    normalize_with_limit(input, context, usize::MAX).unwrap_or_else(|_| input.trim().to_string())
}
/// The exact service entry shared by desktop, CLI and MCP adapters.
pub fn normalize_backup_path_request(input: &str) -> Result<String, PathRequestError> {
    // Reject before host context resolution, identically for desktop/CLI/MCP.
    validate_backup_path_input(input)?;
    normalize_with_limit(input, &PathContext::current(), PROTOCOL_OUTPUT_LIMIT)
        .map_err(|_| PathRequestError::ExpansionLimitExceeded)
}
pub fn normalize_with_limit(input: &str, context: &PathContext, limit: usize) -> Result<String, PathLimitExceeded> {
    if input.len() > limit { return Err(PathLimitExceeded); }
    let result = normalize_inner(input, context, limit)?;
    if result.len() > limit { return Err(PathLimitExceeded); }
    Ok(result)
}
fn normalize_inner(input: &str, context: &PathContext, limit: usize) -> Result<String, PathLimitExceeded> {
    let trimmed = input.trim();
    if trimmed.is_empty() { return Ok(String::new()); }
    let normalized = trimmed.replace('\\', "/");
    // ASCII case-insensitive token match avoids allocating an uppercased path.
    if normalized.eq_ignore_ascii_case("%APPDATA%") { return Ok("%APPDATA%".to_string()); }
    if normalized.get(..10).is_some_and(|s| s.eq_ignore_ascii_case("%APPDATA%/")) {
        return with_storage_prefix("%APPDATA%", &normalized[10..], limit);
    }
    if normalized == "~" { return Ok("~".to_string()); }
    if let Some(relative) = normalized.strip_prefix("~/") { return with_storage_prefix("~", relative, limit); }
    let expanded = expand_with_limit(trimmed, context, limit)?.replace('\\', "/");
    let candidate = if expanded != normalized { &expanded } else { &normalized };
    if let Some(config) = &context.config_dir {
        if let Some(relative) = strip_base_path(candidate, config, context.case_insensitive) { return with_storage_prefix("%APPDATA%", &relative, limit); }
    }
    if let Some(home) = &context.home_dir {
        if let Some(relative) = strip_base_path(candidate, home, context.case_insensitive) { return with_storage_prefix("~", &relative, limit); }
    }
    Ok(candidate.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protocol_input_limit_counts_decoded_utf8_bytes_without_changing_legacy_wrapper() {
        assert_eq!(validate_backup_path_input(&"x".repeat(PROTOCOL_INPUT_LIMIT)), Ok(()));
        assert_eq!(validate_backup_path_input(&"x".repeat(PROTOCOL_INPUT_LIMIT + 1)), Err(PathRequestError::InvalidInput));
        assert_eq!(validate_backup_path_input(&"é".repeat(PROTOCOL_INPUT_LIMIT / 2)), Ok(()));
        assert_eq!(validate_backup_path_input(&"é".repeat(PROTOCOL_INPUT_LIMIT / 2 + 1)), Err(PathRequestError::InvalidInput));
        assert_eq!(validate_backup_path_input("fixture\0path"), Err(PathRequestError::InvalidInput));
        assert_eq!(normalize_backup_storage_path_with_context("fixture\0path", &PathContext::default()), "fixture\0path");
        let legacy = "x".repeat(PROTOCOL_INPUT_LIMIT + 1);
        assert_eq!(normalize_backup_storage_path_with_context(&legacy, &PathContext::default()), legacy);
    }
    fn context() -> PathContext {
        PathContext { home_dir: Some("/fixture/home".into()), config_dir: Some("/fixture/home/config".into()), case_insensitive: false,
            variables: vec![("USERPROFILE", Some("$HOME".into())), ("HOME", Some("/fixture/home".into()))] }
    }
    #[test]
    fn unix_env_expansion_matches_complete_variable_tokens_only() {
        for (input, expected) in [("$HOME/bin", "/home/tester/bin"), ("${HOME}/bin", "/home/tester/bin"), ("$HOMEBREW_PREFIX/bin", "$HOMEBREW_PREFIX/bin"), ("$HOME_SUFFIX/bin", "$HOME_SUFFIX/bin")] {
            assert_eq!(replace_unix_with_limit(input, "HOME", "/home/tester", usize::MAX).unwrap(), expected);
        }
    }
    #[test]
    fn portable_paths_and_relative_segments_keep_existing_semantics() {
        for (input, expected) in [("~\\.config\\tool", "~/.config/tool"), ("%appdata%\\Tool", "%APPDATA%/Tool"), ("relative/../path", "relative/../path"), ("  ", ""), ("$UNKNOWN/path", "$UNKNOWN/path")] {
            assert_eq!(normalize_with_limit(input, &context(), 1024).unwrap(), expected);
        }
        assert_eq!(normalize_with_limit("$HOME/config/tool", &context(), 1024).unwrap(), "%APPDATA%/tool");
        assert_eq!(normalize_with_limit("%USERPROFILE%/file", &context(), 1024).unwrap(), "~/file");
    }
    #[test]
    fn expansion_limit_checks_intermediate_before_portable_shortening() {
        assert_eq!(normalize_with_limit("$HOME/file", &context(), 12), Err(PathLimitExceeded));
        assert_eq!(normalize_with_limit("12345", &context(), 4), Err(PathLimitExceeded));
        assert_eq!(normalize_with_limit("1234", &context(), 4).unwrap(), "1234");
        let short_config = PathContext { config_dir: Some("/c".into()), ..Default::default() };
        assert_eq!(normalize_with_limit("/c/x", &short_config, 4), Err(PathLimitExceeded));
        let ctx = PathContext { variables: vec![("HOME", Some("x".repeat(30)))], ..Default::default() };
        assert_eq!(normalize_with_limit("$HOME/$HOME", &ctx, 40), Err(PathLimitExceeded));
    }
    #[test]
    fn unicode_case_folding_uses_original_separator_boundary() {
        let ctx = PathContext { home_dir: Some("/K".into()), case_insensitive: true, ..Default::default() };
        assert_eq!(normalize_with_limit("/k/é", &ctx, 100).unwrap(), "~/é");
        let ctx = PathContext { home_dir: Some("/k".into()), case_insensitive: true, ..Default::default() };
        assert_eq!(normalize_with_limit("/K/é", &ctx, 100).unwrap(), "~/é");
        assert_eq!(normalize_with_limit("/k-other/é", &ctx, 100).unwrap(), "/k-other/é");
    }
}
