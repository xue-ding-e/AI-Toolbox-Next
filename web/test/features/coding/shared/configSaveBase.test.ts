/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import { pickConfigSaveBase } from '../../../../features/coding/shared/configSaveBase.ts';

type FakeConfig = { provider?: Record<string, unknown>; mcp?: Record<string, unknown> };
type FakeReadResult =
  | { status: 'success'; config: FakeConfig }
  | { status: 'notFound'; path: string }
  | { status: 'parseError'; path: string; error: string }
  | { status: 'error'; error: string };

test('pickConfigSaveBase prefers the config re-read from the file', () => {
  const fromFile: FakeConfig = { provider: {}, mcp: { demo: { type: 'local' } } };
  const fromPage: FakeConfig = { provider: {}, mcp: { old: { type: 'local' } } };

  assert.equal(pickConfigSaveBase({ status: 'success', config: fromFile }, fromPage), fromFile);
});

test('pickConfigSaveBase falls back to the page copy when the file cannot be read', () => {
  const fromPage: FakeConfig = { provider: {} };
  const notFound: FakeReadResult = { status: 'notFound', path: '/tmp/config.json' };
  const parseError: FakeReadResult = { status: 'parseError', path: '/tmp/config.json', error: 'boom' };
  const error: FakeReadResult = { status: 'error', error: 'boom' };

  assert.equal(pickConfigSaveBase(notFound, fromPage), fromPage);
  assert.equal(pickConfigSaveBase(parseError, fromPage), fromPage);
  assert.equal(pickConfigSaveBase(error, fromPage), fromPage);
  assert.equal(pickConfigSaveBase(null, fromPage), fromPage);
  assert.equal(pickConfigSaveBase(undefined, fromPage), fromPage);
});

test('pickConfigSaveBase never returns undefined', () => {
  assert.equal(pickConfigSaveBase(null, null), null);
  assert.equal(pickConfigSaveBase(undefined, undefined), null);
  assert.equal(pickConfigSaveBase({ status: 'success' }, undefined), null);
});
