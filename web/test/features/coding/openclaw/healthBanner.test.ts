/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import { buildHealthBannerItem } from '../../../../features/coding/openclaw/healthBanner.ts';

const t = (key: string) => `T:${key}`;

test('buildHealthBannerItem localizes known codes', () => {
  assert.equal(
    buildHealthBannerItem({ code: 'legacy_agents_timeout', message: 'backend msg' }, t),
    'T:openclaw.healthBanner.warning.legacyTimeout'
  );
  assert.equal(
    buildHealthBannerItem({ code: 'stringified_env_vars', message: 'backend msg' }, t),
    'T:openclaw.healthBanner.warning.stringifiedEnvVars'
  );
});

test('invalid_tools_profile appends the valid enum', () => {
  assert.equal(
    buildHealthBannerItem({ code: 'invalid_tools_profile', message: 'x' }, t),
    'T:openclaw.healthBanner.warning.invalidToolsProfile (minimal / coding / messaging / full)'
  );
});

test('unknown code falls back to the backend message', () => {
  assert.equal(
    buildHealthBannerItem({ code: 'something_new', message: 'raw backend msg' }, t),
    'raw backend msg'
  );
});