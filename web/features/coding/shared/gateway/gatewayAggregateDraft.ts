import type { GatewayAggregateConfig, GatewayAggregateNamingMode } from '@/services';
import {
  normalizeSubagentExposedModels,
  normalizeGatewayAggregateSiteIds,
  validateGatewayAggregateSeparator,
} from './gatewayAggregateConfig';
import {
  orderAggregateSiteIdsByCandidates,
  reconcileAggregateSiteSelection,
  type GatewayAggregateSiteCandidate,
} from './gatewayAggregateCandidates';

/**
 * Mirrors `DEFAULT_AGGREGATE_SEPARATOR` in the service layer and the backend's
 * `AGGREGATE_DEFAULT_SEPARATOR`. Kept local so this helper imports only relative
 * modules, matching the other shared gateway helpers that the plain-node web
 * test runner has to resolve.
 */
const FALLBACK_AGGREGATE_SEPARATOR = '.';

/**
 * Form seed for the aggregate settings block.
 *
 * The aggregate site selection is configuration the user edits before the mode
 * is engaged, so it has to survive closing the editor: an engaged manifest is
 * the routing truth and is read first, otherwise the persisted draft is used and
 * an empty result falls back to the CLI's currently applied provider. Aggregate
 * mode cannot represent "no site", so the seed is never an empty list while the
 * CLI has at least one eligible site.
 */
export interface GatewayAggregateFormSeed {
  siteIds: string[];
  separator: string;
  aliases: Record<string, string>;
  naming: GatewayAggregateNamingMode;
  /**
   * Whether the takeover may move a request to another selected site when the
   * addressed site fails. A stored value that is absent or `false` means the
   * backend default: the request stays on one site, so a failure never spends
   * another site's balance.
   */
  crossSiteFailover: boolean;
  /**
   * Bare model names the takeover keeps publishing as programmable hidden
   * aliases. An empty list is the backend's "publish every bare model" default,
   * never "publish none".
   */
  subagentExposedModels: string[];
  /**
   * True when the stored draft named sites that are no longer selectable
   * (deleted, disabled or official). The caller surfaces it so a silently
   * shortened selection is never mistaken for the user's saved config.
   */
  droppedDraftSites: boolean;
}

/**
 * Default selection: the CLI's currently applied provider when it is eligible,
 * else the first eligible site. Returns an empty list only when the CLI has no
 * proxyable provider at all, which is the existing "no sites" empty state.
 */
export const defaultAggregateSiteIds = (
  candidates: readonly GatewayAggregateSiteCandidate[],
  appliedProviderId?: string | null,
): string[] => {
  const applied = (appliedProviderId ?? '').trim();
  if (applied && candidates.some((candidate) => candidate.id === applied)) {
    return [applied];
  }
  const first = candidates[0];
  return first ? [first.id] : [];
};

const resolveDraftSeparator = (separator?: string | null): string =>
  separator && validateGatewayAggregateSeparator(separator) === null
    ? separator
    : FALLBACK_AGGREGATE_SEPARATOR;

/** Keep only the aliases that address a site in `siteIds`, dropping blank ones. */
export const aliasesForSelectedSites = (
  aliases: Record<string, string> | null | undefined,
  siteIds: readonly string[],
): Record<string, string> => {
  const selected = new Set(siteIds);
  const result: Record<string, string> = {};
  for (const [siteId, alias] of Object.entries(aliases ?? {})) {
    if (!selected.has(siteId)) continue;
    const trimmed = alias.trim();
    if (!trimmed) continue;
    result[siteId] = trimmed;
  }
  return result;
};

export const resolveAggregateFormSeed = (params: {
  /** Manifest selection; non-null only while aggregate mode is engaged. */
  activeConfig: GatewayAggregateConfig | null;
  /** Last persisted draft; only consulted while aggregate mode is not engaged. */
  draftConfig: GatewayAggregateConfig | null;
  candidates: readonly GatewayAggregateSiteCandidate[];
  appliedProviderId?: string | null;
}): GatewayAggregateFormSeed => {
  const { activeConfig, draftConfig, candidates, appliedProviderId } = params;

  if (activeConfig) {
    // Engaged: the manifest is what is actually routing. Stale ids stay in the
    // form on purpose so they surface as an invalid config instead of being
    // silently rewritten out of a running takeover. Site order is never a
    // panel-local choice: it is read from the CLI provider list.
    const siteIds = orderAggregateSiteIdsByCandidates(
      normalizeGatewayAggregateSiteIds(activeConfig.provider_ids),
      candidates,
    );
    return {
      siteIds,
      separator: resolveDraftSeparator(activeConfig.separator),
      aliases: aliasesForSelectedSites(activeConfig.aliases, siteIds),
      naming: activeConfig.naming ?? 'site_model',
      crossSiteFailover: activeConfig.cross_site_failover === true,
      subagentExposedModels: normalizeSubagentExposedModels(
        activeConfig.subagent_exposed_models,
      ),
      droppedDraftSites: false,
    };
  }

  const reconciled = draftConfig
    ? reconcileAggregateSiteSelection(draftConfig.provider_ids, candidates)
    : { siteIds: [], droppedStaleSites: false };
  const siteIds =
    reconciled.siteIds.length > 0
      ? orderAggregateSiteIdsByCandidates(reconciled.siteIds, candidates)
      : defaultAggregateSiteIds(candidates, appliedProviderId);

  return {
    siteIds,
    separator: resolveDraftSeparator(draftConfig?.separator),
    aliases: aliasesForSelectedSites(draftConfig?.aliases, siteIds),
    naming: draftConfig?.naming ?? 'site_model',
    crossSiteFailover: draftConfig?.cross_site_failover === true,
    subagentExposedModels: normalizeSubagentExposedModels(
      draftConfig?.subagent_exposed_models,
    ),
    // A draft whose sites all disappeared is replaced by the default selection;
    // the caller tells the user instead of silently swapping their selection.
    droppedDraftSites: reconciled.droppedStaleSites,
  };
};
