# Automation foundation

## Source of Truth

The closed `Operation` enum and `execute` in `src/lib.rs` define the supported contract. `src/cli.rs` only parses arguments. This crate is deliberately independent of Tauri and has no dependencies, build scripts, state bootstrap, network, or GUI initialization. Keep the Windows console subsystem in release builds.

## Why and boundaries

This is an issue #41 console foundation, not P0 or business coverage. `codex status` is a reserved typed operation returning `NOT_SUPPORTED`; it must not be counted as an implemented business operation. Capabilities explicitly reports no business coverage and no MCP support. There is no daemon, owner locking, profile/path resolution, Codex configuration workflow, or desktop adapter yet.

Future business implementations must extract and reuse existing service semantics, including path resolution and merge/application behavior. Do not add a second configuration merger, direct SQL bypass, arbitrary string invoke endpoint, or mocked `AppHandle` as a production headless adapter.

## Wire and process boundaries

Schema version 1 is separate from the automation package and desktop versions. Breaking wire changes require a new schema version. `request_id` is explicitly null: the foundation has no request correlation, persistence, idempotency, or job semantics. Do not describe it as providing those guarantees.

Result success determines both `ok` and exit status. JSON mode writes exactly one result to stdout, including errors; text errors use stderr. Parse diagnostics never echo raw arguments. The output-only JSON string encoder must escape every control character, quote and backslash while preserving Unicode. No input JSON parser is provided.

## Minimum validation

Run `cargo test --offline --manifest-path crates/ai-toolbox-automation/Cargo.toml --jobs 1` from repository root, with an isolated target directory. Integration tests launch the real binary with a cleared environment, no display variables, isolated empty HOME and non-existent data/cache/tool paths. Each child has a five-second observation deadline, with kill/reap on timeout or observation failure. This harness is limited to the foundation’s short output and absence of descendants: it is not a general process-tree watchdog, and draining inherited pipes after exit is not independently bounded. Fixture cleanup removes only empty directories, retaining unexpected state for diagnosis.

Linux process tests do not establish Windows console behavior, macOS behavior, installed artifact distribution, GUI coexistence, filesystem-read isolation at the OS boundary, or network syscall tracing. Those remain separate acceptance work.
