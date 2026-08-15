/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  parseThinkingLevelEfforts,
  stringifyReasoningEfforts,
} from '../../../../../features/coding/hermes/utils/hermesUtils.ts';

test('parseThinkingLevelEfforts keeps mapped levels and drops null/empty', () => {
  const result = parseThinkingLevelEfforts(
    JSON.stringify({ minimal: null, low: '', medium: 'medium', high: 'high', off: 'off' })
  );
  assert.deepEqual(result, { medium: 'medium', high: 'high', off: 'off' });
});

test('parseThinkingLevelEfforts returns undefined for empty object / blank', () => {
  assert.equal(parseThinkingLevelEfforts('{}'), undefined);
  assert.equal(parseThinkingLevelEfforts('   '), undefined);
  assert.equal(parseThinkingLevelEfforts(''), undefined);
});

test('parseThinkingLevelEfforts returns undefined for invalid JSON', () => {
  assert.equal(parseThinkingLevelEfforts('{ nope !!'), undefined);
});

test('stringifyReasoningEfforts pretty-prints a non-empty mapping', () => {
  const out = stringifyReasoningEfforts({ high: 'high', max: 'max' });
  assert.equal(out, '{\n  "high": "high",\n  "max": "max"\n}');
});

test('stringifyReasoningEfforts returns undefined for empty / non-object', () => {
  assert.equal(stringifyReasoningEfforts({}), undefined);
  assert.equal(stringifyReasoningEfforts(undefined), undefined);
  assert.equal(stringifyReasoningEfforts('oops'), undefined);
});