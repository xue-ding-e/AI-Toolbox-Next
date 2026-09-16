/**
 * Site candidates for the settings-page aggregate block.
 *
 * Mirrors the backend candidate rule in
 * `proxy_gateway/runtime/providers.rs` (`provider_from_record`): a site is
 * proxyable only when it is enabled and its category is not `official`.
 * Official subscription providers use CLI-native credentials and are skipped
 * by the gateway, so they must never appear as selectable aggregate sites.
 */

export interface GatewayAggregateProviderLike {
  id: string;
  name: string;
  category?: string | null;
  isDisabled?: boolean | null;
}

export interface GatewayAggregateSiteCandidate {
  id: string;
  name: string;
}

const isOfficialProviderCategory = (category?: string | null) =>
  (category ?? '').trim().toLowerCase() === 'official';

/** Keep provider order: it is the order the backend falls back through. */
export const toAggregateSiteCandidates = <T extends GatewayAggregateProviderLike>(
  providers: readonly T[],
): GatewayAggregateSiteCandidate[] =>
  providers
    .filter((provider) => !provider.isDisabled && !isOfficialProviderCategory(provider.category))
    .map((provider) => ({ id: provider.id, name: provider.name }));

/**
 * Drop site ids that are no longer selectable (deleted/disabled/official) while
 * keeping the user's saved priority order. Returns the surviving ids plus
 * whether anything was dropped, so callers can tell "user picked nothing" from
 * "the saved selection went stale".
 */
export const reconcileAggregateSiteSelection = (
  savedSiteIds: readonly string[],
  candidates: readonly GatewayAggregateSiteCandidate[],
): { siteIds: string[]; droppedStaleSites: boolean } => {
  const available = new Set(candidates.map((candidate) => candidate.id));
  const siteIds: string[] = [];
  const seen = new Set<string>();
  let droppedStaleSites = false;

  for (const siteId of savedSiteIds) {
    if (!available.has(siteId)) {
      droppedStaleSites = true;
      continue;
    }
    if (seen.has(siteId)) {
      continue;
    }
    seen.add(siteId);
    siteIds.push(siteId);
  }

  return { siteIds, droppedStaleSites };
};

/** Move one site one step earlier/later inside the priority order. */
export const moveAggregateSite = (
  siteIds: readonly string[],
  siteId: string,
  direction: 'up' | 'down',
): string[] => {
  const index = siteIds.indexOf(siteId);
  if (index < 0) {
    return [...siteIds];
  }
  const targetIndex = direction === 'up' ? index - 1 : index + 1;
  if (targetIndex < 0 || targetIndex >= siteIds.length) {
    return [...siteIds];
  }
  const next = [...siteIds];
  next[index] = next[targetIndex];
  next[targetIndex] = siteId;
  return next;
};
