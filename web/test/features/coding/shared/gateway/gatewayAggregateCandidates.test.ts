import assert from 'node:assert/strict';
import test from 'node:test';

import {
  orderAggregateSiteIdsByCandidates,
  reconcileAggregateSiteSelection,
  shortAggregateSiteId,
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

test('orderAggregateSiteIdsByCandidates follows the provider list order', () => {
  const candidates = [{ id: 'a', name: 'A' }, { id: 'b', name: 'B' }, { id: 'c', name: 'C' }];

  // A saved priority that differs from the provider list is rewritten to the
  // provider list order: the CLI list stays the only source of site order.
  assert.deepEqual(orderAggregateSiteIdsByCandidates(['c', 'a', 'b'], candidates), [
    'a',
    'b',
    'c',
  ]);
  assert.deepEqual(orderAggregateSiteIdsByCandidates(['b', 'c'], candidates), ['b', 'c']);
  assert.deepEqual(orderAggregateSiteIdsByCandidates([], candidates), []);

  // Sites that are no longer candidates keep their relative order at the end so
  // the form still names them and the engage fails loudly instead of silently
  // rewriting a running takeover.
  assert.deepEqual(orderAggregateSiteIdsByCandidates(['gone-1', 'a', 'gone-2'], candidates), [
    'a',
    'gone-1',
    'gone-2',
  ]);
});

test('shortAggregateSiteId renders a readable eight-character handle', () => {
  // Site ids are 32-character UUIDs, so the panel shows a stable prefix and keeps
  // the full id in the tooltip. Display only: what is written to the backend uses
  // the untouched provider id.
  assert.equal(shortAggregateSiteId('fa0c3840846b4bd2ba5b1a2c3d4e5f60'), 'fa0c3840');
  assert.equal(shortAggregateSiteId('provider-1'), 'provider');
  assert.equal(shortAggregateSiteId('ab'), 'ab');
});
