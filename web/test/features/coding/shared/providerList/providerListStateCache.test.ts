import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';

let instance = 0;
async function loadIsolatedCache(rejectRead = false) {
  const source = await readFile(new URL('../../../../../features/coding/shared/providerList/providerListStateCache.ts', import.meta.url), 'utf8');
  // Replace only the IPC boundary, not the production cache implementation.
  const importPattern = /import\s*\{[\s\S]*?\}\s*from '@\/services\/providerListApi';/;
  assert.equal((source.match(importPattern) ?? []).length, 1);
  const fixture = source.replace(importPattern, `
    export let reads = 0;
    const getProviderListState = async () => {
      reads++;
      ${rejectRead ? "throw new Error('synthetic unavailable IPC');" : "return { sort_modes: { geminicli: 'custom' }, last_used: {}, view_modes: { geminicli: 'list' } };"}
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
