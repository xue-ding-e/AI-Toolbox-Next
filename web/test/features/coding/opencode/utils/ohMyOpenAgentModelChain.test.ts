/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  getAgentModelDisplay,
  modelChainEntryToId,
} from '../../../../../features/coding/opencode/utils/ohMyOpenAgentModelChain.ts';

test('modelChainEntryToId reads a bare model id string', () => {
  assert.equal(modelChainEntryToId('openai/gpt-5.5'), 'openai/gpt-5.5');
});

test('modelChainEntryToId reads model id from a { model } override object', () => {
  assert.equal(
    modelChainEntryToId({ model: 'anthropic/claude-opus-5', reasoning: 'high' }),
    'anthropic/claude-opus-5',
  );
});

test('modelChainEntryToId drops empty strings and malformed entries', () => {
  assert.equal(modelChainEntryToId(''), undefined);
  assert.equal(modelChainEntryToId(null), undefined);
  assert.equal(modelChainEntryToId({ reasoning: 'high' }), undefined);
  assert.equal(modelChainEntryToId({ model: 123 }), undefined);
});

test('getAgentModelDisplay returns nothing for a missing/empty agent', () => {
  assert.deepEqual(getAgentModelDisplay(undefined), { primaryModel: undefined, fallbackCount: 0 });
  assert.deepEqual(getAgentModelDisplay({}), { primaryModel: undefined, fallbackCount: 0 });
});

test('getAgentModelDisplay reads a lone primary `model` string', () => {
  assert.deepEqual(
    getAgentModelDisplay({ model: 'openai/gpt-5.5' }),
    { primaryModel: 'openai/gpt-5.5', fallbackCount: 0 },
  );
});

test('getAgentModelDisplay reads the canonical `models` chain (primary first, rest fallbacks)', () => {
  // Strings + object entries mixed, with a malformed entry that must be ignored.
  assert.deepEqual(
    getAgentModelDisplay({
      models: [
        { model: 'anthropic/claude-opus-5', reasoning: 'high' },
        'openai/gpt-5.4',
        { model: 'google/gemini-2.5-pro' },
        '',
        { reasoning: 'low' },
      ],
    }),
    {
      primaryModel: 'anthropic/claude-opus-5',
      fallbackCount: 2,
    },
  );
});

test('getAgentModelDisplay handles a `models` chain of bare strings', () => {
  assert.deepEqual(
    getAgentModelDisplay({ models: ['openai/gpt-5.5', 'openai/gpt-5.4-mini', 'google/gemini-2.5-pro'] }),
    { primaryModel: 'openai/gpt-5.5', fallbackCount: 2 },
  );
});

test('getAgentModelDisplay treats a single-entry `models` array as primary with no fallbacks', () => {
  assert.deepEqual(
    getAgentModelDisplay({ models: [{ model: 'openai/gpt-5.5', reasoning: 'max' }] }),
    { primaryModel: 'openai/gpt-5.5', fallbackCount: 0 },
  );
});

test('getAgentModelDisplay falls back to legacy `model` + `fallback_models` array', () => {
  assert.deepEqual(
    getAgentModelDisplay({
      model: 'openai/gpt-5.5',
      fallback_models: ['openai/gpt-5.4-mini', 'google/gemini-2.5-pro'],
    }),
    { primaryModel: 'openai/gpt-5.5', fallbackCount: 2 },
  );
});

test('getAgentModelDisplay coerces a legacy string `fallback_models` to a single fallback', () => {
  assert.deepEqual(
    getAgentModelDisplay({ model: 'openai/gpt-5.5', fallback_models: 'openai/gpt-5.4-mini' }),
    { primaryModel: 'openai/gpt-5.5', fallbackCount: 1 },
  );
});

test('getAgentModelDisplay ignores empty entries in legacy `fallback_models`', () => {
  assert.deepEqual(
    getAgentModelDisplay({
      model: 'openai/gpt-5.5',
      fallback_models: ['', 'openai/gpt-5.4-mini', ''],
    }),
    { primaryModel: 'openai/gpt-5.5', fallbackCount: 1 },
  );
});

test('getAgentModelDisplay prefers the canonical `models` chain over legacy fields', () => {
  // A migrated config should never carry both, but if it does, `models` wins.
  assert.deepEqual(
    getAgentModelDisplay({
      models: ['anthropic/claude-opus-5', 'openai/gpt-5.4'],
      model: 'stale-model',
      fallback_models: ['stale-fallback'],
    }),
    { primaryModel: 'anthropic/claude-opus-5', fallbackCount: 1 },
  );
});

test('getAgentModelDisplay returns no primary when the `models` chain has no valid entry', () => {
  assert.deepEqual(
    getAgentModelDisplay({ models: ['', { reasoning: 'high' }, null] }),
    { primaryModel: undefined, fallbackCount: 0 },
  );
});
