export type BillingPricingModelSource = 'inherit' | 'requested' | 'upstream';

export interface BillingConfigState {
  enabled: boolean;
  costMultiplier?: string;
  pricingModelSource: BillingPricingModelSource;
}

export interface GatewayProviderBillingMeta {
  providerType?: string;
  costMultiplier?: string;
  pricingModelSource?: string;
}

export function normalizeBillingPricingModelSource(
  source?: string,
): BillingPricingModelSource {
  const normalizedSource = source?.trim().toLowerCase();
  if (normalizedSource === 'request' || normalizedSource === 'requested') {
    return 'requested';
  }
  if (normalizedSource === 'upstream' || normalizedSource === 'response') {
    return 'upstream';
  }
  return 'inherit';
}

export function getBillingConfigFromMeta(
  meta?: GatewayProviderBillingMeta,
): BillingConfigState {
  const costMultiplier = meta?.costMultiplier?.trim();
  const pricingModelSource = normalizeBillingPricingModelSource(meta?.pricingModelSource);

  return {
    enabled: Boolean(costMultiplier || pricingModelSource !== 'inherit'),
    costMultiplier: costMultiplier || undefined,
    pricingModelSource,
  };
}

export function mergeBillingConfigIntoMeta<T extends GatewayProviderBillingMeta>(
  meta: T | undefined,
  billingConfig: BillingConfigState,
): T | undefined {
  const nextMeta = { ...(meta || {}) } as T;
  delete nextMeta.costMultiplier;
  delete nextMeta.pricingModelSource;

  if (billingConfig.enabled) {
    const costMultiplier = billingConfig.costMultiplier?.trim();
    if (costMultiplier) {
      nextMeta.costMultiplier = costMultiplier;
    }

    if (billingConfig.pricingModelSource !== 'inherit') {
      nextMeta.pricingModelSource = billingConfig.pricingModelSource;
    }
  }

  return hasMeaningfulMeta(nextMeta) ? nextMeta : undefined;
}

function hasMeaningfulMeta(meta: GatewayProviderBillingMeta): boolean {
  return Object.values(meta).some((value) => value !== undefined && value !== null && value !== '');
}
