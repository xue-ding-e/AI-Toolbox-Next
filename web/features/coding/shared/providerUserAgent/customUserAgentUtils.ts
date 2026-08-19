/**
 * Provider-level custom User-Agent meta read/merge helpers.
 *
 * Mirrors the billing-config pattern (`billingConfigUtils.ts`): each "feature
 * subset" owns its `get*FromMeta` / `merge*IntoMeta` pair and only touches its
 * own keys, so they can be chained in a form's submit `meta:` builder without
 * clobbering one another. `mergeGatewayProfileReferenceIntoMeta` uses a
 * delete whitelist that does not include `customUserAgent`, so switching a
 * gateway profile preserves the user-entered UA (same treatment as billing).
 */
export interface CustomUserAgentState {
  enabled: boolean;
  value: string;
}

export interface GatewayProviderUserAgentMeta {
  customUserAgent?: string;
}

/** Read the custom User-Agent from provider meta into an editable state. */
export function getCustomUserAgentFromMeta(
  meta?: GatewayProviderUserAgentMeta | null,
): CustomUserAgentState {
  const value = meta?.customUserAgent?.trim() ?? '';
  return {
    enabled: Boolean(value),
    value: meta?.customUserAgent ?? '',
  };
}

/** Merge the custom User-Agent state back into provider meta (field-level). */
export function mergeCustomUserAgentIntoMeta<T extends GatewayProviderUserAgentMeta>(
  meta: T | undefined,
  state: CustomUserAgentState,
): T | undefined {
  const nextMeta = { ...(meta || {}) } as T;
  delete nextMeta.customUserAgent;

  if (state.enabled) {
    const value = state.value.trim();
    if (value) {
      nextMeta.customUserAgent = value;
    }
  }

  return hasMeaningfulMeta(nextMeta) ? nextMeta : undefined;
}

function hasMeaningfulMeta(meta: GatewayProviderUserAgentMeta): boolean {
  return Object.values(meta).some(
    (value) => value !== undefined && value !== null && value !== '',
  );
}
