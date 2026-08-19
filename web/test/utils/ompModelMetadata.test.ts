/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildOmpThinkingFromPreset,
  getOmpModelDefaultThinkingLevel,
  getOmpModelThinkingLevels,
  inferOmpThinkingMode,
  normalizeOmpThinkingLevelKey,
} from '../../utils/ompModelMetadata.ts';

test('normalizeOmpThinkingLevelKey maps none to off', () => {
  assert.equal(normalizeOmpThinkingLevelKey('none'), 'off');
  assert.equal(normalizeOmpThinkingLevelKey('medium'), 'medium');
  assert.equal(normalizeOmpThinkingLevelKey('max'), 'max');
  assert.equal(normalizeOmpThinkingLevelKey('unknown'), undefined);
});

test('getOmpModelThinkingLevels returns nothing for non-reasoning or missing model', () => {
  assert.deepEqual(getOmpModelThinkingLevels(undefined), []);
  assert.deepEqual(getOmpModelThinkingLevels({ reasoning: false }), []);
});

test('getOmpModelThinkingLevels strictly follows thinking.efforts (no standard union)', () => {
  const levels = getOmpModelThinkingLevels({
    reasoning: true,
    thinking: { efforts: ['high', 'xhigh'] },
  });
  // Must NOT include minimal/low/medium (they are not declared by the model),
  // matching the backend strict membership check.
  assert.deepEqual(levels, ['high', 'xhigh']);
});

test('getOmpModelThinkingLevels dedupes and orders efforts canonically', () => {
  const levels = getOmpModelThinkingLevels({
    reasoning: true,
    thinking: { efforts: ['xhigh', 'low', 'high', 'low'] },
  });
  assert.deepEqual(levels, ['low', 'high', 'xhigh']);
});

test('getOmpModelThinkingLevels honors minLevel/maxLevel range', () => {
  const levels = getOmpModelThinkingLevels({
    reasoning: true,
    thinking: { minLevel: 'medium', maxLevel: 'max' },
  });
  assert.deepEqual(levels, ['medium', 'high', 'xhigh', 'max']);
});

test('getOmpModelThinkingLevels falls back to standard levels when thinking is absent', () => {
  const levels = getOmpModelThinkingLevels({ reasoning: true });
  assert.deepEqual(levels, ['minimal', 'low', 'medium', 'high']);
});

test('getOmpModelDefaultThinkingLevel reads thinking.defaultLevel', () => {
  assert.equal(
    getOmpModelDefaultThinkingLevel({ reasoning: true, thinking: { defaultLevel: 'high' } }),
    'high',
  );
  assert.equal(getOmpModelDefaultThinkingLevel({ reasoning: true }), undefined);
});

test('buildOmpThinkingFromPreset derives efforts, defaultLevel, and mode from variants', () => {
  const thinking = buildOmpThinkingFromPreset({
    none: { reasoningEffort: 'none' },
    medium: { thinkingConfig: { thinkingLevel: 'medium' } },
    high: { disabled: true },
  });
  // none is not an OMP effort; high is disabled; only medium survives.
  assert.deepEqual(thinking, { mode: 'effort', efforts: ['medium'] });
});

test('buildOmpThinkingFromPreset infers mode from provider api', () => {
  const google = buildOmpThinkingFromPreset(
    { low: { reasoningEffort: 'low' } },
    'google-generative-ai',
  );
  assert.deepEqual(google?.mode, 'google-level');

  const anthropic = buildOmpThinkingFromPreset(
    { low: { reasoningEffort: 'low' } },
    'anthropic-messages',
  );
  assert.deepEqual(anthropic?.mode, 'anthropic-adaptive');
});

test('inferOmpThinkingMode maps apis to mode defaults', () => {
  assert.equal(inferOmpThinkingMode('openai-responses'), 'effort');
  assert.equal(inferOmpThinkingMode('openai-completions'), 'effort');
  assert.equal(inferOmpThinkingMode('openrouter'), 'effort');
  assert.equal(inferOmpThinkingMode('google-generative-ai'), 'google-level');
  assert.equal(inferOmpThinkingMode('google-vertex'), 'google-level');
  assert.equal(inferOmpThinkingMode('anthropic-messages'), 'anthropic-adaptive');
  assert.equal(inferOmpThinkingMode('bedrock-converse-stream'), 'anthropic-adaptive');
  assert.equal(inferOmpThinkingMode(undefined), 'effort');
  assert.equal(inferOmpThinkingMode('some-unknown-api'), 'effort');
});
