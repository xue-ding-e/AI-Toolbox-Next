import assert from 'node:assert/strict';
import test from 'node:test';

import {
  moveAggregateSite,
  reconcileAggregateSiteSelection,
  toAggregateSiteCandidates,
} from '../../../../../features/coding/shared/gateway/gatewayAggregateCandidates.ts';

const providers = [
  { id: 'enabled-third-party', name: 'Enabled', category: 'third_party' },
  { id: 'official', name: 'Official', category: 'official' },
  { id: 'disabled', name: 'Disabled', category: 'custom', isDisabled: true },
  { id: 'enabled-custom', name: 'Custom', category: 'custom' },
  { id: 'no-category', name: 'No category' },
  { id: 'official-upper', name: 'Official upper', category: ' OFFICIAL ' },
];

test('site candidates keep only proxyable providers in provider order', () => {
  assert.deepEqual(toAggregateSiteCandidates(providers), [
    { id: 'enabled-third-party', name: 'Enabled' },
    { id: 'enabled-custom', name: 'Custom' },
    { id: 'no-category', name: 'No category' },
  ]);
});

test('reconcile keeps saved priority order and reports dropped stale sites', () => {
  const candidates = [{ id: 'a', name: 'A' }, { id: 'b', name: 'B' }];

  assert.deepEqual(reconcileAggregateSiteSelection(['b', 'a'], candidates), {
    siteIds: ['b', 'a'],
    droppedStaleSites: false,
  });

  // A now-disabled/deleted site must not stay selected and silently keep
  // routing traffic, but the surviving order must be preserved.
  assert.deepEqual(reconcileAggregateSiteSelection(['b', 'gone', 'a'], candidates), {
    siteIds: ['b', 'a'],
    droppedStaleSites: true,
  });

  assert.deepEqual(reconcileAggregateSiteSelection(['a', 'a'], candidates), {
    siteIds: ['a'],
    droppedStaleSites: false,
  });
});

test('moveAggregateSite swaps adjacent entries and clamps at the edges', () => {
  assert.deepEqual(moveAggregateSite(['a', 'b', 'c'], 'b', 'up'), ['b', 'a', 'c']);
  assert.deepEqual(moveAggregateSite(['a', 'b', 'c'], 'b', 'down'), ['a', 'c', 'b']);
  assert.deepEqual(moveAggregateSite(['a', 'b', 'c'], 'a', 'up'), ['a', 'b', 'c']);
  assert.deepEqual(moveAggregateSite(['a', 'b', 'c'], 'c', 'down'), ['a', 'b', 'c']);
  assert.deepEqual(moveAggregateSite(['a', 'b'], 'missing', 'up'), ['a', 'b']);
  assert.deepEqual(moveAggregateSite([], 'a', 'up'), []);
});
