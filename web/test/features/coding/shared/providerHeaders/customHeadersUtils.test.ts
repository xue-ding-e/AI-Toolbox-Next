import assert from 'node:assert/strict';
import test from 'node:test';

import {
  emptyHeaderEntry,
  getCustomHeadersFromMeta,
  mergeCustomHeadersIntoMeta,
} from '../../../../../features/coding/shared/providerHeaders/customHeadersUtils.ts';

test('getCustomHeadersFromMeta reports enabled when a non-empty array is present', () => {
  assert.deepEqual(
    getCustomHeadersFromMeta({
      customHeaders: [{ op: 'set', name: 'User-Agent', value: 'claude-cli/2.1.161', from: '', to: '' }],
    }),
    {
      enabled: true,
      headers: [{ op: 'set', name: 'User-Agent', value: 'claude-cli/2.1.161', from: '', to: '' }],
    },
  );
});
test('getCustomHeadersFromMeta surfaces legacy customUserAgent as a set row', () => {
  assert.deepEqual(
    getCustomHeadersFromMeta({ customUserAgent: 'claude-cli/2.1.161' }),
    {
      enabled: true,
      headers: [{ op: 'set', name: 'User-Agent', value: 'claude-cli/2.1.161', from: '', to: '' }],
    },
  );
});

test('getCustomHeadersFromMeta surfaces legacy snake custom_user_agent', () => {
  const state = getCustomHeadersFromMeta({ custom_user_agent: 'Kilo-Code/1.0' } as never);
  assert.equal(state.enabled, true);
  assert.deepEqual(state.headers, [
    { op: 'set', name: 'User-Agent', value: 'Kilo-Code/1.0', from: '', to: '' },
  ]);
});

test('getCustomHeadersFromMeta reports disabled and seeds a blank row when unset', () => {
  const fromUndefined = getCustomHeadersFromMeta(undefined);
  assert.equal(fromUndefined.enabled, false);
  assert.equal(fromUndefined.headers.length, 1);
  assert.deepEqual(fromUndefined.headers[0], emptyHeaderEntry());

  const fromEmpty = getCustomHeadersFromMeta({ customHeaders: [] });
  assert.equal(fromEmpty.enabled, false);
  assert.equal(fromEmpty.headers.length, 1);
});

test('getCustomHeadersFromMeta normalizes unknown ops back to set', () => {
  const { headers } = getCustomHeadersFromMeta({
    customHeaders: [{ op: 'bogus', name: 'X-Foo', value: 'bar', from: '', to: '' } as never],
  });
  assert.equal(headers[0].op, 'set');
});

test('mergeCustomHeadersIntoMeta writes trimmed set rows when enabled', () => {
  assert.deepEqual(
    mergeCustomHeadersIntoMeta(undefined, {
      enabled: true,
      headers: [{ op: 'set', name: '  User-Agent  ', value: '  claude-cli/1.0  ', from: '', to: '' }],
    }),
    { customHeaders: [{ op: 'set', name: 'User-Agent', value: 'claude-cli/1.0', from: '', to: '' }] },
  );
});

test('mergeCustomHeadersIntoMeta removes legacy customUserAgent', () => {
  assert.deepEqual(
    mergeCustomHeadersIntoMeta(
      { providerType: 'anthropic', customUserAgent: 'claude-cli/2.1.161' } as never,
      { enabled: false, headers: [] },
    ),
    { providerType: 'anthropic' },
  );
});

test('mergeCustomHeadersIntoMeta removes legacy snake custom_user_agent', () => {
  assert.equal(
    mergeCustomHeadersIntoMeta(
      { custom_user_agent: 'Kilo-Code/1.0' } as never,
      { enabled: false, headers: [] },
    ),
    undefined,
  );
});

test('mergeCustomHeadersIntoMeta replaces legacy customUserAgent with customHeaders when enabled', () => {
  assert.deepEqual(
    mergeCustomHeadersIntoMeta(
      { providerType: 'anthropic', customUserAgent: 'claude-cli/2.1.161' } as never,
      { enabled: true, headers: [{ op: 'set', name: 'User-Agent', value: 'Kilo-Code/1.0', from: '', to: '' }] },
    ),
    { providerType: 'anthropic', customHeaders: [{ op: 'set', name: 'User-Agent', value: 'Kilo-Code/1.0', from: '', to: '' }] },
  );
});

test('mergeCustomHeadersIntoMeta clears the key when disabled', () => {
  assert.deepEqual(
    mergeCustomHeadersIntoMeta(
      { providerType: 'anthropic', customHeaders: [{ op: 'set', name: 'X', value: '1', from: '', to: '' }] } as never,
      { enabled: false, headers: [] },
    ),
    { providerType: 'anthropic' },
  );
});

test('mergeCustomHeadersIntoMeta drops meaningless rows (set without value)', () => {
  assert.equal(
    mergeCustomHeadersIntoMeta(
      { customHeaders: [{ op: 'set', name: 'X', value: '', from: '', to: '' }] },
      { enabled: true, headers: [{ op: 'set', name: 'X', value: '', from: '', to: '' }] },
    ),
    undefined,
  );
});

test('mergeCustomHeadersIntoMeta keeps rename rows with from/to', () => {
  assert.deepEqual(
    mergeCustomHeadersIntoMeta(undefined, {
      enabled: true,
      headers: [{ op: 'rename', name: '', value: '', from: 'X-Old', to: 'X-New' }],
    }),
    { customHeaders: [{ op: 'rename', name: '', value: '', from: 'X-Old', to: 'X-New' }] },
  );
});

test('mergeCustomHeadersIntoMeta preserves unrelated meta fields', () => {
  assert.deepEqual(
    mergeCustomHeadersIntoMeta(
      { providerType: 'openrouter', costMultiplier: '0.5' } as never,
      {
        enabled: true,
        headers: [{ op: 'set', name: 'User-Agent', value: 'Kilo-Code/1.0', from: '', to: '' }],
      },
    ),
    {
      providerType: 'openrouter',
      costMultiplier: '0.5',
      customHeaders: [{ op: 'set', name: 'User-Agent', value: 'Kilo-Code/1.0', from: '', to: '' }],
    },
  );
});
