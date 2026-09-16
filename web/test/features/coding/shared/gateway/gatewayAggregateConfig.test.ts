import assert from 'node:assert/strict';
import test from 'node:test';

import type { GatewayCliTakeoverStatus } from '../../../../../services/proxyGatewayApi.ts';
import {
  buildGatewayAggregateModelSlug,
  isAggregateSiteId,
  normalizeGatewayAggregateSiteIds,
  prepareGatewayAggregateAliasReengage,
  resolveGatewayReengageMode,
  toGatewayAggregateReengageConfig,
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

test('alias edits produce an immediate re-engage payload only when valid', () => {
  assert.deepEqual(
    prepareGatewayAggregateAliasReengage(
      {},
      'site-a',
      'relay-a',
      ['site-a', 'site-b'],
      true,
      '.',
      'site_model',
    ),
    {
      siteIds: ['site-a', 'site-b'],
      separator: '.',
      aliases: { 'site-a': 'relay-a' },
      naming: 'site_model',
    },
  );
  assert.equal(
    prepareGatewayAggregateAliasReengage(
      {},
      'site-a',
      'bad alias',
      ['site-a'],
      true,
      '.',
      'site_model',
    ),
    null,
  );
  assert.equal(
    prepareGatewayAggregateAliasReengage(
      {},
      'site-a',
      'relay-a',
      ['site-a'],
      false,
      '.',
      'site_model',
    ),
    null,
  );
});
