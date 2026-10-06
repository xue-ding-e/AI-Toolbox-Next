/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import {
  buildInputFormat,
  buildZcodeModelRowFromPreset,
  preferredPresetNpmTypes,
  presetInputModalities,
  readInputModalities,
  zcodeModalityValuesFor,
} from '../../../../../features/coding/zcode/utils/zcodeModelFields.ts';
import {
  findPresetModelById,
  updatePresetModels,
} from '../../../../../constants/presetModels.ts';

const bundledPresets = JSON.parse(
  readFileSync(
    new URL('../../../../../../tauri/resources/preset_models.json', import.meta.url),
    'utf8',
  ),
);

test('a known preset fills every field ZCode stores', () => {
  updatePresetModels(bundledPresets);

  const row = buildZcodeModelRowFromPreset(
    'claude-opus-4-8',
    'smart',
    findPresetModelById('claude-opus-4-8'),
  );

  assert.deepEqual(row, {
    modelId: 'claude-opus-4-8',
    displayName: 'Claude Opus 4.8',
    ruleKind: 'smart',
    enabled: true,
    properties: {
      contextWindow: 1_000_000,
      inputFormat: { supportsText: true, supportsImage: true, supportsPdf: true },
      supportsToolCall: true,
    },
    optionSpecs: { maxOutputTokens: { max: 128_000 } },
    isDefault: false,
  });
});

test('a manual rule carries neither the smart-only modalities nor tool call', () => {
  updatePresetModels(bundledPresets);

  const row = buildZcodeModelRowFromPreset(
    'claude-opus-4-8',
    'manual',
    findPresetModelById('claude-opus-4-8'),
  );

  // `text` is smart-only, and ZCode rejects a manual rule that carries
  // `supportsToolCall` — the whole provider file is refused, not just the row.
  assert.deepEqual(row.properties?.inputFormat, { supportsImage: true, supportsPdf: true });
  assert.equal(row.properties?.supportsToolCall, undefined);
});

test('a model the catalog does not know still yields a usable row', () => {
  updatePresetModels(bundledPresets);

  assert.deepEqual(buildZcodeModelRowFromPreset('my-private-model', 'smart'), {
    modelId: 'my-private-model',
    displayName: undefined,
    ruleKind: 'smart',
    enabled: true,
    properties: undefined,
    optionSpecs: undefined,
    isDefault: false,
  });
});

test('preset modalities ZCode cannot carry are dropped, not written', () => {
  const gemini = {
    id: 'gemini-3.8-flash',
    name: 'Gemini 3.8 Flash',
    modalities: { input: ['text', 'image', 'pdf', 'video', 'audio'], output: ['text'] },
  };

  assert.deepEqual(presetInputModalities(gemini, 'smart'), [
    'text',
    'image',
    'pdf',
    'video',
    'audio',
  ]);
  assert.deepEqual(presetInputModalities(gemini, 'manual'), ['image', 'pdf', 'video']);
});

test('an empty modality selection writes no inputFormat at all', () => {
  assert.equal(buildInputFormat([]), undefined);
  assert.deepEqual(buildInputFormat(['image', 'pdf']), {
    supportsImage: true,
    supportsPdf: true,
  });
});

test('the stored per-modality booleans read back as the form selection', () => {
  const stored = buildInputFormat(['text', 'video']);
  assert.deepEqual(readInputModalities(stored ?? {}), ['text', 'video']);
  assert.deepEqual(readInputModalities({}), []);
});

test('a provider API format picks which preset group is consulted first', () => {
  assert.deepEqual(preferredPresetNpmTypes('anthropic-messages'), ['@ai-sdk/anthropic']);
  assert.deepEqual(preferredPresetNpmTypes('openai-chat-completions'), [
    '@ai-sdk/openai',
    '@ai-sdk/openai-compatible',
  ]);
  assert.deepEqual(preferredPresetNpmTypes(undefined), []);
});

test('each rule kind offers exactly the modalities its schema declares', () => {
  assert.deepEqual(zcodeModalityValuesFor('smart'), ['text', 'image', 'video', 'audio', 'pdf']);
  assert.deepEqual(zcodeModalityValuesFor('manual'), ['image', 'video', 'pdf']);
});