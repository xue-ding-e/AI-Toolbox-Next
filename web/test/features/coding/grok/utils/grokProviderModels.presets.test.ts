/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  getGrokPrimaryPresetNpmTypes,
  GROK_XAI_PRESET_NPM,
  mapGrokApiBackendToApiFormat,
  normalizeGrokApiFormat,
  resolveGrokProviderApiFormat,
} from '../../../../../features/coding/grok/utils/grokProviderModels.ts';

test('normalizeGrokApiFormat keeps known formats and defaults to chat', () => {
  assert.equal(normalizeGrokApiFormat('openai_chat'), 'openai_chat');
  assert.equal(normalizeGrokApiFormat('openai_responses'), 'openai_responses');
  assert.equal(normalizeGrokApiFormat('anthropic_messages'), 'anthropic_messages');
  assert.equal(normalizeGrokApiFormat('unknown'), 'openai_chat');
  assert.equal(normalizeGrokApiFormat(undefined), 'openai_chat');
});

test('mapGrokApiBackendToApiFormat maps live backend strings', () => {
  assert.equal(mapGrokApiBackendToApiFormat('chat_completions'), 'openai_chat');
  assert.equal(mapGrokApiBackendToApiFormat('responses'), 'openai_responses');
  assert.equal(mapGrokApiBackendToApiFormat('messages'), 'anthropic_messages');
  assert.equal(mapGrokApiBackendToApiFormat(''), undefined);
});

test('getGrokPrimaryPresetNpmTypes maps each channel type separately', () => {
  assert.deepEqual(
    getGrokPrimaryPresetNpmTypes('openai_chat'),
    [GROK_XAI_PRESET_NPM, '@ai-sdk/openai-compatible'],
  );
  assert.deepEqual(
    getGrokPrimaryPresetNpmTypes('openai_responses'),
    [GROK_XAI_PRESET_NPM, '@ai-sdk/openai'],
  );
  assert.deepEqual(
    getGrokPrimaryPresetNpmTypes('anthropic_messages'),
    [GROK_XAI_PRESET_NPM, '@ai-sdk/anthropic'],
  );
});

test('resolveGrokProviderApiFormat prefers meta then catalog backend', () => {
  assert.equal(
    resolveGrokProviderApiFormat({
      meta: { apiFormat: 'anthropic_messages' },
      settingsConfig: JSON.stringify({
        modelCatalog: {
          models: [{ key: 'm1', model: 'm1', apiBackend: 'chat_completions' }],
        },
      }),
    }),
    'anthropic_messages',
  );

  assert.equal(
    resolveGrokProviderApiFormat({
      settingsConfig: JSON.stringify({
        modelCatalog: {
          models: [{ key: 'm1', model: 'm1', apiBackend: 'responses' }],
        },
      }),
    }),
    'openai_responses',
  );
});
