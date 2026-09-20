import assert from 'node:assert/strict';
import test from 'node:test';

import {
  aliasesForSelectedSites,
  defaultAggregateSiteIds,
  resolveAggregateFormSeed,
} from '../../../../../features/coding/shared/gateway/gatewayAggregateDraft.ts';

const candidates = [
  { id: 'site-a', name: 'A' },
  { id: 'site-b', name: 'B' },
];

test('an engaged manifest seeds the form and keeps stale sites visible', () => {
  const seed = resolveAggregateFormSeed({
    activeConfig: {
      provider_ids: ['site-b', 'gone'],
      separator: '|',
      aliases: { 'site-b': 'b', gone: 'g' },
      naming: 'model_at_site',
    },
    draftConfig: { provider_ids: ['site-a'], separator: '.', naming: 'site_model' },
    candidates,
    appliedProviderId: 'site-a',
  });

  // The manifest is what is actually routing: its order is kept as-is, the
  // unaddressable site surfaces as an invalid config instead of being rewritten
  // out of a running takeover, and the engaged config wins over the draft.
  assert.deepEqual(seed, {
    siteIds: ['site-b', 'gone'],
    separator: '|',
    aliases: { 'site-b': 'b', gone: 'g' },
    naming: 'model_at_site',
    crossSiteFailover: false,
    droppedDraftSites: false,
  });
});

test('the engaged manifest seeds the failover policy', () => {
  const seed = resolveAggregateFormSeed({
    activeConfig: {
      provider_ids: ['site-a'],
      separator: '.',
      naming: 'site_model',
      cross_site_failover: true,
    },
    draftConfig: null,
    candidates,
  });

  assert.equal(seed.crossSiteFailover, true);
});

test('a draft carries the failover policy into the form seed', () => {
  const seed = resolveAggregateFormSeed({
    activeConfig: null,
    draftConfig: {
      provider_ids: ['site-a'],
      separator: '.',
      naming: 'site_model',
      cross_site_failover: true,
    },
    candidates,
  });

  assert.equal(seed.crossSiteFailover, true);
});

test('a missing failover policy seeds the safe default', () => {
  // Old drafts and manifests predate the field, so it has to read as "off".
  const seed = resolveAggregateFormSeed({
    activeConfig: null,
    draftConfig: { provider_ids: ['site-a'], separator: '.', naming: 'site_model' },
    candidates,
  });

  assert.equal(seed.crossSiteFailover, false);
});

test('an engaged manifest drops aliases that address unselected sites only', () => {
  const seed = resolveAggregateFormSeed({
    activeConfig: {
      provider_ids: ['site-a'],
      separator: '.',
      aliases: { 'site-b': 'b' },
      naming: 'site_model',
    },
    draftConfig: null,
    candidates,
  });

  assert.deepEqual(seed.aliases, {});
});

test('without a draft the form defaults to the currently applied provider', () => {
  const seed = resolveAggregateFormSeed({
    activeConfig: null,
    draftConfig: null,
    candidates,
    appliedProviderId: 'site-b',
  });

  // Aggregate mode cannot be empty, so a fresh form is never seeded with zero
  // sites while the CLI has an eligible site.
  assert.deepEqual(seed.siteIds, ['site-b']);
  assert.equal(seed.separator, '.');
  assert.equal(seed.naming, 'site_model');
  assert.deepEqual(seed.aliases, {});
  assert.equal(seed.droppedDraftSites, false);
});

test('the default falls back to the first site when the applied provider is not eligible', () => {
  assert.deepEqual(defaultAggregateSiteIds(candidates, 'official-provider'), ['site-a']);
  assert.deepEqual(defaultAggregateSiteIds(candidates, ''), ['site-a']);
  assert.deepEqual(defaultAggregateSiteIds(candidates, '  '), ['site-a']);
  // No proxyable site at all keeps the existing "no sites" empty state.
  assert.deepEqual(defaultAggregateSiteIds([], 'site-a'), []);
});

test('a saved draft wins over the default selection but is re-ordered by the provider list', () => {
  const seed = resolveAggregateFormSeed({
    activeConfig: null,
    draftConfig: {
      provider_ids: ['site-b', 'site-a'],
      separator: '@',
      aliases: { 'site-a': 'a' },
      naming: 'model_only',
    },
    candidates,
    appliedProviderId: 'site-a',
  });

  // The selection survives, but its order is the CLI provider list order: the
  // provider list is the single source of the site priority order.
  assert.deepEqual(seed, {
    siteIds: ['site-a', 'site-b'],
    separator: '@',
    aliases: { 'site-a': 'a' },
    naming: 'model_only',
    crossSiteFailover: false,
    droppedDraftSites: false,
  });
});

test('an engaged manifest order is rewritten to the provider list order', () => {
  const seed = resolveAggregateFormSeed({
    activeConfig: {
      provider_ids: ['site-b', 'site-a'],
      separator: '|',
      aliases: { 'site-b': 'b' },
      naming: 'site_model',
    },
    draftConfig: null,
    candidates,
  });

  assert.deepEqual(seed.siteIds, ['site-a', 'site-b']);
});

test('unavailable draft sites are dropped and reported', () => {
  const seed = resolveAggregateFormSeed({
    activeConfig: null,
    draftConfig: {
      provider_ids: ['site-b', 'deleted'],
      separator: '.',
      aliases: { deleted: 'd' },
      naming: 'site_model',
    },
    candidates,
    appliedProviderId: 'site-a',
  });

  assert.deepEqual(seed.siteIds, ['site-b']);
  assert.deepEqual(seed.aliases, {});
  assert.equal(seed.droppedDraftSites, true);
});

test('a fully unavailable draft falls back to the default selection', () => {
  const seed = resolveAggregateFormSeed({
    activeConfig: null,
    draftConfig: {
      provider_ids: ['deleted'],
      separator: '.',
      aliases: { deleted: 'd' },
      naming: 'site_model',
    },
    candidates,
    appliedProviderId: 'site-a',
  });

  assert.deepEqual(seed.siteIds, ['site-a']);
  assert.deepEqual(seed.aliases, {});
  assert.equal(seed.droppedDraftSites, true);
});

test('stored values that cannot be used fall back instead of reaching the form', () => {
  const seed = resolveAggregateFormSeed({
    activeConfig: null,
    draftConfig: {
      provider_ids: ['site-a'],
      // A separator containing a site-id character cannot be split back apart.
      separator: 'a',
      aliases: { 'site-a': '  ' },
      naming: 'site_model',
    },
    candidates,
  });

  assert.equal(seed.separator, '.');
  assert.deepEqual(seed.aliases, {});
});

test('aliasesForSelectedSites keeps selected non-blank aliases only', () => {
  assert.deepEqual(
    aliasesForSelectedSites({ 'site-a': ' a ', 'site-b': 'b', deleted: 'd' }, ['site-a', 'site-b']),
    { 'site-a': 'a', 'site-b': 'b' },
  );
  assert.deepEqual(aliasesForSelectedSites(null, ['site-a']), {});
});
