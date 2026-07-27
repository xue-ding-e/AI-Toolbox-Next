/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import type { PresetModel } from '../../../../../constants/presetModels.ts';
import {
  buildFetchedOpenClawModel,
  buildOpenClawModelFromPreset,
} from '../../../../../features/coding/openclaw/utils/openClawFetchedModels.ts';

const minimaxPreset: PresetModel = {
  id: 'MiniMax-M3',
  name: 'MiniMax M3',
  contextLimit: 204800,
  outputLimit: 131072,
  reasoning: true,
  modalities: { input: ['text', 'image'], output: ['text'] },
};

test('buildOpenClawModelFromPreset keeps the provided model id casing', () => {
  const model = buildOpenClawModelFromPreset(minimaxPreset, 'minimax-m3', 'minimax-m3');

  assert.equal(model.id, 'minimax-m3');
  assert.equal(model.name, 'MiniMax M3');
  assert.equal(model.contextWindow, 204800);
  assert.equal(model.maxTokens, 131072);
  assert.equal(model.reasoning, true);
  assert.deepEqual(model.input, ['text', 'image']);
});

test('buildFetchedOpenClawModel preserves upstream id when preset matches case-insensitively', () => {
  const model = buildFetchedOpenClawModel(
    { id: 'minimax-m3', name: 'upstream-name' },
    '@ai-sdk/openai-compatible',
    minimaxPreset,
  );

  assert.equal(model.id, 'minimax-m3');
  assert.equal(model.name, 'MiniMax M3');
  assert.equal(model.contextWindow, 204800);
});

test('buildFetchedOpenClawModel falls back to upstream fields without preset', () => {
  const model = buildFetchedOpenClawModel(
    { id: 'custom-model', name: 'Custom Model' },
    '@ai-sdk/openai-compatible',
  );

  assert.deepEqual(model, {
    id: 'custom-model',
    name: 'Custom Model',
    reasoning: true,
    input: ['text'],
  });
});
