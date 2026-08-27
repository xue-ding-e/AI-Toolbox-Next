/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import { normalizeCodexCatalogModels } from '../../../../../features/coding/codex/utils/codexCatalogModels.ts';
import {
  buildCodexSettingsConfig,
  parseCodexSettingsConfig,
} from '../../../../../features/coding/codex/utils/codexSettingsConfig.ts';
import { extractCodexModel } from '../../../../../utils/codexConfigUtils.ts';

test('parseCodexSettingsConfig accepts JSON objects and rejects other top-level values', () => {
  assert.deepEqual(parseCodexSettingsConfig('{"config":"model = \\"gpt-5.6\\""}'), {
    config: 'model = "gpt-5.6"',
  });
  assert.deepEqual(parseCodexSettingsConfig('null'), {});
  assert.deepEqual(parseCodexSettingsConfig('[]'), {});
  assert.deepEqual(parseCodexSettingsConfig('{invalid'), {});
});

test('normalizeCodexCatalogModels preserves image capability metadata', () => {
  const models = normalizeCodexCatalogModels([
    {
      model: ' text-only-model ',
      displayName: ' Text Only ',
      contextWindow: '128,000',
      supportsImage: false,
      vision: false,
      attachment: false,
      modalities: {
        input: [' text ', 'image', ''],
        output: [' text '],
      },
    },
    {
      model: 'vision-model',
      supportsImage: true,
      modalities: {
        input: ['text', 'image'],
      },
    },
  ]);

  assert.deepEqual(models, [
    {
      model: 'text-only-model',
      displayName: 'Text Only',
      contextWindow: 128000,
      supportsImage: false,
      vision: false,
      attachment: false,
      modalities: {
        input: ['text', 'image'],
        output: ['text'],
      },
    },
    {
      model: 'vision-model',
      supportsImage: true,
      modalities: {
        input: ['text', 'image'],
      },
    },
  ]);
});

test('normalizeCodexCatalogModels keeps same model with distinct display names but collapses identical rows', () => {
  const models = normalizeCodexCatalogModels([
    { model: 'terra', displayName: 'luna' },
    { model: 'terra', displayName: 'luna' }, // fully identical → collapsed
    { model: 'terra', displayName: 'terra' }, // same model, distinct name → kept
    { model: 'terra' }, // same model, no display name → kept as its own row
  ]);

  assert.deepEqual(
    models.map((item) => ({ model: item.model, displayName: item.displayName })),
    [
      { model: 'terra', displayName: 'luna' },
      { model: 'terra', displayName: 'terra' },
      { model: 'terra', displayName: undefined },
    ],
  );
});

test('normalizeCodexCatalogModels preserves reasoning levels and drops empty values', () => {
  const models = normalizeCodexCatalogModels([
    {
      model: 'glm-5.2',
      displayName: 'GLM 5.2',
      reasoningLevels: ['high', 'low', 'bogus', '  '],
      defaultReasoningLevel: '  high  ',
    },
    {
      model: 'deepseek-v4',
      displayName: 'DeepSeek V4',
      reasoningLevels: [],
      defaultReasoningLevel: '',
    },
  ]);

  // Empty/whitespace entries are dropped; non-canonical tokens like "bogus"
  // survive here (canonical filtering happens later at catalog-generation).
  assert.deepEqual(models, [
    {
      model: 'glm-5.2',
      displayName: 'GLM 5.2',
      reasoningLevels: ['high', 'low', 'bogus'],
      defaultReasoningLevel: 'high',
    },
    {
      model: 'deepseek-v4',
      displayName: 'DeepSeek V4',
    },
  ]);
});

test('buildCodexSettingsConfig persists provider-level auto review model override', () => {
  const settingsConfig = JSON.parse(buildCodexSettingsConfig({
    category: 'custom',
    apiKey: 'sk-test',
    baseUrl: 'https://api.example.com/v1',
    model: 'gpt-5.5',
    config: 'model_provider = "custom"',
    catalogModels: [
      {
        model: 'gpt-5.5',
        displayName: 'GPT 5.5',
      },
    ],
    autoReviewModelOverride: ' gpt-5.5 ',
    auth: {},
  }));

  assert.equal(settingsConfig.autoReviewModelOverride, 'gpt-5.5');
  assert.deepEqual(settingsConfig.modelCatalog.models, [
    {
      model: 'gpt-5.5',
      displayName: 'GPT 5.5',
    },
  ]);
});

test('buildCodexSettingsConfig omits empty auto review model override', () => {
  const settingsConfig = JSON.parse(buildCodexSettingsConfig({
    category: 'custom',
    apiKey: 'sk-test',
    baseUrl: 'https://api.example.com/v1',
    model: 'gpt-5.5',
    config: 'model_provider = "custom"',
    catalogModels: [{ model: 'gpt-5.5' }],
    autoReviewModelOverride: '   ',
    auth: {},
  }));

  assert.equal(settingsConfig.autoReviewModelOverride, undefined);
});

test('buildCodexSettingsConfig keeps the default model independent from model mappings', () => {
  const settingsConfig = JSON.parse(buildCodexSettingsConfig({
    category: 'custom',
    apiKey: 'sk-test',
    baseUrl: 'https://api.example.com/v1',
    model: 'gpt-5.4',
    config: 'model_provider = "custom"',
    catalogModels: [
      { model: 'glm-5.2', displayName: 'GLM 5.2' },
      { model: 'deepseek-v4', displayName: 'DeepSeek V4' },
    ],
    auth: {},
  }));

  assert.equal(extractCodexModel(settingsConfig.config), 'gpt-5.4');
  assert.deepEqual(settingsConfig.modelCatalog.models.map((item: { model: string }) => item.model), [
    'glm-5.2',
    'deepseek-v4',
  ]);
});

test('buildCodexSettingsConfig does not promote a model mapping when the default model is empty', () => {
  const settingsConfig = JSON.parse(buildCodexSettingsConfig({
    category: 'custom',
    apiKey: 'sk-test',
    baseUrl: 'https://api.example.com/v1',
    model: '',
    config: 'model = "old-model"\nmodel_provider = "custom"',
    catalogModels: [{ model: 'glm-5.2', displayName: 'GLM 5.2' }],
    auth: {},
  }));

  assert.equal(extractCodexModel(settingsConfig.config), undefined);
  assert.equal(settingsConfig.modelCatalog.models[0].model, 'glm-5.2');
});
