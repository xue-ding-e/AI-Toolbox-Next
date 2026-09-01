import type { KimiProvider, KimiProviderFormData, KimiProviderInput } from '@/types/kimi';
// Value import must stay relative: the node:test loader cannot resolve `@/`.
import { KIMI_LOCAL_PROVIDER_ID } from '../../../../types/kimi.ts';

/**
 * Persist decision for the provider form, keyed off the page-level
 * `editingProvider` state instead of form values (the modal does not echo the
 * id back, so keying off `values.id` used to turn every edit into a create).
 */
export type KimiProviderSavePlan =
  | { action: 'adopt_local'; input: KimiProviderInput }
  | { action: 'update'; provider: KimiProvider }
  | { action: 'create'; input: KimiProviderInput };

export function buildKimiProviderSavePlan(
  editingProvider: KimiProvider | null | undefined,
  values: KimiProviderFormData,
  options?: { isCopy?: boolean },
): KimiProviderSavePlan {
  // A cloned provider always creates a new record — even when the source is
  // the `__local__` projection or an applied row, its id must never be reused
  // for update/adopt.
  if (options?.isCopy) {
    return { action: 'create', input: { ...values } };
  }
  if (editingProvider && editingProvider.id === KIMI_LOCAL_PROVIDER_ID) {
    return { action: 'adopt_local', input: { ...values, id: undefined } };
  }
  if (editingProvider) {
    return { action: 'update', provider: { ...editingProvider, ...values } };
  }
  return { action: 'create', input: { ...values } };
}

/**
 * Any save that rewrites the live `config.toml` while the gateway takeover is
 * active requires a direct-restore round trip around the save
 * (`ensure_kimi_gateway_direct` rejects in-takeover direct writes). That covers
 * editing the applied provider and adopting the `__local__` projection — both
 * records are `isApplied`. Unapplied create/update only touch the DB row, so
 * they never re-engage.
 */
export function shouldReengageKimiGatewayOnSave(
  editingProvider: KimiProvider | null | undefined,
  gatewayMode: string | null | undefined,
): boolean {
  const editingAppliedRecord = Boolean(editingProvider?.isApplied);
  return editingAppliedRecord && (gatewayMode === 'single' || gatewayMode === 'failover');
}
