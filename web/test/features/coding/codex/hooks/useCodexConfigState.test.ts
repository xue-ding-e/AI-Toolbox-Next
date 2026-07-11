/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import { normalizeCodexCatalogModels } from '../../../../../features/coding/codex/utils/codexCatalogModels.ts';
import { buildCodexSettingsConfig } from '../../../../../features/coding/codex/utils/codexSettingsConfig.ts';
import { extractCodexModel } from '../../../../../utils/codexConfigUtils.ts';

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
