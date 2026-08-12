/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildOmpThinkingFromPreset,
  getOmpModelDefaultThinkingLevel,
  getOmpModelThinkingLevels,
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

test('buildOmpThinkingFromPreset derives efforts and defaultLevel from variants', () => {
  const thinking = buildOmpThinkingFromPreset({
    none: { reasoningEffort: 'none' },
    medium: { thinkingConfig: { thinkingLevel: 'medium' } },
    high: { disabled: true },
  });
  // none is not an OMP effort; high is disabled; only medium survives.
  assert.deepEqual(thinking, { efforts: ['medium'] });
});
