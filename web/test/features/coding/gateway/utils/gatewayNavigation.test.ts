import assert from 'node:assert/strict';
import test from 'node:test';

import {
  DEFAULT_GATEWAY_PATH,
  getGatewayPathForTab,
  isGatewayPath,
  resolveGatewayTabFromPath,
} from '../../../../../features/coding/gateway/utils/gatewayNavigation.ts';

test('gateway navigation resolves default path and tab paths', () => {
  assert.equal(DEFAULT_GATEWAY_PATH, '/gateway/statistics');
  assert.equal(getGatewayPathForTab('statistics'), '/gateway/statistics');
  assert.equal(getGatewayPathForTab('requests'), '/gateway/requests');
  assert.equal(getGatewayPathForTab('settings'), '/gateway/settings');
});

test('gateway navigation resolves active tab from path', () => {
  assert.equal(resolveGatewayTabFromPath('/gateway'), 'statistics');
  assert.equal(resolveGatewayTabFromPath('/gateway/statistics'), 'statistics');
  assert.equal(resolveGatewayTabFromPath('/gateway/requests'), 'requests');
  assert.equal(resolveGatewayTabFromPath('/gateway/settings'), 'settings');
  assert.equal(resolveGatewayTabFromPath('/gateway/settings/profile'), 'settings');
});

test('gateway path detection only matches gateway route namespace', () => {
  assert.equal(isGatewayPath('/gateway'), true);
  assert.equal(isGatewayPath('/gateway/settings'), true);
  assert.equal(isGatewayPath('/gateway-settings'), false);
  assert.equal(isGatewayPath('/images'), false);
});
