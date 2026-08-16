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
  // credentialRef 基于 providerId(ccs:claude:deepseek)而非 name(DeepSeek),
  // 避免 CJK 显示名折叠成同一下划线 ref 导致跨 provider 凭据互相覆盖。
  assert.equal(mapped?.credentialRef, 'CCS_CLAUDE_DEEPSEEK_API_KEY');
  assert.equal(mapped?.provider.apiKeyEnv, 'CCS_CLAUDE_DEEPSEEK_API_KEY');
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

test('extractDshProviderFromCcSwitch keeps distinct refs for different CJK names via providerId', () => {
  // 两个不同中文渠道("深度求索"/"月之暗面")若凭据 ref 基于 name 会坍缩到同一下划线 ref;
  // 基于 providerId(ASCII slug,唯一)则保持可区分。
  const deepseek = ccCandidate({
    env: { ANTHROPIC_BASE_URL: 'https://a', ANTHROPIC_AUTH_TOKEN: 'k1' },
  });
  const moonshot = ccCandidate({
    env: { ANTHROPIC_BASE_URL: 'https://b', ANTHROPIC_AUTH_TOKEN: 'k2' },
  });
  deepseek.providerId = 'ccs:claude:deepseek';
  deepseek.name = '深度求索';
  moonshot.providerId = 'ccs:claude:moonshot';
  moonshot.name = '月之暗面';
  const a = extractDshProviderFromCcSwitch(deepseek);
  const b = extractDshProviderFromCcSwitch(moonshot);
  assert.notEqual(a?.credentialRef, b?.credentialRef);
  assert.equal(a?.credentialRef, 'CCS_CLAUDE_DEEPSEEK_API_KEY');
  assert.equal(b?.credentialRef, 'CCS_CLAUDE_MOONSHOT_API_KEY');
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