import assert from 'node:assert/strict';
import test from 'node:test';

import {
  getCustomUserAgentFromMeta,
  mergeCustomUserAgentIntoMeta,
} from '../../../../../features/coding/shared/providerUserAgent/customUserAgentUtils.ts';

test('getCustomUserAgentFromMeta reports enabled when a value is present', () => {
  assert.deepEqual(getCustomUserAgentFromMeta({ customUserAgent: 'claude-cli/2.1.161' }), {
    enabled: true,
    value: 'claude-cli/2.1.161',
  });
});

test('getCustomUserAgentFromMeta reports disabled when unset or whitespace-only', () => {
  assert.deepEqual(getCustomUserAgentFromMeta({ customUserAgent: '   ' }), {
    enabled: false,
    value: '   ',
  });
  assert.deepEqual(getCustomUserAgentFromMeta(undefined), {
    enabled: false,
    value: '',
  });
  assert.deepEqual(getCustomUserAgentFromMeta(null), {
    enabled: false,
    value: '',
  });
});

test('mergeCustomUserAgentIntoMeta writes trimmed value when enabled', () => {
  assert.deepEqual(
    mergeCustomUserAgentIntoMeta(undefined, { enabled: true, value: '  claude-cli/1.0  ' }),
    { customUserAgent: 'claude-cli/1.0' },
  );
});

test('mergeCustomUserAgentIntoMeta clears the key when disabled', () => {
  assert.deepEqual(
    mergeCustomUserAgentIntoMeta(
      { providerType: 'anthropic', customUserAgent: 'old' } as never,
      { enabled: false, value: '' },
    ),
    { providerType: 'anthropic' },
  );
});

test('mergeCustomUserAgentIntoMeta ignores empty value even when enabled', () => {
  assert.equal(
    mergeCustomUserAgentIntoMeta({ customUserAgent: 'old' }, { enabled: true, value: '   ' }),
    undefined,
  );
});

test('mergeCustomUserAgentIntoMeta preserves unrelated meta fields', () => {
  assert.deepEqual(
    mergeCustomUserAgentIntoMeta(
      { providerType: 'openrouter', costMultiplier: '0.5' } as never,
      { enabled: true, value: 'Kilo-Code/1.0' },
    ),
    { providerType: 'openrouter', costMultiplier: '0.5', customUserAgent: 'Kilo-Code/1.0' },
  );
});
