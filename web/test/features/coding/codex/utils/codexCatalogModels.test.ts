/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import {
  importModelsIntoCatalog,
  fillCodexCatalogModelFromPreset,
  normalizeCodexCatalogModels,
} from '../../../../../features/coding/codex/utils/codexCatalogModels.ts';
import { buildCodexSettingsConfig } from '../../../../../features/coding/codex/utils/codexSettingsConfig.ts';
import { findPresetModelById, updatePresetModels, type PresetModel } from '../../../../../constants/presetModels.ts';

const resolvePreset = (modelId: string): PresetModel | undefined => {
  if (modelId === 'gpt-6-astra') {
    return { id: modelId, name: 'GPT-6 Astra', contextLimit: 1_050_000 };
  }
  if (modelId === 'glm-5.3-flash') {
    return { id: modelId, name: 'GLM 5.3 Flash', contextLimit: 1_000_000 };
  }
  return undefined;
};

test('importModelsIntoCatalog reorders existing rows to the fetched display order and appends additions', () => {
  const rows = importModelsIntoCatalog(
    [{ model: 'qwen3.8-flash', contextWindow: 128000 }, { model: 'gpt-6-astra' }],
    [{ id: 'glm-5.3-flash' }],
    [],
    ['gpt-6-astra', 'glm-5.3-flash', 'qwen3.8-flash'],
    resolvePreset,
  );

  assert.deepEqual(rows, [
    { model: 'gpt-6-astra' },
    { model: 'glm-5.3-flash', displayName: 'GLM 5.3 Flash', contextWindow: 1000000 },
    { model: 'qwen3.8-flash', contextWindow: 128000 },
  ]);
});

test('importModelsIntoCatalog keeps rows unknown to the fetch at the end in previous order', () => {
  const rows = importModelsIntoCatalog(
    [{ model: 'custom-model' }, { model: 'qwen3.8-flash', contextWindow: 128000 }, { model: 'pinned-model' }],
    [],
    [],
    ['qwen3.8-flash'],
    resolvePreset,
  );

  assert.deepEqual(rows, [
    { model: 'qwen3.8-flash', contextWindow: 128000 },
    { model: 'custom-model' },
    { model: 'pinned-model' },
  ]);
});

test('importModelsIntoCatalog drops removed ids and can re-add them fresh in one call', () => {
  const rows = importModelsIntoCatalog(
    [{ model: 'met/ds4f', contextWindow: 4096 }],
    [{ id: 'met/ds4f' }],
    ['met/ds4f'],
    ['met/ds4f'],
    resolvePreset,
  );

  assert.deepEqual(rows, [{ model: 'met/ds4f' }]);
});

test('importModelsIntoCatalog keeps duplicate model ids with different display names intact', () => {
  const luna = { model: 'terra', displayName: 'luna' };
  const terra = { model: 'terra', displayName: 'terra' };
  const rows = importModelsIntoCatalog(
    [luna, terra],
    [],
    [],
    ['terra'],
    resolvePreset,
  );

  assert.deepEqual(rows, [luna, terra]);
});

test('importModelsIntoCatalog only adds selected ids even when the fetch returns more', () => {
  const rows = importModelsIntoCatalog(
    [],
    [],
    [],
    ['gpt-6-astra', 'glm-5.3-flash'],
    resolvePreset,
  );

  assert.deepEqual(rows, []);
});

test('importModelsIntoCatalog skips blank ids and dedupes ordered ids', () => {
  const rows = importModelsIntoCatalog(
    [],
    [{ id: 'glm-5.3-flash' }],
    [],
    ['', '  ', 'glm-5.3-flash', 'glm-5.3-flash'],
    resolvePreset,
  );

  assert.deepEqual(rows, [{ model: 'glm-5.3-flash', displayName: 'GLM 5.3 Flash', contextWindow: 1000000 }]);
});

test('importModelsIntoCatalog output stays stable through catalog normalization', () => {
  const rows = importModelsIntoCatalog(
    [],
    [{ id: ' gpt-6-astra ' }],
    [],
    ['gpt-6-astra', 'unknown-model'],
    resolvePreset,
  );

  assert.deepEqual(normalizeCodexCatalogModels(rows.filter((row) => row.model === 'gpt-6-astra')), [
    { model: 'gpt-6-astra', displayName: 'GPT-6 Astra', contextWindow: 1050000 },
  ]);
});

test('import preserves customized rows with whitespace IDs through a full settings save', () => {
  const existing = {
    model: ' gpt-6-astra ', contextWindow: 64000, supportsImage: false,
    reasoningLevels: ['low'], defaultReasoningLevel: 'low', serviceTiers: ['priority'],
  };
  const rows = importModelsIntoCatalog([existing], [{ id: 'gpt-6-astra' }], [], ['gpt-6-astra'], resolvePreset);
  const settings = JSON.parse(buildCodexSettingsConfig({
    category: 'custom', apiKey: 'test-key', baseUrl: 'https://example.test/v1',
    model: 'default-model', config: 'model_provider = "custom"', catalogModels: rows, auth: {},
  }));

  assert.deepEqual(settings.modelCatalog.models, [{ ...existing, model: 'gpt-6-astra' }]);
  assert.equal(rows[0], existing);
});

test('normalized removal removes every alias of an existing model', () => {
  const rows = importModelsIntoCatalog([
    { model: ' retired ', displayName: 'First' },
    { model: 'retired', displayName: 'Second' },
    { model: 'kept', contextWindow: 12345 },
  ], [], [' retired '], ['kept'], resolvePreset);
  assert.deepEqual(rows, [{ model: 'kept', contextWindow: 12345 }]);
});

test('unfetched rows preserve their relative order even with interleaved aliases', () => {
  const existing = [
    { model: 'custom-a', displayName: 'A1' },
    { model: 'custom-b', displayName: 'B' },
    { model: 'custom-a', displayName: 'A2' },
  ];
  const rows = importModelsIntoCatalog(existing, [{ id: 'new-model' }], [], ['new-model'], resolvePreset);
  assert.deepEqual(rows, [{ model: 'new-model' }, ...existing]);
});

test('import uses exact bundled presets for the displayed Claude model and saves all filled fields', () => {
  updatePresetModels(JSON.parse(readFileSync(new URL('../../../../../../tauri/resources/preset_models.json', import.meta.url), 'utf8')));
  const rows = importModelsIntoCatalog([], [{ id: 'claude-opus-4-8', name: 'claude-opus-4-8' }], [], ['claude-opus-4-8'], findPresetModelById);
  const settings = JSON.parse(buildCodexSettingsConfig({
    category: 'custom', apiKey: 'test-key', baseUrl: 'https://example.test/v1',
    model: 'default-model', config: 'model_provider = "custom"', catalogModels: rows, auth: {},
  }));
  assert.deepEqual(settings.modelCatalog.models, [{
    model: 'claude-opus-4-8', displayName: 'Claude Opus 4.8', contextWindow: 1000000,
    reasoningLevels: ['low', 'medium', 'high', 'xhigh', 'max'], defaultReasoningLevel: 'high',
  }]);
  assert.deepEqual(rows[0], fillCodexCatalogModelFromPreset({ model: 'claude-opus-4-8' }, findPresetModelById('claude-opus-4-8')));
});

test('preset enrichment preserves explicit row settings and does not invent speed tiers', () => {
  const existing = {
    model: 'gpt-6-astra', displayName: 'My model', contextWindow: 32000,
    reasoningLevels: ['low'], defaultReasoningLevel: 'low', serviceTiers: ['ultrafast'],
    supportsImage: false,
  };
  assert.deepEqual(fillCodexCatalogModelFromPreset(existing, {
    id: existing.model, name: 'Preset name', contextLimit: 1050000,
    reasoning: true, variants: { high: { reasoningEffort: 'high' } },
  }), existing);
  assert.equal(fillCodexCatalogModelFromPreset({ model: existing.model }, resolvePreset(existing.model)).serviceTiers, undefined);
});

test('preset reasoning supports OpenAI, Anthropic and Gemini effort fields in canonical order', () => {
  const row = fillCodexCatalogModelFromPreset({ model: 'test' }, {
    id: 'test', name: 'Test', reasoning: true,
    variants: {
      extended: { reasoningEffort: 'max' },
      careful: { effort: 'high' },
      quick: { thinkingConfig: { thinkingLevel: 'low' } },
      medium: {},
      xhigh: { disabled: true },
      unknown: { effort: 'unsupported' },
    },
  });
  assert.deepEqual(row.reasoningLevels, ['low', 'medium', 'high', 'max']);
  assert.equal(row.defaultReasoningLevel, 'high');
});

test('reasoning-only presets retain manual defaults while unknown models keep API names', () => {
  const row = fillCodexCatalogModelFromPreset({ model: 'legacy' }, { id: 'legacy', name: 'Legacy', reasoning: true });
  assert.deepEqual(row.reasoningLevels, ['low', 'high', 'max']);
  const rows = importModelsIntoCatalog([], [{ id: 'custom/claude-opus-4-8', name: 'My alias' }], [], ['custom/claude-opus-4-8'], findPresetModelById);
  assert.deepEqual(rows, [{ model: 'custom/claude-opus-4-8', displayName: 'My alias' }]);
});

test('non-reasoning, disabled and unsupported variants do not advertise invented efforts', () => {
  const excludedReasoningPresets: PresetModel[] = [
    { id: 'test', name: 'Test', reasoning: false, variants: { high: {} } },
    { id: 'test', name: 'Test', reasoning: true, variants: { high: { disabled: true } } },
    { id: 'test', name: 'Test', reasoning: true, variants: { unsupported: { effort: 'unknown' } } },
  ];
  for (const preset of excludedReasoningPresets) {
    const row = fillCodexCatalogModelFromPreset({ model: 'test' }, preset);
    assert.equal(row.reasoningLevels, undefined);
    assert.equal(row.defaultReasoningLevel, undefined);
  }
});

test('import leaves intentionally empty fields on existing models untouched', () => {
  const existing = { model: 'gpt-6-astra', displayName: '', reasoningLevels: [] };
  assert.deepEqual(importModelsIntoCatalog([existing], [{ id: existing.model }], [], [existing.model], resolvePreset), [existing]);
});
