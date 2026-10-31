# Shared gateway capability query

## Source of Truth

The original desktop `GatewayCliKey` enum, `as_str`, `supported_mvp` and takeover admission predicate live here. Desktop re-exports the enum and delegates both its gate and supported-key query; CLI and MCP call this same query. Preserve `supported_mvp()` **then** `is_supported_cli` filtering and order; a known enum variant or usage collector is not takeover support.

## Boundaries

This crate is pure: no Tauri, environment resolution, runtime probing, configuration, database, filesystem, credentials, owner startup or network access. Querying capability does not engage a CLI or report its installed/running state. Preserve the original serde spelling, including `OpenCode` serializing as `open_code` while `as_str()` returns `opencode`; neither makes it admitted for takeover. No user data belongs in fixtures.

## Minimum validation

Generate Cargo locks with real Cargo before locked tests; never fabricate resolution entries. Run the shared crate's exact order/exclusion/serde regressions, automation unit and actual-process tests, and the desktop `supported_cli_query_delegates_to_shared_takeover_gate` test. Process assertions use synthetic environment and bounded children and do not establish GUI IPC, native desktop interaction, syscall isolation or installed CLI distribution.
