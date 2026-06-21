/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildPiThinkingLevelMapFromPreset,
  normalizePiThinkingLevelKey,
} from '../../utils/piModelMetadata.ts';

test('normalizePiThinkingLevelKey maps none to off', () => {
  assert.equal(normalizePiThinkingLevelKey('none'), 'off');
  assert.equal(normalizePiThinkingLevelKey('medium'), 'medium');
  assert.equal(normalizePiThinkingLevelKey('unknown'), undefined);
});

test('buildPiThinkingLevelMapFromPreset fills omitted levels when preset has variants', () => {
  const thinkingLevelMap = buildPiThinkingLevelMapFromPreset({
    none: { reasoningEffort: 'none' },
    medium: { thinkingConfig: { thinkingLevel: 'medium' } },
    high: { disabled: true },
  });

  assert.deepEqual(thinkingLevelMap, {
    off: 'none',
    minimal: null,
    low: null,
    medium: 'medium',
    high: null,
    xhigh: null,
  });
});

test('buildPiThinkingLevelMapFromPreset returns empty map when variants are empty', () => {
  assert.deepEqual(buildPiThinkingLevelMapFromPreset({}), {});
  assert.deepEqual(buildPiThinkingLevelMapFromPreset(undefined), {});
});
