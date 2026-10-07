/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import { formatModelLimit, hasCompleteModelLimitPair } from '../../utils/modelLimits.ts';

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

test('a limit of a thousand or more is shown in whole K', () => {
  assert.equal(formatModelLimit(128_000), '128K');
  assert.equal(formatModelLimit(200_000), '200K');
  assert.equal(formatModelLimit(1_000), '1K');
});

test('a limit in the millions keeps one decimal only when there is one', () => {
  assert.equal(formatModelLimit(1_000_000), '1M');
  assert.equal(formatModelLimit(2_000_000), '2M');
  assert.equal(formatModelLimit(1_500_000), '1.5M');
});

test('thousands round rather than showing a decimal', () => {
  // The row is for comparing sizes at a glance, so `8.2K` would claim a
  // precision the reader is not there for.
  assert.equal(formatModelLimit(8_192), '8K');
  assert.equal(formatModelLimit(64_999), '65K');
});

test('a limit below a thousand is shown as-is', () => {
  assert.equal(formatModelLimit(500), '500');
  assert.equal(formatModelLimit(0), '0');
});

test('an absent or unusable limit formats to undefined so the caller can omit it', () => {
  // `undefined` rather than an empty string: the card needs to tell "no limit
  // recorded" from "a limit that renders to nothing", and only the former
  // should drop the separator.
  assert.equal(formatModelLimit(undefined), undefined);
  assert.equal(formatModelLimit(Number.NaN), undefined);
  assert.equal(formatModelLimit(Number.POSITIVE_INFINITY), undefined);
  assert.equal(formatModelLimit(-1), undefined);
});
