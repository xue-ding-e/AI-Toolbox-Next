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
 * keeping their stored relative order. Returns the surviving ids plus whether
 * anything was dropped, so callers can tell "user picked nothing" from "the
 * saved selection went stale". Surviving ids are re-ordered by
 * `orderAggregateSiteIdsByCandidates` before the form is used.
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

/**
 * Canonical site order: the CLI provider list order, i.e. the drag order the
 * Codex page persists as `sort_index` and the order the backend falls back
 * through. Sorting sites is only possible in that provider list, so the
 * aggregate form renders and writes the selection in this order instead of
 * keeping a second, panel-local ordering.
 *
 * Ids that are no longer selectable keep their relative order at the end: the
 * form has to keep naming them so the engage fails loudly as an invalid config
 * instead of silently rewriting a running takeover.
 */
export const orderAggregateSiteIdsByCandidates = (
  siteIds: readonly string[],
  candidates: readonly GatewayAggregateSiteCandidate[],
): string[] => {
  const selected = new Set(siteIds);
  const ordered = candidates
    .filter((candidate) => selected.has(candidate.id))
    .map((candidate) => candidate.id);
  const addressable = new Set(ordered);
  const stale = siteIds.filter((siteId) => !addressable.has(siteId));
  return [...ordered, ...stale];
};

/**
 * Short display handle for a site row.
 *
 * A site id is the CLI provider's database primary key (`db_new_id()`, a UUID v4
 * without hyphens), so it is 32 characters long and unreadable in a dense list.
 * The panel shows only the first 8 characters and keeps the full id in the cell
 * tooltip.
 *
 * Display only: the full id stays the site's identity everywhere it matters —
 * the draft, the aggregate manifest, the routing fallback prefix and the
 * engage/disengage commands. Never shorten what gets written to the backend.
 */
export const shortAggregateSiteId = (siteId: string): string => siteId.slice(0, 8);
