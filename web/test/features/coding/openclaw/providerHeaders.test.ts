/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  applyOpenClawUserAgent,
  hasOpenClawUserAgent,
} from '../../../../features/coding/openclaw/utils/providerHeaders.ts';
import { OPENCLAW_DEFAULT_USER_AGENT } from '../../../../features/coding/openclaw/constants.ts';

test('applyOpenClawUserAgent(true) sets headers to the default User-Agent', () => {
  const result = applyOpenClawUserAgent({ models: [] }, true);
  assert.deepEqual(result.headers, { 'User-Agent': OPENCLAW_DEFAULT_USER_AGENT });
});

test('applyOpenClawUserAgent(true) overwrites existing headers', () => {
  const result = applyOpenClawUserAgent({ models: [], headers: { Authorization: 'x' } }, true);
  assert.deepEqual(result.headers, { 'User-Agent': OPENCLAW_DEFAULT_USER_AGENT });
});

test('applyOpenClawUserAgent(false) removes the whole headers key when only UA was set', () => {
  const result = applyOpenClawUserAgent({ models: [], headers: { 'User-Agent': 'x' } }, false);
  assert.equal('headers' in result, false);
});

test('applyOpenClawUserAgent(false) keeps non-User-Agent headers', () => {
  const result = applyOpenClawUserAgent(
    { models: [], headers: { 'User-Agent': 'x', Authorization: 'Bearer y' } },
    false
  );
  assert.deepEqual(result.headers, { Authorization: 'Bearer y' });
});

test('applyOpenClawUserAgent(false) is a no-op when no headers exist', () => {
  const result = applyOpenClawUserAgent({ models: [] }, false);
  assert.equal('headers' in result, false);
  assert.deepEqual(result.models, []);
});

test('hasOpenClawUserAgent detects the User-Agent header', () => {
  assert.equal(hasOpenClawUserAgent({ models: [], headers: { 'User-Agent': 'x' } }), true);
  assert.equal(hasOpenClawUserAgent({ models: [], headers: { Authorization: 'x' } }), false);
  assert.equal(hasOpenClawUserAgent({ models: [] }), false);
  assert.equal(hasOpenClawUserAgent(null), false);
  assert.equal(hasOpenClawUserAgent(undefined), false);
});