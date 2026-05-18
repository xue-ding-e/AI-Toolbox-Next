/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  CODEX_LOCAL_PROVIDER_ID,
  isCodexLocalProviderId,
  shouldLoadCodexOfficialAccounts,
  shouldShowCodexOfficialAccounts,
} from '../../../../../features/coding/codex/utils/localProvider.ts';

test('isCodexLocalProviderId only matches the Codex local provider sentinel', () => {
  assert.equal(isCodexLocalProviderId(CODEX_LOCAL_PROVIDER_ID), true);
  assert.equal(isCodexLocalProviderId('provider-1'), false);
  assert.equal(isCodexLocalProviderId(undefined), false);
});

test('shouldLoadCodexOfficialAccounts skips local temporary providers', () => {
  assert.equal(shouldLoadCodexOfficialAccounts({ id: CODEX_LOCAL_PROVIDER_ID }), false);
  assert.equal(shouldLoadCodexOfficialAccounts({ id: 'provider-1' }), true);
});

test('shouldShowCodexOfficialAccounts hides official account controls for local providers', () => {
  assert.equal(
    shouldShowCodexOfficialAccounts(
      { id: CODEX_LOCAL_PROVIDER_ID, category: 'official' },
      1,
    ),
    false,
  );
});

test('shouldShowCodexOfficialAccounts preserves normal provider account visibility', () => {
  assert.equal(
    shouldShowCodexOfficialAccounts({ id: 'provider-1', category: 'official' }, 0),
    true,
  );
  assert.equal(
    shouldShowCodexOfficialAccounts({ id: 'provider-2', category: 'custom' }, 1),
    true,
  );
  assert.equal(
    shouldShowCodexOfficialAccounts({ id: 'provider-3', category: 'custom' }, 0),
    false,
  );
});
