import assert from 'node:assert/strict';
import test from 'node:test';

import type { GatewayCliTakeoverStatus } from '../../../../../services/proxyGatewayApi.ts';
import {
  buildGatewayAggregateModelSlug,
  buildGatewayAggregateSitePreviewSlug,
  deriveGatewayAggregateSitePrefix,
  isAggregateSiteId,
  normalizeGatewayAggregateAliases,
  normalizeGatewayAggregateSiteIds,
  resolveGatewayAggregateEffectiveAliases,
  resolveGatewayReengageMode,
  toGatewayAggregateReengageConfig,
  validateGatewayAggregateAlias,
  validateGatewayAggregateSeparator,
} from '../../../../../features/coding/shared/gateway/gatewayAggregateConfig.ts';
import {
  isGatewayProxyMode,
  isGatewayAggregateMode,
  isGatewayFailoverMode,
} from '../../../../../features/coding/shared/gateway/providerProtocol.ts';

const status = (
  partial: Partial<GatewayCliTakeoverStatus> & Pick<GatewayCliTakeoverStatus, 'mode'>,
): GatewayCliTakeoverStatus => ({
  cli_key: 'codex',
  state: 'takeover_applied',
  dot: 'green',
  can_takeover: true,
  can_restore_direct: true,
  gateway_origin: 'http://127.0.0.1:37124',
  runtime_root: null,
  managed_targets: [],
  primary_provider_id: null,
  provider_priorities: [],
  message: null,
  ...partial,
});

// ---- mode guards -----------------------------------------------------------

test('mode guards keep aggregate distinct from failover', () => {
  assert.equal(isGatewayProxyMode('single'), true);
  assert.equal(isGatewayProxyMode('failover'), true);
  assert.equal(isGatewayProxyMode('aggregate'), true);
  assert.equal(isGatewayProxyMode(null), false);
  assert.equal(isGatewayProxyMode(undefined), false);

  // Aggregate must never be reported as failover: the failover UI pins a
  // primary provider and shows P0/P1 priorities, neither of which exists here.
  assert.equal(isGatewayFailoverMode('aggregate'), false);
  assert.equal(isGatewayFailoverMode('failover'), true);
  assert.equal(isGatewayAggregateMode('failover'), false);
  assert.equal(isGatewayAggregateMode('aggregate'), true);
});

// ---- separator validation --------------------------------------------------

test('separator must be non-empty and free of site-id characters', () => {
  assert.equal(validateGatewayAggregateSeparator('.'), null);
  assert.equal(validateGatewayAggregateSeparator('::'), null);
  assert.equal(validateGatewayAggregateSeparator('/'), null);
  assert.equal(validateGatewayAggregateSeparator('.'), null);

  assert.equal(validateGatewayAggregateSeparator(''), 'empty');
  assert.equal(validateGatewayAggregateSeparator('a'), 'reservedCharacters');
  assert.equal(validateGatewayAggregateSeparator('9'), 'reservedCharacters');
  assert.equal(validateGatewayAggregateSeparator('_'), 'reservedCharacters');
  assert.equal(validateGatewayAggregateSeparator('-'), 'reservedCharacters');
  // One bad character inside an otherwise fine separator is still rejected.
  assert.equal(validateGatewayAggregateSeparator('.-'), 'reservedCharacters');
});

// ---- site ids --------------------------------------------------------------

test('site ids follow the backend addressable charset', () => {
  assert.equal(isAggregateSiteId('76a6ef74'), true);
  assert.equal(isAggregateSiteId('site-1'), true);
  assert.equal(isAggregateSiteId('site_1'), true);
  assert.equal(isAggregateSiteId(''), false);
  assert.equal(isAggregateSiteId('site.1'), false);
  assert.equal(isAggregateSiteId('site:1'), false);
  assert.equal(isAggregateSiteId('site 1'), false);
});

test('normalize keeps order, drops duplicates and unusable ids', () => {
  assert.deepEqual(
    normalizeGatewayAggregateSiteIds(['a', 'b', 'a', ' site:1 ', 'c', '']),
    ['a', 'b', 'c'],
  );
  assert.deepEqual(normalizeGatewayAggregateSiteIds([]), []);
});

// ---- re-engage resolution --------------------------------------------------

test('re-engage mode only accepts aggregate when its selection is complete', () => {
  assert.equal(resolveGatewayReengageMode(status({ mode: 'single' })), 'single');
  assert.equal(resolveGatewayReengageMode(status({ mode: 'failover' })), 'failover');
  assert.equal(resolveGatewayReengageMode(status({ mode: null })), null);
  assert.equal(resolveGatewayReengageMode(null), null);
  assert.equal(resolveGatewayReengageMode(undefined), null);

  // Aggregate without manifest details must not re-engage: engaging with an
  // empty site list would silently drop the whole cross-site model list.
  assert.equal(resolveGatewayReengageMode(status({ mode: 'aggregate' })), null);
  assert.equal(
    resolveGatewayReengageMode(
      status({ mode: 'aggregate', aggregate: { provider_ids: [], separator: '.' } }),
    ),
    null,
  );
  assert.equal(
    resolveGatewayReengageMode(
      status({ mode: 'aggregate', aggregate: { provider_ids: ['site1'], separator: '-' } }),
    ),
    null,
  );
  assert.equal(
    resolveGatewayReengageMode(
      status({ mode: 'aggregate', aggregate: { provider_ids: ['site1'], separator: '.' } }),
    ),
    'aggregate',
  );
});

test('aggregate reengage config is only built for a valid aggregate manifest', () => {
  assert.equal(toGatewayAggregateReengageConfig(status({ mode: 'single' })), null);
  // An invalid separator cannot be replayed: the backend would reject it.
  assert.equal(
    toGatewayAggregateReengageConfig(
      status({ mode: 'aggregate', aggregate: { provider_ids: ['b', 'a'], separator: '-' } }),
    ),
    null,
  );
  assert.deepEqual(
    toGatewayAggregateReengageConfig(
      status({
        mode: 'aggregate',
        aggregate: { provider_ids: ['b', 'a', 'b'], separator: '::' },
      }),
    ),
    { providerIds: ['b', 'a'], separator: '::', aliases: {}, naming: 'site_model' },
  );
});

test('aggregate slug joins site id and model with the configured separator', () => {
  assert.equal(buildGatewayAggregateModelSlug('76a6ef74', 'deepseek-v4-flash', '.'), '76a6ef74.deepseek-v4-flash');
  assert.equal(buildGatewayAggregateModelSlug('site-1', 'glm-5.3', '::'), 'site-1::glm-5.3');
});

test('aggregate preview uses the effective alias and naming template', () => {
  assert.equal(
    buildGatewayAggregateSitePreviewSlug('site-a', '.', 'site_model', { 'site-a': 'relay' }),
    'relay.<model>',
  );
  assert.equal(
    buildGatewayAggregateSitePreviewSlug('site-a', '@', 'model_at_site', { 'site-a': 'relay' }),
    '<model>@relay',
  );
  assert.equal(
    buildGatewayAggregateSitePreviewSlug('site-a', '.', 'model_only', { 'site-a': 'relay' }),
    '<model>',
  );
});

test('aliases may not shadow any candidate site id', () => {
  // Another *selected* site's id: the auto-derived prefix already collides.
  assert.equal(
    normalizeGatewayAggregateAliases({ 'site-a': 'site-b' }, ['site-a', 'site-b']),
    null,
  );
  // An *unselected* candidate keeps answering to its provider id at request
  // time, so the backend refuses this prefix; the form must not submit it.
  assert.equal(
    normalizeGatewayAggregateAliases({ 'site-a': 'site-c' }, ['site-a'], ['site-a', 'site-c']),
    null,
  );
  // Prefix matching is case-insensitive, so the shadow check must be too.
  assert.equal(
    normalizeGatewayAggregateAliases({ 'site-a': 'SITE-C' }, ['site-a'], ['site-a', 'site-c']),
    null,
  );
  // A real alias, and an alias equal to its own site id, both stay usable.
  assert.deepEqual(
    normalizeGatewayAggregateAliases({ 'site-a': 'relay' }, ['site-a'], ['site-a', 'site-c']),
    { 'site-a': 'relay' },
  );
  assert.deepEqual(
    normalizeGatewayAggregateAliases({ 'site-a': 'site-a' }, ['site-a'], ['site-a', 'site-c']),
    { 'site-a': 'site-a' },
  );
  // Only *effective* prefixes must be unique: site-b answers to its own alias,
  // so site-a may take over the raw id `site-b` (the backend accepts this too).
  assert.deepEqual(
    normalizeGatewayAggregateAliases(
      { 'site-a': 'site-b', 'site-b': 'relay-b' },
      ['site-a', 'site-b'],
    ),
    { 'site-a': 'site-b', 'site-b': 'relay-b' },
  );
  // Blank aliases are dropped rather than treated as an alias.
  assert.deepEqual(
    normalizeGatewayAggregateAliases({ 'site-a': '  ' }, ['site-a'], ['site-a']),
    {},
  );
});

// ---- site-name prefixes and bare-name aliases ------------------------------

test('explicit aliases allow CJK but reject structural characters', () => {
  assert.equal(validateGatewayAggregateAlias('思源888', '.'), true);
  assert.equal(validateGatewayAggregateAlias('中文', '.'), true);
  // A space is the separator between a display name and a model, so an
  // explicit alias can never contain one.
  assert.equal(validateGatewayAggregateAlias('思源888 pro', '.'), false);
  assert.equal(validateGatewayAggregateAlias('with.dot', '.'), false);
  assert.equal(validateGatewayAggregateAlias('with/slash', '.'), true);
  assert.equal(validateGatewayAggregateAlias('with/slash', '/'), false);
  assert.equal(validateGatewayAggregateAlias('', '.'), false);
  assert.equal(validateGatewayAggregateAlias('a'.repeat(33), '.'), false);
});

test('derived site prefixes normalise a display name into a usable token', () => {
  assert.equal(deriveGatewayAggregateSitePrefix('思源888 pro', '.'), '思源888-pro');
  assert.equal(deriveGatewayAggregateSitePrefix('ai.nexfaro.com', '.'), 'ai-nexfaro-com');
  assert.equal(deriveGatewayAggregateSitePrefix('  a\t\tb  ', '.'), 'a-b');
  assert.equal(deriveGatewayAggregateSitePrefix('   ', '.'), null);
});

test('effective aliases default to the site name but never steal an address', () => {
  const selected = [
    { id: '76a6ef74af6c4151812787cc519b534b', name: '思源888 pro' },
    { id: 'ccex', name: 'Unsee Relay' },
  ];
  const allIds = ['76a6ef74af6c4151812787cc519b534b', 'ccex'];

  assert.deepEqual(resolveGatewayAggregateEffectiveAliases({}, selected, allIds, '.'), {
    '76a6ef74af6c4151812787cc519b534b': '思源888-pro',
    ccex: 'Unsee-Relay',
  });

  // An explicit alias always wins over the derived display name.
  assert.deepEqual(
    resolveGatewayAggregateEffectiveAliases({ ccex: 'relay' }, selected, allIds, '.'),
    { '76a6ef74af6c4151812787cc519b534b': '思源888-pro', ccex: 'relay' },
  );

  // A derived name that collides with another address keeps the provider id
  // instead of shadowing it, so engaging never fails on a duplicate name.
  assert.deepEqual(
    resolveGatewayAggregateEffectiveAliases(
      {},
      [
        { id: 'a', name: 'b' },
        { id: 'b', name: 'Bee' },
      ],
      ['a', 'b'],
      '.',
    ),
    { b: 'Bee' },
  );

  // Two sites sharing a display name: the first keeps it, the second falls
  // back to its provider id (no silent overwrite).
  assert.deepEqual(
    resolveGatewayAggregateEffectiveAliases(
      {},
      [
        { id: 'a', name: 'Relay' },
        { id: 'b', name: 'Relay' },
      ],
      ['a', 'b'],
      '.',
    ),
    { a: 'Relay' },
  );
});

test('effective alias map round-trips a saved aggregate manifest', () => {
  // A manifest saved before names were derived stores only explicit aliases;
  // re-engaging must not lose them, and a CJK alias must survive the trip.
  const manifest = {
    provider_ids: ['76a6ef74', 'ccex'],
    separator: '.',
    aliases: { '76a6ef74': '思源888' },
  };
  assert.deepEqual(
    toGatewayAggregateReengageConfig(
      status({ mode: 'aggregate', aggregate: manifest }),
    ),
    {
      providerIds: ['76a6ef74', 'ccex'],
      separator: '.',
      aliases: { '76a6ef74': '思源888' },
      naming: 'site_model',
    },
  );
});
