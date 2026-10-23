# Local management owner boundary

## Source of Truth

This internal crate is an issue #41 candidate, not a complete headless service. It is not connected to desktop startup or CLI business writes yet. The current transport supports a closed owner-status handshake only; never describe unexposed operations as available or use a generic invoke, SQL or shell endpoint.

## Ownership and safety

- A profile identity is an existing canonical directory and physical identity, not an input label, cached path, PID or port number. Client inspection must not create directories, open a database, seed, migrate, import or self-heal configuration.
- The GUI must acquire and retain the profile lease before database initialization and background writers. A permit must outlive all accepted work; socket disconnect is not cancellation or rollback. Epochs correlate lifetimes and are not authentication tokens.
- Linux peer UID and private endpoint checks are the initial platform scope. Windows named-pipe current-user DACL, peer identity and actual native acceptance remain unfinished. Unsupported platforms must fail explicitly, never fall back to TCP or direct database writes.
- Fixed lease files must not be unlinked or replaced. Runtime identity and leases must be shared across different profiles pointing to the same tool root. Missing or insecure paths, changed identity and read-only roots fail closed instead of selecting another lock domain.
- Backup, restore, directory sync and import must exclude coordination artifacts and refuse destructive destination replacement that would remove an existing lease/socket. Exclusion of source files alone does not protect a destination lock inode. These integrations are required before enabling writes.
- All same-resource desktop, tray, background, Gateway, MCP/plugin and recovery writers must participate in a common admission strategy. Preserve lock order and pass explicit permits to nested code; a mutex in one command or one filesystem helper does not close the operation boundary.

## Codex data boundary

Future master-helper integration must read existing JSONB records through shared storage helpers; that helper is not part of this owner-primitives crate yet. Provider settings_config is a JSON string, not a nested JSONB object: preserve its other fields while replacing only config. Common patches retain root and unknown fields. Reuse the sole Codex planner, never add a separate merger or call the full auth/catalog writer for a context-only operation.

Raw records/candidates can contain secrets. They are internal, non-serialized data, not default CLI output. Typed errors and operation records must not include raw TOML, auth, paths or arbitrary database errors. Revision and idempotency are separate: check an existing request before rejecting revision drift caused by that same request.

## Verification and unfinished work

Only synthetic, isolated profiles may be used during development. Tests must prove real binary/DB/file effects and bounded cleanup, not just string generation. Until the complete admission map is connected, no GUI-owner or P0 business closure is claimed. Cold standalone owner lifecycle, Windows support, effect execution, recovery and installed artifacts remain separate acceptance work.

The synthetic owner suite must create its runtime root with explicit 0700 permissions rather than relying on the process umask. Normal actor tests require their own successful libtest result and completion marker; the single deliberate abrupt-exit actor uses a distinct exit code and marker because running Drop or libtest completion would invalidate that fixture. Report these executions separately.
