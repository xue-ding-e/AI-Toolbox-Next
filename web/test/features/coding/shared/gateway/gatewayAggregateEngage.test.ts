import assert from 'node:assert/strict';
import test from 'node:test';

import {
  aggregateEngageErrorNoticeKey,
  aggregateEngageRequiresDirectRestore,
} from '../../../../../features/coding/shared/gateway/gatewayAggregateEngage.ts';

const takeover = (
  mode: string | null,
  primaryProviderId: string | null,
) =>
  ({ mode, primary_provider_id: primaryProviderId }) as Parameters<
    typeof aggregateEngageRequiresDirectRestore
  >[1];

test('a direct CLI never needs the restore round trip', () => {
  assert.equal(aggregateEngageRequiresDirectRestore(['a'], takeover(null, null)), false);
  assert.equal(aggregateEngageRequiresDirectRestore(['a'], null), false);
});

test('an engage that keeps the enabled primary engages directly', () => {
  // The draft/seed order matched the running takeover, which is the common case.
  assert.equal(aggregateEngageRequiresDirectRestore(['a', 'b'], takeover('single', 'a')), false);
  assert.equal(
    aggregateEngageRequiresDirectRestore(['a', 'b'], takeover('aggregate', 'a')),
    false,
  );
});

test('an engage that would change the enabled primary restores direct first', () => {
  // The provider list order moved another site to the front: engaging straight
  // away would be rejected by the backend primary-switch guard.
  assert.equal(aggregateEngageRequiresDirectRestore(['b', 'a'], takeover('single', 'a')), true);
  assert.equal(aggregateEngageRequiresDirectRestore(['b'], takeover('failover', 'a')), true);
  assert.equal(
    aggregateEngageRequiresDirectRestore(['b', 'a'], takeover('aggregate', 'a')),
    true,
  );
  // Deselecting the primary site promotes the next one.
  assert.equal(aggregateEngageRequiresDirectRestore(['b'], takeover('aggregate', 'a')), true);
});

test('aggregateEngageErrorNoticeKey maps only the primary-switch guard', () => {
  assert.equal(
    aggregateEngageErrorNoticeKey(
      'Restore direct mode before switching the primary Gateway proxy provider',
    ),
    'gateway.aggregate.notice.primarySwitchRequiresDirect',
  );
  // Every other backend failure keeps its own text instead of a generic hint.
  assert.equal(aggregateEngageErrorNoticeKey('Site selection is empty'), null);
  assert.equal(aggregateEngageErrorNoticeKey(''), null);
});
