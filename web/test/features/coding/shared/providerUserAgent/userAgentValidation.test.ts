import assert from 'node:assert/strict';
import test from 'node:test';

import { isValidUserAgentHeader } from '../../../../../features/coding/shared/providerUserAgent/userAgentValidation.ts';

// Byte-aligned with Rust `http::HeaderValue::from_str` (via parse_custom_user_agent):
// legal = b>=32 && b!=127 || b=='\t'. Only control chars (0x00-0x1F except \t, plus 0x7F)
// are illegal; visible ASCII and non-ASCII are legal. Control chars are built with
// String.fromCharCode to avoid embedding raw bytes in source.
const NUL = String.fromCharCode(0);
const DEL = String.fromCharCode(0x7f);

test('treats empty / whitespace-only as valid (unset)', () => {
  assert.equal(isValidUserAgentHeader(''), true);
  assert.equal(isValidUserAgentHeader('   '), true);
});

test('accepts visible ASCII (trimmed)', () => {
  assert.equal(isValidUserAgentHeader('claude-cli/2.1.161'), true);
  assert.equal(isValidUserAgentHeader('  claude-cli/2.1.161  '), true);
});

test('accepts non-ASCII — matches backend HeaderValue byte rule', () => {
  assert.equal(isValidUserAgentHeader('claude-cli/1.0 中文'), true);
});

test('accepts internal tab', () => {
  assert.equal(isValidUserAgentHeader('claude\tcli'), true);
});

test('rejects control characters (newline / null / DEL)', () => {
  assert.equal(isValidUserAgentHeader('claude\ncli'), false);
  assert.equal(isValidUserAgentHeader(`claude${NUL}cli`), false);
  assert.equal(isValidUserAgentHeader(`claude${DEL}cli`), false);
});
