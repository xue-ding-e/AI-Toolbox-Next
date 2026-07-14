/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import { normalizeGrokCatalogModels } from '../../../../../features/coding/grok/utils/grokCatalogModels.ts';
import { buildGrokSettingsConfig } from '../../../../../features/coding/grok/utils/grokSettingsConfig.ts';

test('Grok catalog normalization preserves the complete model payload', () => {
  const normalizedModels = normalizeGrokCatalogModels([{
      key: 'grok-complete',
      model: 'upstream-grok',
      displayName: 'Grok Complete',
      description: 'Complete field fixture',
      baseUrl: 'https://model.example.com/v1',
      apiBackend: 'responses',
      apiKey: null,
      envKey: 'XAI_API_KEY',
      contextWindow: 131072,
      maxCompletionTokens: 16384,
      temperature: 0,
      topP: 0.9,
      supportsBackendSearch: false,
      supportsReasoningEffort: true,
      reasoningEffort: 'high',
      streamToolCalls: false,
      maxRetries: 0,
      inferenceIdleTimeoutSecs: 120,
      extraHeaders: {},
      extraConfig: {},
      supportsImage: false,
      vision: true,
      attachment: false,
      modalities: {
        input: ['text', 'image'],
        output: ['text'],
      },
    }]);

  assert.deepEqual(normalizedModels[0], {
    key: 'grok-complete',
    model: 'upstream-grok',
    displayName: 'Grok Complete',
    description: 'Complete field fixture',
    baseUrl: 'https://model.example.com/v1',
    apiBackend: 'responses',
    apiKey: null,
    envKey: 'XAI_API_KEY',
    contextWindow: 131072,
    maxCompletionTokens: 16384,
    temperature: 0,
    topP: 0.9,
    supportsBackendSearch: false,
    supportsReasoningEffort: true,
    reasoningEffort: 'high',
    streamToolCalls: false,
    maxRetries: 0,
    inferenceIdleTimeoutSecs: 120,
    extraHeaders: {},
    extraConfig: {},
    supportsImage: false,
    vision: true,
    attachment: false,
    modalities: {
      input: ['text', 'image'],
      output: ['text'],
    },
  });
});

test('buildGrokSettingsConfig overwrites stale model apiBackend with form apiFormat', () => {
  const settingsConfig = JSON.parse(buildGrokSettingsConfig({
    category: 'custom',
    apiKey: 'secret',
    baseUrl: 'https://chat.example.com/v1',
    model: 'custom',
    apiFormat: 'openai_chat',
    config: '',
    catalogModels: [{
      key: 'custom',
      model: 'grok-4.5',
      displayName: 'custom',
      // Stale value left from a previous responses channel / import.
      apiBackend: 'responses',
    }],
    auth: {},
  }));

  assert.equal(settingsConfig.defaultModelKey, 'custom');
  assert.equal(settingsConfig.modelCatalog.models[0].apiBackend, 'chat_completions');
});

test('buildGrokSettingsConfig projects anthropic and responses form formats', () => {
  const responsesConfig = JSON.parse(buildGrokSettingsConfig({
    category: 'custom',
    apiKey: 'secret',
    baseUrl: 'https://api.example.com/v1',
    model: 'custom',
    apiFormat: 'openai_responses',
    config: '',
    catalogModels: [{ key: 'custom', model: 'grok-4.5', apiBackend: 'chat_completions' }],
    auth: {},
  }));
  assert.equal(responsesConfig.modelCatalog.models[0].apiBackend, 'responses');

  const anthropicConfig = JSON.parse(buildGrokSettingsConfig({
    category: 'custom',
    apiKey: 'secret',
    baseUrl: 'https://api.example.com/v1',
    model: 'custom',
    apiFormat: 'anthropic_messages',
    config: '',
    catalogModels: [{ key: 'custom', model: 'claude-sonnet', apiBackend: 'responses' }],
    auth: {},
  }));
  assert.equal(anthropicConfig.modelCatalog.models[0].apiBackend, 'messages');
});

test('buildGrokSettingsConfig forces supportsBackendSearch across catalog models', () => {
  const enabledConfig = JSON.parse(buildGrokSettingsConfig({
    category: 'custom',
    apiKey: 'secret',
    baseUrl: 'https://cpa.example.com/v1',
    model: 'cpa-grok45',
    apiFormat: 'openai_responses',
    supportsBackendSearch: true,
    config: '',
    catalogModels: [
      { key: 'cpa-grok45', model: 'grok-4.5', supportsBackendSearch: false },
      { key: 'cpa-fast', model: 'grok-4-fast' },
    ],
    auth: {},
  }));
  assert.equal(enabledConfig.modelCatalog.models[0].supportsBackendSearch, true);
  assert.equal(enabledConfig.modelCatalog.models[1].supportsBackendSearch, true);

  const emptyCatalogConfig = JSON.parse(buildGrokSettingsConfig({
    category: 'custom',
    apiKey: 'secret',
    baseUrl: 'https://cpa.example.com/v1',
    model: 'cpa-grok45',
    apiFormat: 'openai_responses',
    supportsBackendSearch: true,
    config: '',
    catalogModels: [],
    auth: {},
  }));
  assert.equal(emptyCatalogConfig.modelCatalog.models[0].supportsBackendSearch, true);

  const disabledConfig = JSON.parse(buildGrokSettingsConfig({
    category: 'custom',
    apiKey: 'secret',
    baseUrl: 'https://cpa.example.com/v1',
    model: 'cpa-grok45',
    apiFormat: 'openai_responses',
    supportsBackendSearch: false,
    config: '',
    catalogModels: [{ key: 'cpa-grok45', model: 'grok-4.5', supportsBackendSearch: true }],
    auth: {},
  }));
  assert.equal(disabledConfig.modelCatalog.models[0].supportsBackendSearch, false);
});
