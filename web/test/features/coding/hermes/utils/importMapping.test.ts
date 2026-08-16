/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  extractHermesProviderFromCcSwitch,
  parseCcSwitchEnv,
} from '../../../../../features/coding/hermes/utils/importMapping.ts';
import type { CcSwitchProviderCandidate } from '../../../../../services/ccSwitchApi.ts';

const candidate = (
  settingsConfig: string | Record<string, unknown>
): CcSwitchProviderCandidate =>
  ({
    providerId: 'ccs:claude:deepseek',
    rawId: 'deepseek',
    name: 'DeepSeek',
    appType: 'claude',
    settingsConfig,
    extraSettingsConfig: '{}',
    sourceProviderId: 'ccs:claude:deepseek',
  }) as CcSwitchProviderCandidate;

test('extractHermesProviderFromCcSwitch maps env to provider fields', () => {
  const provider = extractHermesProviderFromCcSwitch(
    candidate({
      env: {
        ANTHROPIC_BASE_URL: 'https://api.deepseek.com',
        ANTHROPIC_AUTH_TOKEN: 'sk-test',
      },
    })
  );
  assert.deepEqual(provider, {
    api_mode: 'anthropic',
    base_url: 'https://api.deepseek.com',
    api_key: 'sk-test',
    models: [],
    display_name: 'DeepSeek',
  });
});

test('extractHermesProviderFromCcSwitch falls back to ANTHROPIC_API_KEY', () => {
  const provider = extractHermesProviderFromCcSwitch(
    candidate({
      env: { ANTHROPIC_BASE_URL: 'https://api.deepseek.com', ANTHROPIC_API_KEY: 'sk-alt' },
    })
  );
  assert.equal(provider?.api_key, 'sk-alt');
});

test('extractHermesProviderFromCcSwitch returns null without base url or key', () => {
  assert.equal(extractHermesProviderFromCcSwitch(candidate({ env: {} })), null);
  assert.equal(extractHermesProviderFromCcSwitch(candidate('{ invalid !!')), null);
});

test('parseCcSwitchEnv accepts a JSON string settings_config', () => {
  const env = parseCcSwitchEnv(
    candidate('{"env":{"ANTHROPIC_BASE_URL":"https://x"}}')
  );
  assert.deepEqual(env, { ANTHROPIC_BASE_URL: 'https://x' });
});