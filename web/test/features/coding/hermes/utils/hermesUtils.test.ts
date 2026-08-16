/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  parseReasoningEffort,
  HERMES_REASONING_LEVELS,
} from '../../../../../features/coding/hermes/utils/hermesUtils.ts';

test('HERMES_REASONING_LEVELS matches the official 8 levels', () => {
  assert.deepEqual([...HERMES_REASONING_LEVELS], [
    'none', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max', 'ultra',
  ]);
});

test('parseReasoningEffort normalizes a valid level', () => {
  assert.equal(parseReasoningEffort(' high '), 'high');
  assert.equal(parseReasoningEffort('medium'), 'medium');
  assert.equal(parseReasoningEffort('xhigh'), 'xhigh');
});

test('parseReasoningEffort returns undefined for blank/invalid/non-string', () => {
  assert.equal(parseReasoningEffort(''), undefined);
  assert.equal(parseReasoningEffort('  '), undefined);
  assert.equal(parseReasoningEffort('insane'), undefined);
  assert.equal(parseReasoningEffort(undefined), undefined);
  assert.equal(parseReasoningEffort(42), undefined);
});