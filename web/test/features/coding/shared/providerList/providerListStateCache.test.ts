import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';

let instance = 0;
async function loadIsolatedCache(rejectRead = false, deferredRead = false) {
  const source = await readFile(new URL('../../../../../features/coding/shared/providerList/providerListStateCache.ts', import.meta.url), 'utf8');
  // Replace only the IPC boundary, not the production cache implementation.
  const importPattern = /import\s*\{[\s\S]*?\}\s*from '@\/services\/providerListApi';/;
  assert.equal((source.match(importPattern) ?? []).length, 1);
  const fixture = source.replace(importPattern, `
    export let reads = 0;
    export let resolveRead;
    export let rejectPendingRead;
    const pending = new Promise((resolve, reject) => { resolveRead = resolve; rejectPendingRead = reject; });
    const getProviderListState = async () => {
      reads++;
      ${deferredRead ? "return pending;" : rejectRead ? "throw new Error('synthetic unavailable IPC');" : "return { sort_modes: { geminicli: 'custom' }, last_used: {}, view_modes: { geminicli: 'list' } };"}
    };
  `);
  const js = stripTypeScriptTypes(fixture);
  return import('data:text/javascript;base64,' + Buffer.from(js).toString('base64') + '#' + instance++);
}

test('later callers receive patched view, sort and recent-use state without another IPC read', async () => {
  const cache = await loadIsolatedCache();
  const initial = await cache.loadProviderListState();
  cache.patchCachedProviderListState({ view_modes: { geminicli: 'card' } });
  cache.patchCachedProviderListState({ sort_modes: { geminicli: 'name' }, last_used: { 'geminicli/provider': '123' } });
  const remounted = await cache.loadProviderListState();
  assert.equal(remounted.view_modes.geminicli, 'card');
  assert.equal(remounted.sort_modes.geminicli, 'name');
  assert.equal(remounted.last_used['geminicli/provider'], '123');
  assert.equal(remounted, cache.readCachedProviderListState());
  assert.equal(initial.view_modes.geminicli, 'list');
  assert.equal(cache.reads, 1);
});

test('concurrent initial callers share one read and receive the same state', async () => {
  const cache = await loadIsolatedCache();
  const [first, second] = await Promise.all([cache.loadProviderListState(), cache.loadProviderListState()]);
  assert.equal(first, second);
  assert.equal(first.view_modes.geminicli, 'list');
  assert.equal(cache.reads, 1);
});

test('a failed initial read still permits later local preference patches', async () => {
  const cache = await loadIsolatedCache(true);
  const initial = await cache.loadProviderListState();
  assert.deepEqual(initial, { sort_modes: {}, last_used: {}, view_modes: {} });
  cache.patchCachedProviderListState({ view_modes: { grok: 'card' } });
  assert.equal((await cache.loadProviderListState()).view_modes.grok, 'card');
  assert.equal(cache.reads, 1);
});

for (const outcome of ['success', 'failure']) {
  test(`local changes during initial hydration survive ${outcome}`, async () => {
    const cache = await loadIsolatedCache(false, true);
    const hydration = cache.loadProviderListState();
    cache.patchCachedProviderListState({ view_modes: { codex: 'card' } });
    cache.patchCachedProviderListState({
      sort_modes: { codex: 'name' },
      last_used: { 'codex:provider': '2026-10-20T08:00:00Z' },
    });
    if (outcome === 'success') {
      cache.resolveRead({
        view_modes: { codex: 'list', opencode: 'card' },
        sort_modes: { codex: 'custom', opencode: 'recent' },
        last_used: { 'codex:provider': 'older', 'opencode:provider': 'untouched' },
      });
    } else {
      cache.rejectPendingRead(new Error('synthetic delayed IPC failure'));
    }
    const state = await hydration;
    assert.equal(state.view_modes.codex, 'card');
    assert.equal(state.sort_modes.codex, 'name');
    assert.equal(state.last_used['codex:provider'], '2026-10-20T08:00:00Z');
    if (outcome === 'success') {
      assert.equal(state.view_modes.opencode, 'card');
      assert.equal(state.sort_modes.opencode, 'recent');
      assert.equal(state.last_used['opencode:provider'], 'untouched');
    }
    assert.equal(await cache.loadProviderListState(), state);
    assert.equal(cache.reads, 1);
  });
}
