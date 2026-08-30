# Official Account Auth Refresh (shared)

## One-line role

- Schedules **startup** and **interval** OAuth token freshness passes for official CLI accounts (Grok / Codex / Gemini CLI / Kimi).
- Does **not** own OAuth HTTP, auth file schemas, or quota/limits APIs.

## Source of Truth

- Per-tool lead windows and refresh HTTP live in each tool's `official_accounts.rs`.
- This module only decides **when** to call each tool's `refresh_applied_*_accounts_if_needed`.
- On-demand paths (apply / refresh limits / force refresh) still call tool-local `ensure_fresh` directly and do not go through this loop.

## Why

- Grok access tokens last hours; Codex lasts much longer; Gemini is short-lived with a 5-minute lead. One shared loop avoids N independent `spawn`s and makes “startup first pass” a first-class event.
- Token refresh ≠ quota refresh. Do not schedule `wham/usage` or Grok billing here.

## Pass model

1. **Startup pass** (after global ~90s delay): every registered tool with `run_on_startup` runs once.
2. **Interval pass**: tools with `interval` run when due; tick granularity is 60s; per-tool `in_flight` prevents re-entry.
3. Candidates are **persisted OAuth accounts** (each tool filters; virtual `__local__` excluded). Applied and non-applied both refresh tokens into SQLite; only applied accounts rewrite live auth files.
4. Interval timing is **after the last completed pass** (startup resets the interval clock). Example: Grok startup at T+90s, next interval ≈ T+90s+15m — not every 15m from process start.

### Defaults

| Tool | Startup | Interval | Notes |
|------|---------|----------|-------|
| Grok | yes | 15m | Lead 30m inside Grok ensure_fresh |
| Gemini CLI | yes | 15m | Lead 5m inside Gemini ensure_fresh |
| Codex | yes | 12h | Lead 3d inside Codex ensure_fresh |
| Kimi | yes | 15m | Lead inside Kimi ensure_fresh |

## Gotchas

- Do not put billing/limits/model-catalog refresh into this module.
- Do not share one OAuth lock across tools; locks stay tool-local.
- Startup is still `ensure_fresh(force=false)`: not near expiry ⇒ no HTTP refresh.
- Failures are isolated per account / per tool (debug log only).

## Minimal verification

- `cargo test --lib coding::auth_refresh`
- With a persisted Grok OAuth account near expiry (applied or not): after app start + ~90s, token expiry/last_refresh update without clicking UI. Non-applied must not rewrite live auth.
