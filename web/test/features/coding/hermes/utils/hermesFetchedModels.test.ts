/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import { buildFetchedHermesModel } from '../../../../../features/coding/hermes/utils/hermesFetchedModels.ts';
import type { PresetModel } from '../../../../../constants/presetModels.ts';
import type { OpenCodeModelVariant } from '../../../../../types/opencode.ts';

test('buildFetchedHermesModel keeps upstream id casing and name', () => {
  const record = buildFetchedHermesModel({ id: 'Claude-Sonnet-5', name: 'Sonnet 5' });
  assert.equal(record.id, 'Claude-Sonnet-5');
  assert.equal(record.name, 'Sonnet 5');
});

test('buildFetchedHermesModel fills preset context/max_tokens/reasoning', () => {
  const preset: PresetModel = {
    id: 'claude-sonnet-5',
    name: 'Claude Sonnet 5',
    contextLimit: 200000,
    outputLimit: 64000,
    reasoning: true,
    modalities: { input: ['text', 'image'], output: ['text'] },
  };
  const record = buildFetchedHermesModel({ id: 'claude-sonnet-5', name: 'Sonnet' }, preset);
  assert.equal(record.context_length, 200000);
  assert.equal(record.max_tokens, 64000);
  assert.equal(record.reasoning, true);
});

test('buildFetchedHermesModel omits limits when preset lacks them', () => {
  const fallback = buildFetchedHermesModel({ id: 'x' }, { id: 'x', name: 'X' } as PresetModel);
  assert.equal('context_length' in fallback, false);
  assert.equal('max_tokens' in fallback, false);
  assert.equal('reasoning' in fallback, false);
  assert.equal(fallback.id, 'x');
});

test('buildFetchedHermesModel maps preset variants to reasoningEfforts', () => {
  const variants: Record<string, OpenCodeModelVariant> = {
    low: { thinkingLevel: 'low' },
    high: { thinkingLevel: 'high' },
    xhigh: { thinkingLevel: 'xhigh' },
  };
  const record = buildFetchedHermesModel(
    { id: 'claude-opus-5' },
    { id: 'claude-opus-5', name: 'Opus', variants } as PresetModel
  );
  assert.deepEqual(record.reasoningEfforts, { low: 'low', high: 'high', xhigh: 'xhigh' });
});

test('buildFetchedHermesModel omits reasoningEfforts when variants produce none', () => {
  const record = buildFetchedHermesModel(
    { id: 'plain-model' },
    { id: 'plain-model', name: 'Plain' } as PresetModel
  );
  assert.equal('reasoningEfforts' in record, false);
});