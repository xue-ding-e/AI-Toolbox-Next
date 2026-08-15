/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildDshCredentialRef,
  buildDshProviderFromAllApiHub,
  extractDshProviderFromCcSwitch,
} from '../../../../../features/coding/dsh/utils/importMapping.ts';
import type { CcSwitchProviderCandidate } from '../../../../../services/ccSwitchApi.ts';
import type { AllApiHubProviderItem } from '../../../../../types/allApiHub.ts';

const ccCandidate = (
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

test('extractDshProviderFromCcSwitch builds route + credential ref', () => {
  const mapped = extractDshProviderFromCcSwitch(
    ccCandidate({
      env: {
        ANTHROPIC_BASE_URL: 'https://api.deepseek.com',
        ANTHROPIC_AUTH_TOKEN: 'sk-test',
      },
    })
  );
  assert.equal(mapped?.apiKey, 'sk-test');
  assert.equal(mapped?.credentialRef, 'DEEPSEEK_API_KEY');
  assert.equal(mapped?.provider.apiKeyEnv, 'DEEPSEEK_API_KEY');
  assert.equal(mapped?.provider.api, 'anthropic-messages');
  assert.equal(mapped?.provider.baseURL, 'https://api.deepseek.com');
});

test('extractDshProviderFromCcSwitch returns null without usable fields', () => {
  assert.equal(extractDshProviderFromCcSwitch(ccCandidate({ env: {} })), null);
  assert.equal(extractDshProviderFromCcSwitch(ccCandidate('not json')), null);
});

test('buildDshCredentialRef sanitizes to uppercase underscore env name', () => {
  assert.equal(buildDshCredentialRef('DeepSeek-API'), 'DEEPSEEK_API_API_KEY');
  assert.equal(buildDshCredentialRef('ollama.local'), 'OLLAMA_LOCAL_API_KEY');
});

test('buildDshProviderFromAllApiHub extracts api_key for credential write', () => {
  const item: AllApiHubProviderItem = {
    providerId: 'ext:deepseek',
    name: 'DeepSeek',
    apiProtocol: 'anthropic-messages',
    baseUrl: 'https://api.deepseek.com',
    requiresBrowserOpen: false,
    isDisabled: false,
    hasApiKey: true,
    accountLabel: 'a',
    sourceProfileName: 'p',
    sourceExtensionId: 'e',
    config: {
      api: 'anthropic-messages',
      baseURL: 'https://api.deepseek.com',
      api_key: 'sk-test',
      models: [],
    },
  };
  const { providerKey, provider, apiKey, credentialRef } = buildDshProviderFromAllApiHub(item);
  assert.equal(providerKey, 'ext:deepseek');
  assert.equal(apiKey, 'sk-test');
  assert.equal('api_key' in provider, false);
  assert.equal(provider.apiKeyEnv, credentialRef);
});