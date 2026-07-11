/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import { hasCompleteModelLimitPair } from '../../utils/modelLimits.ts';

test('model limits allow both fields to be empty', () => {
  assert.equal(hasCompleteModelLimitPair(undefined, undefined), true);
});

test('model limits allow both fields to be filled', () => {
  assert.equal(hasCompleteModelLimitPair(500_000, 500_000), true);
});

test('model limits reject a context-only limit', () => {
  assert.equal(hasCompleteModelLimitPair(500_000, undefined), false);
});

test('model limits reject an output-only limit', () => {
  assert.equal(hasCompleteModelLimitPair(undefined, 500_000), false);
});
