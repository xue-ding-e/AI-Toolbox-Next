# Shared path service

## Source of Truth

The service in `src/lib.rs` is the shared implementation for desktop path expansion and backup-path normalization. Desktop, standalone CLI and MCP call the same bounded request entry. Existing internal backup helpers retain the legacy unbounded wrapper; that wrapper is not an additional advertised protocol operation.

## Why and boundaries

PathContext separates host home/config directory resolution and the ordered allowlisted USERPROFILE, APPDATA, LOCALAPPDATA and HOME expansion from deterministic transformation. Production context reads those path values only, not the full environment. OS directory resolution may itself read OS account metadata; this is not a zero-syscall guarantee. No target path is opened, created, canonicalized or validated, and no database, owner or GUI is initialized.

Preserve original ordering and token semantics: leading tilde first; Windows references then complete Unix variable tokens in each allowlisted slot. Unknown variables and relative `..` segments remain. Config-relative storage prefers `%APPDATA%` over `~`; supported portable prefixes remain portable. Normalization must never be presented as path-safety validation.

## Gotchas

The protocol request first validates the decoded UTF-8 path field against one shared 32 KiB limit before host context resolution and rejects NUL as invalid path input. CLI/MCP invoke the same validator; JSON escaping does not count toward this business-field bound. The protocol request bounds both intermediate variable expansion and final output to 1 MiB. Check replacement lengths before allocation, even when the eventual portable result would be short. Internal expansion reports PathLimitExceeded; the request boundary maps it to PathRequestError::ExpansionLimitExceeded. Invalid size/NUL is PathRequestError::InvalidInput. Adapters must not turn either rejection into success with the original input. Legacy internal wrappers preserve existing behavior rather than silently imposing a new backup-storage limit.

Windows-style case folding changes UTF-8 byte lengths. Locate a base-prefix separator in the original candidate by folded character lengths, never by indexing with the original base byte length. Scan linearly rather than repeatedly folding path prefixes.

## Minimum validation

Run crate tests with locked dependencies and one build job, then actual standalone CLI/MCP process equivalence in synthetic homes, including HOME/config expansion rather than only the portable-prefix fast path. Keep desktop normalization tests and GUI rejected-normalization coverage. Unit tests use injected context and no host environment mutation. These tests do not establish native GUI acceptance, real backup/restore or whole-product protocol parity.
