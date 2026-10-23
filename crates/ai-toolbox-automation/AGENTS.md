# Automation foundation

## Source of Truth

The closed `Command`/`Operation` enums and `execute` in `src/lib.rs` define the supported contract. `src/cli.rs` parses arguments and dispatches only after complete validation. This crate is deliberately independent of Tauri and has no build scripts, state bootstrap, or GUI initialization. Linux owner inspection reuses the management client, linking its storage dependencies without calling any storage initializer. Keep the Windows console subsystem in release builds.

## Why and boundaries

This is an issue #41 console foundation, not P0 or business coverage. `codex status` is a reserved typed operation returning `NOT_SUPPORTED`; it must not be counted as an implemented business operation. Capabilities explicitly reports no business coverage and no MCP support. `daemon status` requires explicit absolute existing profile and private endpoint directories and is Linux-only. It uses `ProfileIdentity::existing`, `RuntimeDirectory::existing`, and the normal `OwnerClient` handshake; it never creates a path/lease, opens a DB, acquires ownership, starts a daemon/GUI, or falls back to standalone storage. Only the held profile lease is reported, not database readiness or complete writer admission. There is no Codex configuration workflow or desktop adapter yet.

Future business implementations must extract and reuse existing service semantics, including path resolution and merge/application behavior. Do not add a second configuration merger, direct SQL bypass, arbitrary string invoke endpoint, or mocked `AppHandle` as a production headless adapter.

## Wire and process boundaries

Schema version 1 is separate from the automation package and desktop versions. Breaking wire changes require a new schema version. Envelope `request_id` is explicitly null: there is no persistent request identity, idempotency, or job semantics. The owner wire uses a nonzero identifier only within its single-request connection; reusing it on another connection has no replay or deduplication guarantee.

Result success determines both `ok` and exit status. JSON mode writes exactly one result to stdout, including errors; text errors use stderr. Parse diagnostics never echo raw arguments. The output-only JSON string encoder must escape every control character, quote and backslash while preserving Unicode. No input JSON parser is provided.

Keep existing default text output and `--format text|json` behavior. Discovery, invalid inputs, duplicate options and relative owner paths finish without filesystem inspection. Owner failures map to static diagnostics, never raw paths, tokens, protocol payloads, or operating-system error strings. The synchronous console owns an I/O-and-time-enabled runtime only for owner-status dispatch; do not call this adapter inside an async GUI chain.

## Minimum validation

Run `cargo test --offline --manifest-path crates/ai-toolbox-automation/Cargo.toml --jobs 1` from repository root, with an isolated target directory. Integration tests launch the real binary with a cleared environment, no display variables, isolated empty HOME and non-existent data/cache/tool paths. Each child has a five-second observation deadline, with kill/reap on timeout or observation failure. This harness is limited to the foundation’s short output and absence of descendants: it is not a general process-tree watchdog, and draining inherited pipes after exit is not independently bounded. Fixture cleanup removes only empty directories, retaining unexpected state for diagnosis.

Linux process tests do not establish Windows console behavior, macOS behavior, installed artifact distribution, GUI coexistence, filesystem-read isolation at the OS boundary, or network syscall tracing. Those remain separate acceptance work.

`tests/owner_console.rs` is the Linux actual-console-to-actual-OwnerServer slice. It uses only synthetic 0700 temporary roots on a filesystem admitted by management; an unsuitable TMPDIR must fail rather than silently select a real profile. Its regular-file child output avoids inherited-pipe drains; every child has a deadline and bounded kill/reap cleanup. The DB inotify watch observes only the fixture DB, not arbitrary filesystem reads. Missing paths, rejected protocol/profile identity, unavailable owners, existing lease identity, discovery and parse errors must remain fail-closed. Do not enable business writes based on these read-only checks.
