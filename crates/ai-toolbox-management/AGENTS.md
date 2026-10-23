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

The private Linux ConfigOnly service reads existing JSONB records through shared storage helpers. It remains unavailable through GUI, CLI or transport until full writer admission is closed. Provider settings_config is a JSON string, not a nested JSONB object: preserve its other fields while replacing only config. Common patches retain root and unknown fields. Reuse the sole Codex planner, never add a separate merger or call the full auth/catalog writer for a context-only operation.

Raw records/candidates can contain secrets. They are internal, non-serialized data, not default CLI output. Typed errors and operation records must not include raw TOML, auth, paths or arbitrary database errors. Revision and idempotency are separate: check an existing request before rejecting revision drift caused by that same request.

## Verification and unfinished work

Only synthetic, isolated profiles may be used during development. Tests must prove real binary/DB/file effects and bounded cleanup, not just string generation. Until the complete admission map is connected, no GUI-owner or P0 business closure is claimed. Cold standalone owner lifecycle, Windows support, effect execution, recovery and installed artifacts remain separate acceptance work.

The synthetic owner suite must create its runtime root with explicit 0700 permissions rather than relying on the process umask. Normal actor tests require their own successful libtest result and completion marker; the single deliberate abrupt-exit actor uses a distinct exit code and marker because running Drop or libtest completion would invalidate that fixture. Report these executions separately.

## Private ConfigOnly effect slice

The internal service requires an existing database and an explicit stored absolute runtime root. Owner acquisition precedes shared database initialization, which may migrate or seed: this is writer initialization, never a zero-effect read command. Internal revision inspection also acquires runtime coordination leases and must not be advertised as a read-only CLI operation.

Attaching the private service reuses the existing owner's allocation and the exact shared SQLite connection; it never reacquires a lease, opens a database, seeds or migrates. Attachment cannot retroactively establish owner-before-open. The caller must already satisfy that startup order, and attaching still does not connect the GUI or close the application writer admission map. The shared connection retains an opaque owner lease for all DB clones and closes before releasing it; the retained owner must never point back to the DB state.

Owner, cached path, SQLite's actual main filename, main inode and Linux HAS_MOVED checks run under the same connection mutex as one-time owner retention. Failed validation installs no guard; the same owner allocation is idempotent and a different allocation cannot replace it. Recheck the actual connection inside each DB read/write and before the file effect, rather than trusting attachment forever. Sidecars require safe type, current UID, mode and single-link metadata plus stability within each validation, but their inodes are not pinned across operations: SQLite may legitimately create or remove them. These checks do not audit SQLite's internal sidecar FDs or isolate malicious code using the trusted Rust connection API; Windows remains unfinished.

After owner retention, trusted with_conn_mut callers must not replace, swap or move out the Connection, or let the original connection escape the shared holder. Any future replacement requires a separately designed detach/reattach lifecycle. Reopening the same path and inode can pass file-identity checks without being the original connection or preserving its transaction/hooks; these checks do not prove connection identity, and raw-pointer equality is not an ABA-safe substitute.

Only the two typed context numbers are planned through the existing Codex planner. Master changes and SavedOnly journal insertion share one transaction. A committed ProjectionStarted transition is required before the fixed config.toml effect; replay never authorizes another write. Partial and uncertain outcomes are non-success and are not automatically retried. Explicit new requests require a fresh revision. This is not atomic database-plus-file commit or power-loss recovery.

The file effect uses the held runtime directory FD and fixed config.toml leaf, preserves ordinary mode/uid/gid and checks identity/content before publishing and on readback. It never opens auth.json. ACL/xattr preservation, hostile same-UID race containment and Windows remain unsupported. Failed staging can retain an owned temporary file, initially created as 0600 and later set to the original config mode: do not unlink a pathname after a race or claim zero residual effects. Tests clean only their own isolated directory after all permits and database references are dropped.

SQLite commit-hook veto fixtures test actual transaction commit rejection, not hardware I/O faults or power loss. Interruption checkpoints are controlled returns followed by close/reopen, not process crashes. Keep those evidence levels separate.
