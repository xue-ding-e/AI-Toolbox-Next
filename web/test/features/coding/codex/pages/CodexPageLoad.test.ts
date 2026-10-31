/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { performance } from 'node:perf_hooks';
import { runInNewContext } from 'node:vm';
import { withTimeout } from '../../../../../utils/withTimeout.ts';
import * as accountRefresh from '../../../../../features/coding/codex/utils/codexAccountRefresh.ts';

// Execute the real page callback with synthetic Tauri results and React setters.
// This checks async state ownership, not DOM rendering or native account access.
const pageSource = readFileSync(new URL(
  '../../../../../features/coding/codex/pages/CodexPage.tsx', import.meta.url,
), 'utf8');
const startMarker = '  const loadConfig = React.useCallback(';
const endMarker = '\n  const loadFavoriteProviders =';
assert.equal(pageSource.split(startMarker).length, 2, 'load entry must be unique');
assert.equal(pageSource.split(endMarker).length, 2, 'next entry must be unique');
const start = pageSource.indexOf(startMarker);
const end = pageSource.indexOf(endMarker, start);
assert.ok(end > start, 'the complete load callback must be present');
const callbackSource = stripTypeScriptTypes(
  `${pageSource.slice(start, end)}\nloadConfig;`, { mode: 'strip' },
);

type Account = { id: string };
type Provider = { id: string; isApplied: boolean };
function deferred() {
  let resolve!: (value: Account[]) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<Account[]>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const flush = () => new Promise<void>((resolve) => setImmediate(resolve));

for (const outcome of ['resolve', 'reject']) {
  test(`older enrichment (${outcome}) cannot overwrite a newer provider load`, { timeout: 2_000 }, async () => {
    const old = deferred();
    const latest = deferred();
    let sequence = 0;
    const state = {
      providers: [] as Provider[], accounts: {} as Record<string, Account[]>,
      applied: '', token: 0, loading: false,
    };
    const loadConfig = runInNewContext(callbackSource, {
      React: { useCallback: (callback: unknown) => callback },
      officialAccountLoadGeneration: { current: 0 },
      officialAccountUsageRevision: { current: 0 },
      officialAccountUsageResults: { current: new Map() },
      performance, console: { info() {}, warn() {}, error() {} },
      setLoading(value: boolean) { state.loading = value; },
      setConfigPath() {}, setRootPathInfo() {},
      setProviders(value: Provider[]) { state.providers = value; },
      getCodexConfigFilePath: async () => 'synthetic-config',
      getCodexRootPathInfo: async () => ({ path: 'synthetic-root' }),
      listCodexProviders: async () => [{ id: ++sequence === 1 ? 'old-provider' : 'latest-provider', isApplied: true }],
      shouldLoadCodexOfficialAccounts: () => true,
      listCodexOfficialAccounts: (id: string) => id === 'old-provider' ? old.promise : latest.promise,
      withTimeout,
      ...accountRefresh,
      setOfficialAccountsByProviderId(value: Record<string, Account[]>) { state.accounts = value; },
      setOfficialAccountDetails(update: (previous: null) => unknown) { update(null); },
      setPluginPanelRefreshToken(update: (previous: number) => number) { state.token = update(state.token); },
      isLocalConfigId: () => false,
      setAppliedProviderId(value: string) { state.applied = value; },
      message: { error() {} }, t: (value: string) => value,
    }) as () => Promise<void>;
    try {
      await loadConfig();
      assert.equal(state.loading, false, 'base providers do not wait for enrichment');
      await loadConfig();
      latest.resolve([{ id: 'latest-account' }]);
      await flush();
      assert.equal(state.applied, 'latest-provider');
      assert.deepEqual(Object.keys(state.accounts), ['latest-provider']);
      const latestToken = state.token;
      if (outcome === 'resolve') old.resolve([{ id: 'old-account' }]);
      else old.reject(new Error('synthetic account read failure'));
      await flush();
      assert.deepEqual(state.providers.map((provider) => provider.id), ['latest-provider']);
      assert.deepEqual(Object.keys(state.accounts), ['latest-provider']);
      assert.equal(state.accounts['latest-provider'][0].id, 'latest-account');
      assert.equal(state.applied, 'latest-provider');
      assert.equal(state.token, latestToken);
    } finally {
      // Clear the real withTimeout timers even if an earlier assertion fails.
      old.resolve([]);
      latest.resolve([]);
      await flush();
    }
  });
}
