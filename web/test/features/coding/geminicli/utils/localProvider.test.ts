/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  GEMINI_CLI_LOCAL_PROVIDER_ID,
  isGeminiCliLocalProviderId,
  shouldLoadGeminiCliOfficialAccounts,
  shouldShowGeminiCliOfficialAccounts,
} from '../../../../../features/coding/geminicli/utils/localProvider.ts';

test('isGeminiCliLocalProviderId only matches the Gemini CLI local provider sentinel', () => {
  assert.equal(isGeminiCliLocalProviderId(GEMINI_CLI_LOCAL_PROVIDER_ID), true);
  assert.equal(isGeminiCliLocalProviderId('provider-1'), false);
  assert.equal(isGeminiCliLocalProviderId(undefined), false);
});

test('shouldLoadGeminiCliOfficialAccounts skips local temporary providers', () => {
  assert.equal(shouldLoadGeminiCliOfficialAccounts({ id: GEMINI_CLI_LOCAL_PROVIDER_ID }), false);
  assert.equal(shouldLoadGeminiCliOfficialAccounts({ id: 'provider-1' }), true);
});

test('shouldShowGeminiCliOfficialAccounts hides official account controls for local providers', () => {
  assert.equal(
    shouldShowGeminiCliOfficialAccounts(
      { id: GEMINI_CLI_LOCAL_PROVIDER_ID, category: 'official' },
      1,
    ),
    false,
  );
});

test('shouldShowGeminiCliOfficialAccounts preserves normal provider account visibility', () => {
  assert.equal(
    shouldShowGeminiCliOfficialAccounts({ id: 'provider-1', category: 'official' }, 0),
    true,
  );
  assert.equal(
    shouldShowGeminiCliOfficialAccounts({ id: 'provider-2', category: 'custom' }, 1),
    true,
  );
  assert.equal(
    shouldShowGeminiCliOfficialAccounts({ id: 'provider-3', category: 'custom' }, 0),
    false,
  );
});
