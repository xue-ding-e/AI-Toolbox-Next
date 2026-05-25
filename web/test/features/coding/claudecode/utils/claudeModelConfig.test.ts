import assert from 'node:assert/strict';
import test from 'node:test';

import {
  getClaudeConfiguredModelIds,
  getClaudeProviderModelConfig,
  hasClaudeOneMMarker,
  setClaudeOneMMarker,
  stripClaudeOneMMarker,
} from '../../../../../features/coding/claudecode/utils/claudeModelConfig.ts';

test('Claude 1M marker helpers handle case-insensitive suffixes', () => {
  assert.equal(hasClaudeOneMMarker('claude-sonnet[1m]'), true);
  assert.equal(stripClaudeOneMMarker('claude-sonnet[1m]'), 'claude-sonnet');
  assert.equal(setClaudeOneMMarker('claude-sonnet', true), 'claude-sonnet[1M]');
  assert.equal(setClaudeOneMMarker('claude-sonnet[1M]', false), 'claude-sonnet');
});

test('getClaudeProviderModelConfig prefers env fields and keeps legacy fallbacks', () => {
  const config = getClaudeProviderModelConfig({
    env: {
      ANTHROPIC_MODEL: 'env-fallback',
      ANTHROPIC_DEFAULT_SONNET_MODEL: 'env-sonnet[1M]',
      ANTHROPIC_DEFAULT_SONNET_MODEL_NAME: 'Env Sonnet',
      ANTHROPIC_DEFAULT_OPUS_MODEL: 'env-opus',
      ANTHROPIC_DEFAULT_HAIKU_MODEL: 'env-haiku[1M]',
      ANTHROPIC_REASONING_MODEL: 'env-reasoning',
    },
    model: 'legacy-fallback',
    haikuModel: 'legacy-haiku',
    sonnetModel: 'legacy-sonnet',
    opusModel: 'legacy-opus',
    reasoningModel: 'legacy-reasoning',
  });

  assert.equal(config.fallbackModel, 'env-fallback');
  assert.equal(config.roles.sonnet.model, 'env-sonnet[1M]');
  assert.equal(config.roles.sonnet.displayName, 'Env Sonnet');
  assert.equal(config.roles.opus.model, 'env-opus');
  assert.equal(config.roles.opus.displayName, 'env-opus');
  assert.equal(config.roles.haiku.model, 'env-haiku');
  assert.equal(config.roles.haiku.displayName, 'env-haiku');
  assert.equal(config.legacyReasoningModel, 'env-reasoning');
});

test('getClaudeConfiguredModelIds can strip 1M markers and dedupe ids', () => {
  const modelIds = getClaudeConfiguredModelIds({
    env: {
      ANTHROPIC_MODEL: 'env-sonnet',
      ANTHROPIC_DEFAULT_SONNET_MODEL: 'env-sonnet[1M]',
      ANTHROPIC_DEFAULT_OPUS_MODEL: 'env-opus[1M]',
      ANTHROPIC_REASONING_MODEL: 'env-opus',
    },
  }, {
    stripOneMMarker: true,
  });

  assert.deepEqual(modelIds, ['env-sonnet', 'env-opus']);
});
