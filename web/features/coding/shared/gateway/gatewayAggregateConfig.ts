import type { GatewayAggregateNamingMode, GatewayCliTakeoverStatus } from '@/services';
import {
  isGatewayReengageMode,
  type GatewayAggregateReengageConfig,
} from './providerSaveReengage';

/**
 * Aggregate-mode helpers shared by the gateway settings panel and the provider
 * save/re-engage flow.
 *
 * Backend contract (mirrors `cli_proxy/manifest.rs`): a site id must match
 * `^[A-Za-z0-9_-]+$`, and the separator must be non-empty and must not contain
 * letters, digits, `_` or `-`, otherwise `<site_id><sep><model>` cannot be
 * split back into its parts. Keep this module free of i18n text so the callers
 * decide how to phrase the error.
 */

export type GatewayAggregateSeparatorInvalidReason = 'empty' | 'reservedCharacters';

const SITE_ID_PATTERN = /^[A-Za-z0-9_-]+$/;
const ALIAS_PATTERN = /^[A-Za-z0-9_-]+$/;
export const AGGREGATE_ALIAS_MAX_LENGTH = 32;

/** Site ids the backend can address; anything else must be dropped before engaging. */
export const isAggregateSiteId = (siteId: string): boolean =>
  SITE_ID_PATTERN.test(siteId.trim());

/**
 * Validate a user-supplied separator. Returns `null` when the separator is
 * usable; the caller maps the reason code to a localized message.
 */
export const validateGatewayAggregateSeparator = (
  separator: string,
): GatewayAggregateSeparatorInvalidReason | null => {
  if (separator.length === 0) {
    return 'empty';
  }
  if (/[A-Za-z0-9_-]/.test(separator)) {
    return 'reservedCharacters';
  }
  return null;
};

export const validateGatewayAggregateAlias = (alias: string): boolean =>
  alias.length > 0 && alias.length <= AGGREGATE_ALIAS_MAX_LENGTH && ALIAS_PATTERN.test(alias);

export const normalizeGatewayAggregateAliases = (
  aliases: Record<string, string> | null | undefined,
  selectedSiteIds: readonly string[],
  allSiteIds: readonly string[] = selectedSiteIds,
): Record<string, string> | null => {
  const selected = new Set(selectedSiteIds);
  const normalized: Record<string, string> = {};
  for (const [siteId, rawAlias] of Object.entries(aliases ?? {})) {
    const alias = rawAlias.trim();
    if (!alias) continue;
    if (!selected.has(siteId) || !validateGatewayAggregateAlias(alias)) return null;
    normalized[siteId] = alias;
  }
  // Every candidate must answer to exactly one prefix, mirroring the backend's
  // `validate_aggregate_site_prefixes`: a site is addressed by its alias when it
  // has one, otherwise by its provider id. Unselected candidates always keep
  // their id (request-time routing leaves them addressable as fallbacks), so an
  // alias may not shadow one of those either — the form must not submit a
  // selection the engage command would refuse.
  const prefixes = new Set<string>();
  for (const siteId of allSiteIds) {
    const prefix = (selected.has(siteId) ? normalized[siteId] : undefined) ?? siteId;
    const key = prefix.toLowerCase();
    if (prefixes.has(key)) return null;
    prefixes.add(key);
  }
  return normalized;
};

/** Drop duplicate/non-addressable site ids while preserving the user's order. */
export const normalizeGatewayAggregateSiteIds = (siteIds: readonly string[]): string[] => {
  const seen = new Set<string>();
  const normalized: string[] = [];
  for (const siteId of siteIds) {
    const trimmed = siteId.trim();
    if (!isAggregateSiteId(trimmed) || seen.has(trimmed)) {
      continue;
    }
    seen.add(trimmed);
    normalized.push(trimmed);
  }
  return normalized;
};

/**
 * Read the aggregate selection that must be replayed when re-engaging.
 *
 * Returns `null` when the backend did not expose aggregate details, so callers
 * can skip the aggregate round trip instead of silently re-engaging with an
 * empty site list (which would drop the whole cross-site model list).
 */
export const toGatewayAggregateReengageConfig = (
  status?: GatewayCliTakeoverStatus | null,
): GatewayAggregateReengageConfig | null => {
  if (status?.mode !== 'aggregate') {
    return null;
  }
  const aggregate = status.aggregate ?? null;
  if (!aggregate) {
    return null;
  }
  const providerIds = normalizeGatewayAggregateSiteIds(aggregate.provider_ids);
  const separator = aggregate.separator ?? '';
  if (providerIds.length === 0 || validateGatewayAggregateSeparator(separator) !== null) {
    return null;
  }
  const naming: GatewayAggregateNamingMode = aggregate.naming ?? 'site_model';
  if (!['site_model', 'model_at_site', 'model_only'].includes(naming)) {
    return null;
  }
  const aliases = normalizeGatewayAggregateAliases(aggregate.aliases, providerIds);
  if (!aliases) {
    return null;
  }
  return { providerIds, separator, aliases, naming };
};

/**
 * Decide which takeover mode a provider save must replay around itself.
 *
 * `single` and `failover` behave exactly as before. `aggregate` is only
 * replayable when its selection is available, so a missing/incomplete status
 * degrades to "no re-engage" instead of engaging a mode with no sites.
 */
export const resolveGatewayReengageMode = (
  status?: GatewayCliTakeoverStatus | null,
): 'single' | 'failover' | 'aggregate' | null => {
  const mode = status?.mode ?? null;
  if (!isGatewayReengageMode(mode)) {
    return null;
  }
  if (mode === 'aggregate' && !toGatewayAggregateReengageConfig(status)) {
    return null;
  }
  return mode;
};

/** Label of the model list entry Codex sees for one (site, model) pair. */
export const buildGatewayAggregateModelSlug = (
  siteId: string,
  modelId: string,
  separator: string,
  naming: GatewayAggregateNamingMode = 'site_model',
): string => {
  if (naming === 'model_only') return modelId;
  return naming === 'model_at_site'
    ? `${modelId}${separator}${siteId}`
    : `${siteId}${separator}${modelId}`;
};

/** Preview slug shown for one selected site in the aggregate takeover dialog. */
export const buildGatewayAggregateSitePreviewSlug = (
  siteId: string,
  separator: string,
  naming: GatewayAggregateNamingMode = 'site_model',
  aliases?: Record<string, string> | null,
): string => {
  const alias = aliases?.[siteId]?.trim();
  return buildGatewayAggregateModelSlug(alias || siteId, '<model>', separator, naming);
};
