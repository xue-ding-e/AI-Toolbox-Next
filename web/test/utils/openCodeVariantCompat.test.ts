/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import { normalizeVariantsForProviderNpm } from '../../utils/openCodeVariantCompat.ts';

test('normalizeVariantsForProviderNpm rewrites thinkingConfig variants for openai-compatible providers', () => {
  const variants = {
    low: { thinkingConfig: { includeThoughts: true, thinkingLevel: 'low' } },
    high: { thinkingConfig: { includeThoughts: true, thinkingLevel: 'high' } },
  };

  assert.deepStrictEqual(normalizeVariantsForProviderNpm(variants, '@ai-sdk/openai-compatible'), {
    low: { reasoningEffort: 'low' },
    high: { reasoningEffort: 'high' },
  });
});

test('normalizeVariantsForProviderNpm falls back to the variant name when thinkingLevel is missing', () => {
  const variants = {
    high: { thinkingConfig: { includeThoughts: true, thinkingBudget: 32768 } },
  };

  assert.deepStrictEqual(normalizeVariantsForProviderNpm(variants, '@ai-sdk/openai-compatible'), {
    high: { reasoningEffort: 'high' },
  });
});

test('normalizeVariantsForProviderNpm keeps sibling options and non-object variants', () => {
  const variants: Record<string, unknown> = {
    high: { thinkingConfig: { includeThoughts: true }, reasoningSummary: 'auto' },
    custom: 'not-an-object',
  };

  assert.deepStrictEqual(normalizeVariantsForProviderNpm(variants, '@ai-sdk/openai-compatible'), {
    high: { reasoningSummary: 'auto', reasoningEffort: 'high' },
    custom: 'not-an-object',
  });
});

test('normalizeVariantsForProviderNpm keeps thinkingConfig for non-openai-compatible providers', () => {
  const variants = {
    high: { thinkingConfig: { includeThoughts: true, thinkingLevel: 'high' } },
  };

  assert.strictEqual(normalizeVariantsForProviderNpm(variants, '@ai-sdk/google'), variants);
  assert.strictEqual(normalizeVariantsForProviderNpm(variants), variants);
});

test('normalizeVariantsForProviderNpm returns reasoningEffort-style variants untouched and handles empty input', () => {
  const variants = { high: { reasoningEffort: 'high' } };

  assert.strictEqual(
    normalizeVariantsForProviderNpm(variants, '@ai-sdk/openai-compatible'),
    variants,
  );
  assert.strictEqual(
    normalizeVariantsForProviderNpm(undefined, '@ai-sdk/openai-compatible'),
    undefined,
  );
});
