import type { GatewayAggregateNamingMode, GatewayCliTakeoverStatus } from '@/services';
import {
  isGatewayReengageMode,
  type GatewayAggregateReengageConfig,
} from './providerSaveReengage';

/**
 * Aggregate-mode helpers shared by the gateway settings panel and the provider
 * save/re-engage flow.
 *
 * Backend contract (mirrors `cli_proxy/manifest.rs` and `aggregate_naming.rs`):
 * a provider id matches `^[A-Za-z0-9_-]+$`, a *prefix token* additionally
 * allows any non-ASCII character (CJK site names are a first-class case) and
 * must not contain the separator, whitespace or control characters. The
 * separator must be non-empty and must not contain letters, digits, `_` or `-`,
 * otherwise `<site_id><sep><model>` cannot be split back into its parts. Keep
 * this module free of i18n text so the callers decide how to phrase the error.
 */

export type GatewayAggregateSeparatorInvalidReason = 'empty' | 'reservedCharacters';

const SITE_ID_PATTERN = /^[A-Za-z0-9_-]+$/;
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

/**
 * Mirror of the backend `validate_aggregate_alias`.
 *
 * Only *structural* characters are refused — whitespace, control characters
 * and the configured separator. Every other character (including CJK) is
 * allowed, because the default prefix is the user's own site name. Passing an
 * empty separator skips the separator check (the field is optional in preview
 * call sites).
 */
export const validateGatewayAggregateAlias = (alias: string, separator = ''): boolean => {
  if (alias.length === 0 || alias.length > AGGREGATE_ALIAS_MAX_LENGTH) return false;
  if (/\s/.test(alias)) return false;
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u001f\u007f]/.test(alias)) return false;
  if (separator.length > 0 && alias.includes(separator)) return false;
  return true;
};

/**
 * Mirror of the backend `derive_site_prefix_from_name`.
 *
 * The aggregate catalog defaults a site's prefix to its display name so the
 * model list reads `思源888 pro · gpt-5.6-luna` instead of an opaque provider
 * id. Names are free-form, so normalise: strip control characters, fold
 * whitespace runs to `-`, replace the separator with `-`, and trim dashes.
 * Returns `null` when nothing usable is left (caller keeps the provider id).
 */
export const deriveGatewayAggregateSitePrefix = (
  name: string,
  separator: string,
): string | null => {
  const replaced = separator.length > 0 ? name.split(separator).join('-') : name;
  const out: string[] = [];
  let pendingDash = false;
  for (const ch of replaced.trim()) {
    // Whitespace is checked before the control check: tab/newline are both, and
    // folding them into '-' reads far better than deleting them.
    if (/\s/.test(ch)) {
      pendingDash = out.length > 0;
      continue;
    }
    // eslint-disable-next-line no-control-regex
    if (/[\u0000-\u001f\u007f]/.test(ch)) continue;
    if (pendingDash) {
      out.push('-');
      pendingDash = false;
    }
    out.push(ch);
  }
  const prefix = out.join('').replace(/^-+/, '').replace(/-+$/, '');
  return prefix.length > 0 ? prefix : null;
};

export const normalizeGatewayAggregateAliases = (
  aliases: Record<string, string> | null | undefined,
  selectedSiteIds: readonly string[],
  allSiteIds: readonly string[] = selectedSiteIds,
  separator = '',
): Record<string, string> | null => {
  const selected = new Set(selectedSiteIds);
  const normalized: Record<string, string> = {};
  for (const [siteId, rawAlias] of Object.entries(aliases ?? {})) {
    const alias = rawAlias.trim();
    if (!alias) continue;
    if (!selected.has(siteId) || !validateGatewayAggregateAlias(alias, separator)) return null;
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
 * Mirror of the backend `resolve_effective_site_aliases`.
 *
 * Explicit aliases win unchanged. A selected site without one falls back to its
 * normalised display name, unless that derived prefix would steal another
 * site's address (another provider id, or an explicit alias): those cases keep
 * the provider id. The backend applies exactly the same rule, so previews and
 * the engaged catalog cannot drift apart.
 */
export const resolveGatewayAggregateEffectiveAliases = (
  explicit: Record<string, string> | null | undefined,
  selectedSites: readonly { id: string; name: string }[],
  allSiteIds: readonly string[],
  separator: string,
): Record<string, string> => {
  const effective: Record<string, string> = { ...(explicit ?? {}) };
  const taken = new Set<string>();
  for (const id of allSiteIds) taken.add(id.toLowerCase());
  for (const alias of Object.values(effective)) taken.add(alias.toLowerCase());

  for (const site of selectedSites) {
    if (effective[site.id]) continue;
    const prefix = deriveGatewayAggregateSitePrefix(site.name, separator);
    if (!prefix || prefix === site.id) continue;
    if (prefix.length > AGGREGATE_ALIAS_MAX_LENGTH) continue;
    if (!validateGatewayAggregateAlias(prefix, separator)) continue;
    const key = prefix.toLowerCase();
    if (taken.has(key)) continue;
    taken.add(key);
    effective[site.id] = prefix;
  }
  return effective;
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
  const aliases = normalizeGatewayAggregateAliases(
    aggregate.aliases,
    providerIds,
    providerIds,
    separator,
  );
  if (!aliases) {
    return null;
  }
  // Carry the managed Codex `[agents]` defaults through the re-engage round
  // trip. Restoring direct drops them, so without this a provider save would
  // silently unset the user's subagent default.
  const subagentModel = aggregate.subagent?.model?.trim();
  const subagentReasoningEffort = aggregate.subagent?.reasoning_effort?.trim();
  return {
    providerIds,
    separator,
    aliases,
    naming,
    // Only present when the takeover actually manages the key: an explicit
    // `undefined` would read as "managed but blank" to callers that inspect the
    // object's own keys.
    ...(subagentModel ? { subagentModel } : {}),
    ...(subagentReasoningEffort ? { subagentReasoningEffort } : {}),
  };
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
