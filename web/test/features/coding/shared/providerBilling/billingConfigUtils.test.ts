import assert from 'node:assert/strict';
import test from 'node:test';

import {
  getBillingConfigFromMeta,
  mergeBillingConfigIntoMeta,
  normalizeBillingPricingModelSource,
} from '../../../../../features/coding/shared/providerBilling/billingConfigUtils.ts';

test('normalizeBillingPricingModelSource accepts UI and legacy aliases', () => {
  assert.equal(normalizeBillingPricingModelSource('requested'), 'requested');
  assert.equal(normalizeBillingPricingModelSource('request'), 'requested');
  assert.equal(normalizeBillingPricingModelSource('upstream'), 'upstream');
  assert.equal(normalizeBillingPricingModelSource('response'), 'upstream');
  assert.equal(normalizeBillingPricingModelSource(undefined), 'inherit');
});

test('getBillingConfigFromMeta enables provider billing when meta has custom values', () => {
  assert.deepEqual(getBillingConfigFromMeta({
    costMultiplier: ' 1.50 ',
    pricingModelSource: 'request',
  }), {
    enabled: true,
    costMultiplier: '1.50',
    pricingModelSource: 'requested',
  });

  assert.deepEqual(getBillingConfigFromMeta(undefined), {
    enabled: false,
    costMultiplier: undefined,
    pricingModelSource: 'inherit',
  });
});

test('mergeBillingConfigIntoMeta clears billing fields when disabled', () => {
  assert.deepEqual(mergeBillingConfigIntoMeta({
    providerType: 'anthropic',
    costMultiplier: '1.5',
    pricingModelSource: 'upstream',
  }, {
    enabled: false,
    pricingModelSource: 'inherit',
  }), {
    providerType: 'anthropic',
  });
});

test('mergeBillingConfigIntoMeta omits inherit pricing source when enabled', () => {
  assert.deepEqual(mergeBillingConfigIntoMeta(undefined, {
    enabled: true,
    costMultiplier: ' 2 ',
    pricingModelSource: 'inherit',
  }), {
    costMultiplier: '2',
  });
});
