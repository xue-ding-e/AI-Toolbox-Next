import type { GatewayCliTakeoverStatus } from '@/services';

/**
 * Does this engage have to restore direct first?
 *
 * The backend refuses to change `primary_provider_id` while a manifest is
 * enabled (`prepare_manifest` in `cli_proxy/mod.rs`), because the supported way
 * to switch the primary is the *restore direct -> engage again* round trip the
 * provider list already uses for single/failover takeovers
 * (`proxy_gateway_switch_primary_provider` -> `apply_or_switch_provider`).
 *
 * Aggregate mode names the first selected site as the primary, so an engage
 * whose first site differs from the enabled manifest's primary hits exactly that
 * guard. Detecting it here keeps the raw backend error out of the user's way and
 * turns it into the round trip instead of a failure.
 *
 * `takeover.mode` is non-null exactly while a manifest is enabled (single,
 * failover or aggregate), so a CLI in direct mode never needs the round trip.
 */
export const aggregateEngageRequiresDirectRestore = (
  nextSiteIds: readonly string[],
  takeover: Pick<GatewayCliTakeoverStatus, 'mode' | 'primary_provider_id'> | null | undefined,
): boolean => {
  if (!takeover?.mode || !takeover.primary_provider_id) {
    return false;
  }
  return nextSiteIds[0] !== takeover.primary_provider_id;
};

/**
 * Localized notice key for a failed aggregate engage.
 *
 * The panel normally pre-empts the primary-switch guard by restoring direct
 * first (`aggregateEngageRequiresDirectRestore`); this only fires when the
 * guard is still reached, for example when the panel's status was stale. The
 * backend answers that case with an English rule sentence, which must not be
 * pasted into a localized notice, so it is replaced by a translated hint.
 *
 * Returns `null` for every other failure so callers keep showing the backend
 * detail instead of hiding an unexplained error.
 */
export const aggregateEngageErrorNoticeKey = (message: string): string | null =>
  message.includes('Restore direct mode before switching the primary')
    ? 'gateway.aggregate.notice.primarySwitchRequiresDirect'
    : null;
