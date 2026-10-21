/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';
import {
  canPreserveImportedClaudeDesktopCredentials,
  isClaudeDesktopOfficialRestoreProvider,
} from '../../../../../features/coding/claudedesktop/utils/officialRestore.ts';

const baseUrl = 'https://fixture.example.invalid';
const token = 'synthetic-not-a-real-credential';
const makeProvider = (env: Record<string, unknown>) => ({
  id: 'synthetic-import',
  category: 'official',
  sourceProviderId: 'ccs:claude:synthetic-origin',
  settingsConfig: JSON.stringify({ env }),
});

test('restore mode distinguishes imported credentials from the raw official category', () => {
  assert.equal(isClaudeDesktopOfficialRestoreProvider(makeProvider({})), true);
  assert.equal(isClaudeDesktopOfficialRestoreProvider(makeProvider({ ANTHROPIC_BASE_URL: baseUrl })), false);
  assert.equal(isClaudeDesktopOfficialRestoreProvider(makeProvider({ ANTHROPIC_AUTH_TOKEN: token })), false);
  assert.equal(isClaudeDesktopOfficialRestoreProvider(makeProvider({ ANTHROPIC_BASE_URL: baseUrl, ANTHROPIC_AUTH_TOKEN: token })), false);
  assert.equal(isClaudeDesktopOfficialRestoreProvider({ ...makeProvider({}), category: 'custom' }), false);
});

test('restore predicate preserves the backend seed override, parsing and legacy-key semantics', () => {
  assert.equal(isClaudeDesktopOfficialRestoreProvider({ ...makeProvider({}), id: 'claude-desktop-official', category: 'custom', settingsConfig: 'not json' }), true);
  assert.equal(isClaudeDesktopOfficialRestoreProvider({ ...makeProvider({}), settingsConfig: 'not json' }), false);
  assert.equal(isClaudeDesktopOfficialRestoreProvider({ ...makeProvider({}), settingsConfig: '{}' }), true);
  assert.equal(isClaudeDesktopOfficialRestoreProvider(makeProvider({ ANTHROPIC_API_KEY: token })), true);
  assert.equal(isClaudeDesktopOfficialRestoreProvider(makeProvider({ ANTHROPIC_AUTH_TOKEN: '\u0085' })), true);
  assert.equal(isClaudeDesktopOfficialRestoreProvider(makeProvider({ ANTHROPIC_AUTH_TOKEN: '\uFEFF' })), false);
});

test('existing imported official URL-only and token-only pairs can remain unchanged', () => {
  assert.equal(canPreserveImportedClaudeDesktopCredentials(makeProvider({ ANTHROPIC_BASE_URL: baseUrl }), { baseUrl, apiKey: '' }, false), true);
  assert.equal(canPreserveImportedClaudeDesktopCredentials(makeProvider({ ANTHROPIC_AUTH_TOKEN: token }), { baseUrl: '', apiKey: token }, false), true);
  const fromClaude = { ...makeProvider({ ANTHROPIC_AUTH_TOKEN: token }), sourceProviderId: 'claude:synthetic-origin' };
  assert.equal(canPreserveImportedClaudeDesktopCredentials(fromClaude, { baseUrl: '', apiKey: token }, false), true);
});

test('any credential difference, including whitespace, retains both required rules', () => {
  const urlOnly = makeProvider({ ANTHROPIC_BASE_URL: baseUrl });
  for (const values of [
    { baseUrl: baseUrl + ' ', apiKey: '' },
    { baseUrl: ' ' + baseUrl, apiKey: '' },
    { baseUrl: baseUrl + '/changed', apiKey: '' },
    { baseUrl: '', apiKey: '' },
    { baseUrl, apiKey: ' ' },
    { baseUrl, apiKey: token },
  ]) assert.equal(canPreserveImportedClaudeDesktopCredentials(urlOnly, values, false), false);
  const tokenOnly = makeProvider({ ANTHROPIC_AUTH_TOKEN: token });
  assert.equal(canPreserveImportedClaudeDesktopCredentials(tokenOnly, { baseUrl: '', apiKey: token + ' ' }, false), false);
  assert.equal(canPreserveImportedClaudeDesktopCredentials(tokenOnly, { baseUrl: '', apiKey: '' }, false), false);
});

test('absence and empty strings match actual form initialization without trimming or treating undefined as empty', () => {
  for (const env of [
    { ANTHROPIC_BASE_URL: baseUrl },
    { ANTHROPIC_BASE_URL: baseUrl, ANTHROPIC_AUTH_TOKEN: '' },
  ]) {
    const provider = makeProvider(env);
    assert.equal(canPreserveImportedClaudeDesktopCredentials(provider, { baseUrl, apiKey: '' }, false), true);
    assert.equal(canPreserveImportedClaudeDesktopCredentials(provider, { baseUrl, apiKey: undefined }, false), false);
    assert.equal(canPreserveImportedClaudeDesktopCredentials(provider, { baseUrl, apiKey: null }, false), false);
  }
  const spaced = makeProvider({ ANTHROPIC_BASE_URL: ' ' + baseUrl, ANTHROPIC_AUTH_TOKEN: '' });
  assert.equal(canPreserveImportedClaudeDesktopCredentials(spaced, { baseUrl: ' ' + baseUrl, apiKey: '' }, false), true);
  assert.equal(canPreserveImportedClaudeDesktopCredentials(spaced, { baseUrl, apiKey: '' }, false), false);
});

test('new, copied, custom, restore, and unidentifiable rows receive no validation exemption', () => {
  const provider = makeProvider({ ANTHROPIC_BASE_URL: baseUrl });
  const values = { baseUrl, apiKey: '' };
  assert.equal(canPreserveImportedClaudeDesktopCredentials(undefined, values, false), false);
  assert.equal(canPreserveImportedClaudeDesktopCredentials(provider, values, true), false);
  assert.equal(canPreserveImportedClaudeDesktopCredentials({ ...provider, category: 'custom' }, values, false), false);
  assert.equal(canPreserveImportedClaudeDesktopCredentials({ ...provider, id: 'claude-desktop-official' }, values, false), false);
  for (const sourceProviderId of [undefined, '', 'ccs:claude:', 'claude:', 'ccs:codex:synthetic', 'unverified-origin']) {
    assert.equal(canPreserveImportedClaudeDesktopCredentials({ ...provider, sourceProviderId }, values, false), false);
  }
});

test('invalid or non-string stored credentials receive no validation exemption', () => {
  for (const env of [
    { ANTHROPIC_BASE_URL: baseUrl, ANTHROPIC_AUTH_TOKEN: null },
    { ANTHROPIC_BASE_URL: baseUrl, ANTHROPIC_AUTH_TOKEN: 5 },
    { ANTHROPIC_BASE_URL: baseUrl, ANTHROPIC_API_KEY: false },
  ]) assert.equal(canPreserveImportedClaudeDesktopCredentials(makeProvider(env), { baseUrl, apiKey: '' }, false), false);
  assert.equal(canPreserveImportedClaudeDesktopCredentials({ ...makeProvider({}), settingsConfig: 'bad json' }, { baseUrl: '', apiKey: '' }, false), false);
});
