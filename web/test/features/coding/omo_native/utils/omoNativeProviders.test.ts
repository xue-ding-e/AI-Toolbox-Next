/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildFetchedOmoNativeModel,
  buildOmoNativeModelFromPreset,
  parseInputTypes,
  parseJsonRecord,
  stringifyInputTypes,
  stringifyRecordField,
} from '../../../../../features/coding/omo_native/utils/omoNativeProviders.ts';
import type { PresetModel } from '../../../../../constants/presetModels.ts';

/**
 * 这些断言守的是 `models.json` 的**字段形状**。
 *
 * OmO Native 与 Pi 同用 senpi 引擎、同读 `models.json`（OMP 读的是 `models.yml`，
 * 形状不同），权威定义在上游 `<runtime>/docs/models.md`。页面最初照 OMP 写，
 * 于是把 OMP 的 `thinking: { efforts, defaultLevel }` 结构写进了这个文件——
 * 引擎不读那个键，用户的编辑静默丢失。
 */

const presetWithThinking: PresetModel = {
  id: 'grok-4.5',
  name: 'Grok 4.5',
  reasoning: true,
  contextLimit: 500_000,
  outputLimit: 500_000,
  modalities: { input: ['text', 'image'], output: ['text'] },
  variants: {
    low: { reasoningEffort: 'low' } as never,
    medium: { reasoningEffort: 'medium' } as never,
  },
};

test('parseInputTypes turns the modal JSON string into the array models.json wants', () => {
  assert.deepEqual(parseInputTypes('["text","image"]'), ['text', 'image']);
  assert.deepEqual(parseInputTypes('["text"]'), ['text']);
  assert.deepEqual(parseInputTypes(undefined), []);
  assert.deepEqual(parseInputTypes(''), []);
  assert.deepEqual(parseInputTypes('not json'), []);
  assert.deepEqual(parseInputTypes('{"a":1}'), [], 'non-array JSON yields no input types');
});

test('parseInputTypes drops non-string members instead of writing them through', () => {
  assert.deepEqual(parseInputTypes('["text",42,null,"image"]'), ['text', 'image']);
});

test('stringifyInputTypes round-trips a file array back into the modal string', () => {
  assert.equal(stringifyInputTypes(['text', 'image']), '["text","image"]');
  assert.equal(stringifyInputTypes([]), undefined);
  assert.equal(stringifyInputTypes(undefined), undefined);
  assert.equal(stringifyInputTypes('["text"]'), undefined, 'a legacy string is not an array');
});

test('stringifyRecordField keeps thinkingLevelMap and drops empty objects', () => {
  const map = { high: 'high', minimal: null };
  assert.deepEqual(JSON.parse(stringifyRecordField(map) ?? '{}'), map);
  assert.equal(stringifyRecordField({}), undefined);
  assert.equal(stringifyRecordField(undefined), undefined);
});

test('parseJsonRecord is the inverse of stringifyRecordField', () => {
  const map = { high: 'high', minimal: null };
  assert.deepEqual(parseJsonRecord(stringifyRecordField(map)), map);
  assert.deepEqual(parseJsonRecord(undefined), {});
  assert.deepEqual(parseJsonRecord('{broken'), {});
});

test('buildOmoNativeModelFromPreset writes thinkingLevelMap, never OMP\'s thinking key', () => {
  const model = buildOmoNativeModelFromPreset(presetWithThinking, 'grok-4.5', 'Grok 4.5');

  assert.ok('thinkingLevelMap' in model, 'senpi reads thinkingLevelMap');
  assert.equal('thinking' in model, false, 'senpi has no bare thinking field');
  assert.equal('thinkingLevelMap' in model && typeof model.thinkingLevelMap === 'object', true);
});

test('buildOmoNativeModelFromPreset writes input as an array, not a string', () => {
  const model = buildOmoNativeModelFromPreset(presetWithThinking, 'grok-4.5', 'Grok 4.5');

  assert.ok(Array.isArray(model.input), 'models.json stores input as an array');
  assert.deepEqual(model.input, ['text', 'image']);
});

test('buildOmoNativeModelFromPreset never writes a model-level api override', () => {
  const model = buildOmoNativeModelFromPreset(presetWithThinking, 'grok-4.5', 'Grok 4.5');

  // `api` is an optional override of the provider's value; the engine's own rows omit it.
  assert.equal('api' in model, false);
});

test('buildOmoNativeModelFromPreset keeps the upstream id casing verbatim', () => {
  const model = buildOmoNativeModelFromPreset(presetWithThinking, 'Grok-4.5-Preview', 'fallback');

  assert.equal(model.id, 'Grok-4.5-Preview');
  assert.equal(model.name, 'Grok 4.5', 'the preset supplies the display name');
});

test('buildFetchedOmoNativeModel falls back to id + name when no preset matches', () => {
  const model = buildFetchedOmoNativeModel({ id: 'unknown-model', name: 'Unknown' }, null);

  assert.deepEqual(model, { id: 'unknown-model', name: 'Unknown' });
  assert.equal('thinkingLevelMap' in model, false);
  assert.equal('contextWindow' in model, false);
});

test('buildFetchedOmoNativeModel enriches a matched preset with capabilities', () => {
  const model = buildFetchedOmoNativeModel({ id: 'grok-4.5', name: '' }, presetWithThinking);

  assert.equal(model.id, 'grok-4.5');
  assert.equal(model.contextWindow, 500_000);
  assert.equal(model.maxTokens, 500_000);
  assert.equal(model.reasoning, true);
  assert.deepEqual(model.input, ['text', 'image']);
  assert.ok('thinkingLevelMap' in model);
});

test('buildFetchedOmoNativeModel uses the id as the fallback name', () => {
  const model = buildFetchedOmoNativeModel({ id: 'grok-4.5', name: '' }, presetWithThinking);

  assert.equal(model.name, 'Grok 4.5', 'preset name wins over the empty upstream name');
});
