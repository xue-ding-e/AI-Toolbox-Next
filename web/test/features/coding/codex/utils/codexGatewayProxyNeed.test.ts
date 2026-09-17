/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  codexProviderNeedsGatewayProxy,
  primaryCodexProviderNeedsGatewayProxy,
} from '../../../../../features/coding/codex/utils/codexGatewayProxyNeed.ts';
import { isCodexLocalProviderId } from '../../../../../features/coding/codex/utils/localProvider.ts';
import type { CodexProvider } from '../../../../../types/codex.ts';

/**
 * The Codex page, the provider card and the aggregate settings panel all call
 * this helper, so it is the only place the "must Codex keep talking to the
 * gateway?" decision is made. Codex speaks `openai_responses` natively; any
 * other resolved protocol has to be converted.
 */
const buildProvider = (overrides: Partial<CodexProvider> = {}): CodexProvider => ({
  id: 'provider-1',
  name: 'Provider 1',
  category: 'custom',
  settingsConfig: JSON.stringify({ config: 'model = "gpt-5"\n' }),
  createdAt: '2026-01-01T00:00:00.000Z',
  updatedAt: '2026-01-01T00:00:00.000Z',
  ...overrides,
});

test('codexProviderNeedsGatewayProxy only accepts the native Responses wire', () => {
  assert.equal(
    codexProviderNeedsGatewayProxy(buildProvider({ meta: { apiFormat: 'openai_responses' } })),
    false,
  );
  assert.equal(
    codexProviderNeedsGatewayProxy(buildProvider({ meta: { apiFormat: 'openai_chat' } })),
    true,
  );
  assert.equal(
    codexProviderNeedsGatewayProxy(buildProvider({ meta: { apiFormat: 'anthropic_messages' } })),
    true,
  );
});

test('codexProviderNeedsGatewayProxy reports no proxy when nothing declares a protocol', () => {
  // No meta signal and no protocol hint in config.toml. `providerNeedsGatewayProxy`
  // answers false for an unknown target, so a provider that declares nothing is
  // treated as usable directly rather than as needing conversion.
  assert.equal(codexProviderNeedsGatewayProxy(buildProvider()), false);
});

test('codexProviderNeedsGatewayProxy reads wire_api and base_url out of config.toml', () => {
  const wireApiProvider = buildProvider({
    settingsConfig: JSON.stringify({ config: 'wire_api = "chat"\n' }),
  });
  assert.equal(codexProviderNeedsGatewayProxy(wireApiProvider), true);

  const chatCompletionsBaseUrlProvider = buildProvider({
    settingsConfig: JSON.stringify({
      config: 'base_url = "https://api.example.com/v1/chat/completions"\n',
    }),
  });
  assert.equal(codexProviderNeedsGatewayProxy(chatCompletionsBaseUrlProvider), true);

  const responsesBaseUrlProvider = buildProvider({
    settingsConfig: JSON.stringify({ config: 'base_url = "https://api.example.com/v1"\n' }),
  });
  assert.equal(codexProviderNeedsGatewayProxy(responsesBaseUrlProvider), false);
});

test('codexProviderNeedsGatewayProxy prefers meta over config.toml', () => {
  // `firstGatewayApiFormat` walks meta first, so an explicit meta declaration
  // wins over the protocol implied by config.toml.
  const provider = buildProvider({
    meta: { apiFormat: 'openai_responses' },
    settingsConfig: JSON.stringify({ config: 'wire_api = "chat"\n' }),
  });
  assert.equal(codexProviderNeedsGatewayProxy(provider), false);
});

test('primaryCodexProviderNeedsGatewayProxy resolves the id against the provider list', () => {
  const providers = [
    buildProvider({ id: 'needs-proxy', meta: { apiFormat: 'openai_chat' } }),
    buildProvider({ id: 'direct', meta: { apiFormat: 'openai_responses' } }),
  ];

  assert.deepEqual(
    primaryCodexProviderNeedsGatewayProxy(providers, 'needs-proxy', isCodexLocalProviderId),
    { needsProxy: true, reason: 'protocol' },
  );
  assert.deepEqual(
    primaryCodexProviderNeedsGatewayProxy(providers, 'direct', isCodexLocalProviderId),
    { needsProxy: false, reason: null },
  );
  // No primary yet (mode not engaged) or a primary that has since been deleted:
  // there is nothing to guard, so the panel must not block restore-direct.
  assert.deepEqual(
    primaryCodexProviderNeedsGatewayProxy(providers, null, isCodexLocalProviderId),
    { needsProxy: false, reason: null },
  );
  assert.deepEqual(
    primaryCodexProviderNeedsGatewayProxy(providers, 'deleted', isCodexLocalProviderId),
    { needsProxy: false, reason: null },
  );
});

test('primaryCodexProviderNeedsGatewayProxy exempts official and local providers', () => {
  const providers = [
    buildProvider({ id: 'official-1', category: 'official', meta: { apiFormat: 'openai_chat' } }),
    buildProvider({ id: '__local__', meta: { apiFormat: 'openai_chat' } }),
  ];

  // Both kinds bypass the gateway by construction, so a protocol mismatch on
  // record must not be read as "the primary needs the takeover".
  assert.deepEqual(
    primaryCodexProviderNeedsGatewayProxy(providers, 'official-1', isCodexLocalProviderId),
    { needsProxy: false, reason: null },
  );
  assert.deepEqual(
    primaryCodexProviderNeedsGatewayProxy(providers, '__local__', isCodexLocalProviderId),
    { needsProxy: false, reason: null },
  );
});
