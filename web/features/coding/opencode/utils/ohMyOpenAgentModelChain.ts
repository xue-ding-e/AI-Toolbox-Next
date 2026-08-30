import type { OhMyOpenAgentAgentConfig } from '@/types/ohMyOpenAgent';

/** A `models` chain entry — either a bare model id string or a per-entry
 *  override object (`{ model, reasoning, ... }`). Upstream
 *  `2026-08-reasoning-unification` emits the primary entry as an object
 *  carrying `reasoning`; older/manual configs may use plain strings. */
export type ModelChainEntry = string | Record<string, unknown>;

/** Coerce a single `models` chain entry into its model id string. Entries may
 *  be plain strings or `{ model, reasoning }` objects (canonical upstream
 *  shape). Returns undefined for non-string / malformed / empty entries so
 *  callers can drop them. Mirrors the `splitModelChain` logic in
 *  `OhMyOpenAgentConfigModal`. */
export function modelChainEntryToId(entry: unknown): string | undefined {
  if (typeof entry === 'string') return entry || undefined;
  if (entry && typeof entry === 'object') {
    const model = (entry as Record<string, unknown>).model;
    if (typeof model === 'string') return model || undefined;
  }
  return undefined;
}

/** Extract the primary model id and fallback count for card display from an
 *  OMO agent config. Handles the canonical `models` chain (primary at index 0,
 *  rest are fallbacks) and the legacy `model` + `fallback_models` shape, so
 *  agents with fallbacks (which no longer carry a top-level `model`) still
 *  show up in the preview. */
export function getAgentModelDisplay(agent: OhMyOpenAgentAgentConfig | undefined): {
  primaryModel?: string;
  fallbackCount: number;
} {
  if (!agent) return { primaryModel: undefined, fallbackCount: 0 };
  const rawModels = (agent as Record<string, unknown>).models;
  if (Array.isArray(rawModels) && rawModels.length > 0) {
    const primaryModel = modelChainEntryToId(rawModels[0]);
    const fallbackCount = (rawModels.slice(1) as unknown[])
      .filter((e) => typeof modelChainEntryToId(e) === 'string')
      .length;
    return { primaryModel, fallbackCount };
  }
  const model = typeof agent.model === 'string' && agent.model ? agent.model : undefined;
  const rawFallback = (agent as Record<string, unknown>).fallback_models;
  let fallbackCount = 0;
  if (Array.isArray(rawFallback)) {
    fallbackCount = rawFallback.filter((e) => typeof e === 'string' && !!e).length;
  } else if (typeof rawFallback === 'string' && rawFallback) {
    fallbackCount = 1;
  }
  return { primaryModel: model, fallbackCount };
}
