import type { OpenClawConfig } from '@/types/openclaw';
import { isJsonObject } from '../../../../utils/json.ts';

/**
 * Top-level sections this page renders somewhere other than the "其他配置" box.
 *
 * `mcp` lives in `mcp.servers` of the very same `openclaw.json` and is owned by
 * the MCP page; `models`/`agents` have their own cards. Hiding them keeps the
 * editor text from carrying a stale copy, but it also means the saved payload
 * cannot be built from the editor text alone — see
 * `mergeOpenClawOtherConfigFields`.
 */
export const OPENCLAW_OTHER_CONFIG_HIDDEN_KEYS = ['models', 'agents', 'mcp'] as const;

/** The other-config editor's initial value: every top-level key it owns. */
export const extractOpenClawOtherConfigFields = (
  config: OpenClawConfig | null | undefined,
): OpenClawConfig | undefined => {
  if (!config) return undefined;

  const rest: OpenClawConfig = { ...config };
  OPENCLAW_OTHER_CONFIG_HIDDEN_KEYS.forEach((key) => {
    delete rest[key];
  });

  return Object.keys(rest).length > 0 ? rest : undefined;
};

/**
 * Build the payload of an other-config save.
 *
 * The backend's `apply_root_section_diff` **removes every root section the
 * payload omits**, so the payload has to start from the freshly re-read file
 * (which still holds `mcp.servers` and the other hidden sections) and then let
 * the editor text override only what it owns. Spreading the base first is what
 * keeps a server the MCP page wrote a moment ago from being deleted outright;
 * omitting it is not a mere revert, it is a deletion. A hand-typed hidden key
 * still wins, because the editor value is merged last.
 */
export const mergeOpenClawOtherConfigFields = (
  config: OpenClawConfig,
  value: unknown,
): OpenClawConfig => ({
  ...config,
  ...(isJsonObject(value) ? value : {}),
});
