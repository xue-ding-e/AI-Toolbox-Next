import assert from 'node:assert/strict';
import test from 'node:test';

import {
  emptyModelRewriteEntry,
  getModelRewritesFromMeta,
  mergeModelRewritesIntoMeta,
} from '../../../../../features/coding/shared/providerModelRewrites/modelRewritesUtils.ts';

test('getModelRewritesFromMeta reports enabled when a non-empty array is present', () => {
  assert.deepEqual(
    getModelRewritesFromMeta({
      modelRewrites: [{ from: 'gpt-5-luna', to: 'gpt-5-mini' }],
    }),
    {
      enabled: true,
      rewrites: [{ from: 'gpt-5-luna', to: 'gpt-5-mini' }],
    },
  );
});

test('getModelRewritesFromMeta reads snake_case model_rewrites array', () => {
  const state = getModelRewritesFromMeta({
    model_rewrites: [{ from: 'gemini-2.5-flash-lite', to: 'gemini-2.5-flash' }],
  } as never);
  assert.equal(state.enabled, true);
  assert.deepEqual(state.rewrites, [
    { from: 'gemini-2.5-flash-lite', to: 'gemini-2.5-flash' },
  ]);
});

test('getModelRewritesFromMeta reports disabled and seeds a blank row when unset', () => {
  const fromUndefined = getModelRewritesFromMeta(undefined);
  assert.equal(fromUndefined.enabled, false);
  assert.equal(fromUndefined.rewrites.length, 1);
  assert.deepEqual(fromUndefined.rewrites[0], emptyModelRewriteEntry());

  const fromEmpty = getModelRewritesFromMeta({ modelRewrites: [] });
  assert.equal(fromEmpty.enabled, false);
  assert.equal(fromEmpty.rewrites.length, 1);
});

test('mergeModelRewritesIntoMeta round-trips enabled rules and trims values', () => {
  const state = {
    enabled: true,
    rewrites: [
      { from: ' gpt-5-luna ', to: ' gpt-5-mini ' },
      { from: '', to: 'ignored' },
      { from: 'only-from', to: '' },
    ],
  };
  const meta = mergeModelRewritesIntoMeta(undefined, state);
  assert.deepEqual(meta, {
    modelRewrites: [{ from: 'gpt-5-luna', to: 'gpt-5-mini' }],
  });

  const readBack = getModelRewritesFromMeta(meta);
  assert.equal(readBack.enabled, true);
  assert.deepEqual(readBack.rewrites, [{ from: 'gpt-5-luna', to: 'gpt-5-mini' }]);
});

test('mergeModelRewritesIntoMeta drops the key when disabled or blank', () => {
  const disabled = mergeModelRewritesIntoMeta(
    { modelRewrites: [{ from: 'a', to: 'b' }] },
    { enabled: false, rewrites: [{ from: 'a', to: 'b' }] },
  );
  assert.equal(disabled, undefined);

  const blankRows = mergeModelRewritesIntoMeta(
    undefined,
    { enabled: true, rewrites: [{ from: '  ', to: '' }] },
  );
  assert.equal(blankRows, undefined);
});

test('mergeModelRewritesIntoMeta keeps unrelated meta keys from other subsets', () => {
  const meta = mergeModelRewritesIntoMeta(
    {
      customHeaders: [{ op: 'set', name: 'User-Agent', value: 'x', from: '', to: '' }],
    } as never,
    { enabled: true, rewrites: [{ from: 'a', to: 'b' }] },
  );
  assert.deepEqual((meta as never as { modelRewrites: unknown }).modelRewrites, [
    { from: 'a', to: 'b' },
  ]);
  assert.deepEqual(
    (meta as never as { customHeaders: unknown }).customHeaders,
    [{ op: 'set', name: 'User-Agent', value: 'x', from: '', to: '' }],
  );
});
