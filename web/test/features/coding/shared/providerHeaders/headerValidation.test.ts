import assert from 'node:assert/strict';
import test from 'node:test';

import {
  isValidHeaderName,
  isValidHeaderValue,
  validateHeaderEntry,
} from '../../../../../features/coding/shared/providerHeaders/headerValidation.ts';

// Byte-aligned with Rust `http::HeaderValue::from_str` (via parse_header_override_value):
// legal = b>=32 && b!=127 || b=='\t'. Only control chars (0x00-0x1F except \t, plus 0x7F)
// are illegal; visible ASCII and non-ASCII are legal. Control chars are built with
// String.fromCharCode to avoid embedding raw bytes in source.
const NUL = String.fromCharCode(0);
const DEL = String.fromCharCode(0x7f);

test('isValidHeaderValue treats empty / whitespace-only as valid (unset)', () => {
  assert.equal(isValidHeaderValue(''), true);
  assert.equal(isValidHeaderValue('   '), true);
});

test('isValidHeaderValue accepts visible ASCII, non-ASCII, and tab', () => {
  assert.equal(isValidHeaderValue('claude-cli/2.1.161'), true);
  assert.equal(isValidHeaderValue('claude-cli/1.0 中文'), true);
  assert.equal(isValidHeaderValue('claude\tcli'), true);
});

test('isValidHeaderValue rejects control characters', () => {
  assert.equal(isValidHeaderValue('claude\ncli'), false);
  assert.equal(isValidHeaderValue(`claude${NUL}cli`), false);
  assert.equal(isValidHeaderValue(`claude${DEL}cli`), false);
});

test('isValidHeaderName accepts valid token chars and rejects empty / illegal', () => {
  assert.equal(isValidHeaderName('User-Agent'), true);
  assert.equal(isValidHeaderName('X-Custom-123'), true);
  assert.equal(isValidHeaderName(''), false);
  assert.equal(isValidHeaderName('   '), false);
  assert.equal(isValidHeaderName('bad name'), false); // space illegal
  assert.equal(isValidHeaderName('bad\nname'), false);
});

test('validateHeaderEntry set requires name + value', () => {
  assert.deepEqual(validateHeaderEntry({ op: 'set', name: 'User-Agent', value: 'x', from: '', to: '' }), {
    meaningful: true,
    valid: true,
  });
  assert.equal(validateHeaderEntry({ op: 'set', name: '', value: 'x', from: '', to: '' }).meaningful, false);
  assert.equal(validateHeaderEntry({ op: 'set', name: 'X', value: '', from: '', to: '' }).meaningful, false);
  assert.equal(validateHeaderEntry({ op: 'set', name: 'bad name', value: 'x', from: '', to: '' }).valid, false);
});

test('validateHeaderEntry delete only requires name', () => {
  assert.deepEqual(validateHeaderEntry({ op: 'delete', name: 'X', value: '', from: '', to: '' }), {
    meaningful: true,
    valid: true,
  });
  assert.equal(validateHeaderEntry({ op: 'delete', name: '', value: '', from: '', to: '' }).meaningful, false);
});

test('validateHeaderEntry rename/copy require from + to', () => {
  assert.deepEqual(validateHeaderEntry({ op: 'rename', name: '', value: '', from: 'A', to: 'B' }), {
    meaningful: true,
    valid: true,
  });
  assert.equal(validateHeaderEntry({ op: 'copy', name: '', value: '', from: '', to: 'B' }).meaningful, false);
  assert.equal(validateHeaderEntry({ op: 'rename', name: '', value: '', from: 'A', to: '' }).meaningful, false);
});
