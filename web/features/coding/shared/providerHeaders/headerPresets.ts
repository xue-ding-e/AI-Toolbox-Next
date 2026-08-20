/**
 * User-Agent presets, surfaced as quick-insert `set` rows in the header
 * override editor.
 *
 * Sourced from cc-switch's `USER_AGENT_PRESETS` (PR #3671), which curl-tested
 * the Kimi Coding Plan (`api.kimi.com/coding`) UA allowlist: `claude-cli/*`,
 * `claude-code/*`, `Kilo-Code/*` pass; `codex-cli`, `kimi-cli` get 403. The
 * allowlist only checks the UA name prefix (not the version), so static values
 * survive Claude Code upgrades.
 *
 * The first entry is the full format the official Claude Code CLI actually
 * sends (see `claude-cli/2.1.2 (external, cli)` detected in `stream_check.rs`)
 * — closest to a real client and most robust under strict UA checks; the rest
 * are shorter variants.
 *
 * These presets target the "non-allowlisted coding agent (Codex/Gemini/Grok/
 * Claude Desktop, etc.) wants to reach a UA-restricted upstream" scenario:
 * disguise forwarded requests as an allowlisted client. Use is the user's
 * explicit choice.
 */
import type { CustomHeaderEntry } from './customHeadersUtils';

export const HEADER_USER_AGENT_PRESETS: readonly string[] = [
  'claude-cli/2.1.161 (external, cli)',
  'claude-cli/2.1.161',
  'claude-code/1.0.0',
  'claude-code/0.1.0',
  'Kilo-Code/1.0',
];

/** Build a `set` row that overrides User-Agent with the given preset value. */
export function userAgentPresetToHeaderEntry(preset: string): CustomHeaderEntry {
  return { op: 'set', name: 'User-Agent', value: preset, from: '', to: '' };
}
