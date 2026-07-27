/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import type { PresetModel } from '../../../../../constants/presetModels.ts';
import {
  buildFetchedPiModel,
  buildPiModelFromPreset,
  piApiToSdkName,
} from '../../../../../features/coding/pi/utils/piFetchedModels.ts';

const minimaxPreset: PresetModel = {
  id: 'MiniMax-M3',
  name: 'MiniMax M3',
  contextLimit: 204800,
  outputLimit: 131072,
  reasoning: true,
  modalities: { input: ['text', 'image'], output: ['text'] },
  cost: {
    input: 0.3,
    output: 1.2,
    cacheRead: 0.03,
    cacheWrite: 0.375,
  },
};

test('buildPiModelFromPreset keeps the provided model id casing', () => {
  const model = buildPiModelFromPreset(minimaxPreset, 'minimax-m3', 'minimax-m3');

  assert.equal(model.id, 'minimax-m3');
  assert.equal(model.name, 'MiniMax M3');
  assert.equal(model.contextWindow, 204800);
  assert.equal(model.maxTokens, 131072);
  assert.equal(model.reasoning, true);
  assert.deepEqual(model.input, ['text', 'image']);
  assert.deepEqual(model.cost, {
    input: 0.3,
    output: 1.2,
    cacheRead: 0.03,
    cacheWrite: 0.375,
  });
});

test('buildFetchedPiModel preserves upstream id when preset matches case-insensitively', () => {
  const model = buildFetchedPiModel(
    { id: 'minimax-m3', name: 'upstream-name' },
    minimaxPreset,
  );

  assert.equal(model.id, 'minimax-m3');
  assert.equal(model.name, 'MiniMax M3');
  assert.equal(model.contextWindow, 204800);
});

test('buildFetchedPiModel falls back to upstream fields without preset', () => {
  const model = buildFetchedPiModel(
    { id: 'custom-model', name: 'Custom Model' },
  );

  assert.deepEqual(model, {
    id: 'custom-model',
    name: 'Custom Model',
  });
});

test('piApiToSdkName maps known Pi APIs', () => {
  assert.equal(piApiToSdkName('anthropic-messages'), '@ai-sdk/anthropic');
  assert.equal(piApiToSdkName('google-generative-ai'), '@ai-sdk/google');
  assert.equal(piApiToSdkName('openai-completions'), '@ai-sdk/openai-compatible');
});
