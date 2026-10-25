/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { runInNewContext } from 'node:vm';
import type { CodexOfficialAccount } from '../../../../../types/codex.ts';
import {
  matchesOfficialAccountRefresh,
  markOfficialAccountRefreshFailed,
  reconcileOfficialAccountUsage,
  type OfficialAccountUsageResult,
} from '../../../../../features/coding/codex/utils/codexAccountRefresh.ts';

// Execute the real page handler, not a copied refresh algorithm. This small
// integration test mocks React state setters and the Tauri boundary; it does
// not mount React, render a browser, or exercise the desktop backend.
const pageSource = readFileSync(new URL(
  '../../../../../features/coding/codex/pages/CodexPage.tsx', import.meta.url,
), 'utf8');
const startMarker = '  const handleRefreshOfficialAccount = async (';
const endMarker = '\n  const handleViewOfficialAccountDetails =';
assert.equal(pageSource.split(startMarker).length, 2, 'refresh entry must be unique');
assert.equal(pageSource.split(endMarker).length, 2, 'next entry must be unique');
const start = pageSource.indexOf(startMarker);
const end = pageSource.indexOf(endMarker, start);
assert.ok(end > start, 'the complete refresh handler must be present');
const handlerSource = stripTypeScriptTypes(
  `${pageSource.slice(start, end)}\nhandleRefreshOfficialAccount;`,
  { mode: 'strip' },
);

const provider = { id: 'official-synthetic' };
const local: CodexOfficialAccount = {
  id: '__local__', providerId: provider.id, accountId: 'synthetic-login-a',
  name: 'Synthetic account', email: 'a@example.invalid', kind: 'local',
  isApplied: false, isVirtual: true, createdAt: '', updatedAt: '',
  credits: { hasCredits: true, balance: 42 }, limitWeeklyText: '75%',
  lastLimitsFetchedAt: '2026-10-24T12:00:00Z',
};
type Details = { provider: typeof provider; account: CodexOfficialAccount } | null;
type Update<T> = T | ((previous: T) => T);
const update = <T>(previous: T, next: Update<T>): T =>
  typeof next === 'function' ? (next as (previous: T) => T)(previous) : next;

function harness(initial = local) {
  const state = {
    rows: { [provider.id]: [initial] },
    details: { provider, account: initial } as Details,
    refreshing: null as string | null,
    cache: { current: new Map<string, OfficialAccountUsageResult>() },
    revision: { current: 0 },
    errors: [] as string[],
    successes: [] as string[],
  };
  const requests: Array<{
    resolve: (account: CodexOfficialAccount) => void;
    reject: (reason: Error) => void;
  }> = [];
  const refresh = runInNewContext(handlerSource, {
    Error,
    console: { error: () => undefined },
    t: (key: string) => key,
    message: {
      error: (value: string) => state.errors.push(value),
      success: (value: string) => state.successes.push(value),
    },
    officialAccountRefreshVersions: { current: new Map<string, number>() },
    officialAccountUsageResults: state.cache,
    officialAccountUsageRevision: state.revision,
    matchesOfficialAccountRefresh,
    markOfficialAccountRefreshFailed,
    reconcileOfficialAccountUsage,
    setRefreshingOfficialAccountId: (next: Update<string | null>) => {
      state.refreshing = update(state.refreshing, next);
    },
    setOfficialAccountsByProviderId: (next: Update<typeof state.rows>) => {
      state.rows = update(state.rows, next);
    },
    setOfficialAccountDetails: (next: Update<Details>) => {
      state.details = update(state.details, next);
    },
    refreshCodexOfficialAccountLimits: (providerId: string, accountId: string) => {
      assert.equal(providerId, provider.id);
      assert.equal(accountId, initial.id);
      return new Promise<CodexOfficialAccount>((resolve, reject) => {
        requests.push({ resolve, reject });
      });
    },
  }) as (provider: { id: string }, account: CodexOfficialAccount) => Promise<void>;
  return { state, requests, refresh: () => refresh(provider, initial) };
}

for (const virtual of [false, true]) {
  test(`page failure preserves the newer ${virtual ? 'virtual' : 'saved'} row and detail usage`, async () => {
    const original = { ...local, id: virtual ? '__local__' : 'saved-a', isVirtual: virtual };
    const h = harness(original);
    const pending = h.refresh();
    const fresh = {
      ...original, name: 'Renamed while waiting', limitWeeklyText: '90%',
      lastLimitsFetchedAt: '2026-10-24T13:00:00Z',
    };
    h.state.rows[provider.id] = [fresh];
    h.state.details = { provider, account: fresh };
    h.requests[0].reject(new Error('synthetic timeout'));
    await pending;
    for (const current of [h.state.rows[provider.id][0], h.state.details?.account]) {
      assert.equal(current?.limitWeeklyText, '90%', 'failure must not restore click-time windows');
      assert.equal(current?.lastLimitsFetchedAt, fresh.lastLimitsFetchedAt);
      assert.equal(current?.name, fresh.name);
      assert.equal(current?.credits, undefined);
      assert.equal(current?.lastError, 'synthetic timeout');
    }
    assert.equal(h.state.cache.current.size, 0, 'failure is not a cached success');
    assert.equal(h.state.revision.current, 0, 'failure must not advance the usage revision');
    assert.equal(h.state.refreshing, null);
  });
}

test('page failure removes old virtual success so list reload cannot resurrect credits', async () => {
  const h = harness();
  const key = `${provider.id}:${local.id}`;
  h.state.cache.current.set(key, { account: local, revision: 1 });
  const pending = h.refresh();
  h.requests[0].reject(new Error('synthetic rejection'));
  await pending;
  assert.equal(h.state.cache.current.has(key), false);
  const listed = { ...local, credits: undefined, limitWeeklyText: '95%' };
  const reloaded = reconcileOfficialAccountUsage(listed, h.state.cache.current.get(key), 2);
  assert.equal(reloaded, listed);
  assert.equal(reloaded.credits, undefined);
  assert.equal(reloaded.lastError, undefined, 'failure must not be replayed onto future list reads');
});

test('page failure does not mark a switched local login or reopen closed details', async () => {
  const h = harness();
  const pending = h.refresh();
  const switched = { ...local, accountId: 'synthetic-login-b', email: 'b@example.invalid' };
  h.state.rows[provider.id] = [switched];
  h.state.details = null;
  h.requests[0].reject(new Error('old request failed'));
  await pending;
  assert.equal(h.state.rows[provider.id][0], switched);
  assert.equal(h.state.details, null);
});

test('superseded page failure cannot evict a newer successful refresh', async () => {
  const h = harness();
  const first = h.refresh();
  const second = h.refresh();
  const recovered = { ...local, credits: { hasCredits: true, balance: 7 } };
  h.requests[1].resolve(recovered);
  await second;
  const cached = h.state.cache.current.get(`${provider.id}:${local.id}`);
  h.requests[0].reject(new Error('superseded timeout'));
  await first;
  assert.equal(h.state.cache.current.get(`${provider.id}:${local.id}`), cached);
  assert.equal(h.state.rows[provider.id][0].credits?.balance, 7);
  assert.equal(h.state.errors.length, 0);
  assert.equal(h.state.successes.length, 1);
});
