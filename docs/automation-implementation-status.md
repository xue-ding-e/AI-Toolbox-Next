# Management CLI and MCP implementation status

Tracking: [issue #41](https://github.com/xue-ding-e/AI-Toolbox-Next/issues/41).

This is a staged implementation, not a completed P0 or a shipped CLI/MCP product.
The management CLI operates AI Toolbox; it is not a replacement for the Codex CLI.
No installation, release, real-account request, or real-data migration is part of
this foundation.

## Current foundation

- A separate console binary exposes help, version, and static capabilities without
  initializing the desktop application. Reserved business commands fail explicitly.
- A generated inventory records all currently registered desktop handler paths and
  their source hash. It is not a list of implemented or safe public operations.

`operations:check` detects inventory drift. `operations:complete` deliberately
fails while semantic classification and applicable business coverage are missing.
A successful `NOT_SUPPORTED` test does not count as implemented business coverage.

The shared Codex merger and typed planning are separate candidates, not part of
this console/inventory foundation.

## P0 acceptance still required

1. Share resolved profile paths, storage, operation coordination, event dispatch,
   and task ownership with the GUI without constructing a fake AppHandle.
2. Provide same-user, profile-aware owner/IPC on Unix and Windows, including GUI
   first and daemon first startup, stale owners, concurrency, and safe shutdown.
3. Provide existing-readonly storage access that does not create, migrate, seed,
   repair, or silently redirect an invalid explicit data directory.
4. Implement Codex status and redacted provider/account metadata, common/provider/
   effective configuration reads, and explain through shared business services.
5. Connect typed planning to revision/idempotency-aware save, live-file projection,
   parsed verification, synchronization, and reload state. Ordinary config patches
   must not rewrite authentication or change account/gateway ownership.
6. Exercise the requested 1,000,000 / 900,000 override in a real isolated database
   and runtime file, reopen the process, reapply providers, switch synthetic
   official accounts, and verify unknown/runtime-owned content remains intact.
7. Exercise conflicts, invalid inputs, busy/read-only storage, partial projection
   failure and recovery, and gateway restore/apply/re-engage failure paths.
8. Test the real console binary on supported platforms. Linux foundation process
   tests do not establish Windows/macOS packaging or GUI-owner integration.

Pure plans must never report saved, applied, verified, synchronized, or client
reloaded. Saving master data alone is not a successful effective-config update.

## Later acceptance remains open

- P1: classify handler, frontend service, tray, and background entry points into
  typed business operations with schemas, effects, platforms, tests, and justified
  exclusions; implement applicable domains and long-running task lifecycle.
- P2: expose the same operation registry through a real MCP stdio server and test
  protocol negotiation, discovery/pagination, errors, progress/cancellation, and
  equivalence with a real SDK client. Do not shell out to parse human CLI output.
- P3: finish schema and shell/AI documentation, platform artifacts, installation
  guidance, and validation. Release or system installation requires separate scope.
