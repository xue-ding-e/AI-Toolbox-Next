/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import type { PresetModel } from '../../../../../constants/presetModels.ts';
import {
  buildFetchedOmpModel,
  buildOmpModelFromPreset,
  ompApiToSdkName,
} from '../../../../../features/coding/oh_my_pi/utils/ompFetchedModels.ts';

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

const thinkingPreset: PresetModel = {
  id: 'deepseek-reasoner',
  name: 'DeepSeek Reasoner',
  variants: {
    low: { reasoningEffort: 'low' },
    high: { reasoningEffort: 'high' },
    max: { reasoningEffort: 'max' },
  },
};

test('buildOmpModelFromPreset keeps the provided model id casing', () => {
  const model = buildOmpModelFromPreset(minimaxPreset, 'minimax-m3', 'minimax-m3');

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

test('buildFetchedOmpModel fills thinking from preset variants, not thinkingLevelMap', () => {
  const model = buildFetchedOmpModel(
    { id: 'deepseek-reasoner', name: 'DeepSeek Reasoner' },
    thinkingPreset,
  );

  assert.equal(model.id, 'deepseek-reasoner');
  assert.equal(model.name, 'DeepSeek Reasoner');
  assert.deepEqual(model.thinking, { efforts: ['low', 'high', 'max'], defaultLevel: 'high' });
  assert.equal('thinkingLevelMap' in model, false);
});

test('buildFetchedOmpModel omits thinking when preset has no variants', () => {
  const model = buildFetchedOmpModel(
    { id: 'minimax-m3' },
    minimaxPreset,
  );

  assert.equal('thinking' in model, false);
});

test('buildFetchedOmpModel falls back to upstream fields without preset', () => {
  const model = buildFetchedOmpModel(
    { id: 'custom-model', name: 'Custom Model' },
  );

  assert.deepEqual(model, {
    id: 'custom-model',
    name: 'Custom Model',
  });
});

test('ompApiToSdkName maps known OMP APIs', () => {
  assert.equal(ompApiToSdkName('anthropic-messages'), '@ai-sdk/anthropic');
  assert.equal(ompApiToSdkName('google-generative-ai'), '@ai-sdk/google');
  assert.equal(ompApiToSdkName('openai-completions'), '@ai-sdk/openai-compatible');
});