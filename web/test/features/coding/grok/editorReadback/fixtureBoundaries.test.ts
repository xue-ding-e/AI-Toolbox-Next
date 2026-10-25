import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

// Exercise the actual fixture boundaries, not production handlers or a copied
// mock implementation. These tests do not establish that a browser case ran.
const ipcSource = readFileSync(new URL('./fixtures/boundary.mjs', import.meta.url), 'utf8')
  .replace("import { mockIPC } from '@tauri-apps/api/mocks';", '')
  .replace('export const boundary =', 'const boundary =');
type Provider = { id: string; category: string; isApplied: boolean; settingsConfig: string };
type Snapshot = { providers: Provider[]; saveCount: number; forbidden: string[]; writes: unknown[] };
function ipc(scenario = 'save') {
  let handler: (command: string, args?: unknown) => Promise<unknown> = async () => { throw new Error('IPC not installed'); };
  const window = {};
  const document = { createElement: () => ({}), head: { append() {} }, addEventListener() {} };
  const make = new Function('mockIPC', 'window', 'location', 'document', ipcSource + '\nreturn boundary;');
  const boundary = make((callback: typeof handler) => { handler = callback; }, window,
    { search: '?scenario=' + scenario + '&caseToken=synthetic' }, document) as {
      snapshot(): Snapshot; releaseSave(): void;
    };
  return { call: (command: string, args?: unknown) => handler(command, args), ...boundary };
}

test('fixture refuses unknown scenarios before installing a permissive fallback', () => {
  assert.throws(() => ipc('unknown'), /Unknown Grok fixture scenario/);
});

test('fixture refuses unexpected IPC, account IDs and live model fetching', async () => {
  const run = ipc();
  await assert.rejects(run.call('unexpected-command'), /Forbidden/);
  await assert.rejects(run.call('list_grok_official_accounts', { providerId: 'another-id' }), /Forbidden/);
  await assert.rejects(run.call('fetch_provider_models'), /Forbidden/);
  assert.equal(run.snapshot().forbidden.length, 3);
  assert.equal(run.snapshot().saveCount, 0);
});

test('fixture refuses applied providers and credentials without recording a write', async () => {
  const run = ipc();
  const provider = run.snapshot().providers[0];
  await assert.rejects(run.call('update_grok_provider', { provider: { ...provider, isApplied: true } }), /:args/);
  const settings = JSON.parse(provider.settingsConfig);
  settings.auth.API_KEY = 'synthetic-forbidden';
  await assert.rejects(run.call('update_grok_provider', { provider: { ...provider, settingsConfig: JSON.stringify(settings) } }), /:settings/);
  assert.equal(run.snapshot().saveCount, 0);
  assert.deepEqual(run.snapshot().writes, []);
});

test('retry fixture rejects its first save without changing the stored provider', async () => {
  const run = ipc('retry');
  const before = run.snapshot().providers;
  const provider = { ...before[0], settingsConfig: JSON.stringify({ ...JSON.parse(before[0].settingsConfig), fixtureMarker: 'synthetic' }) };
  await assert.rejects(run.call('update_grok_provider', { provider }), /Synthetic Grok save failure/);
  assert.deepEqual(run.snapshot().providers, before);
  await run.call('update_grok_provider', { provider });
  assert.deepEqual(run.snapshot().providers, [provider]);
  assert.equal(run.snapshot().saveCount, 2);
});

test('pending fixture keeps storage unchanged until explicit synthetic completion', async () => {
  const run = ipc('pending');
  const before = run.snapshot().providers;
  const provider = { ...before[0], settingsConfig: JSON.stringify({ ...JSON.parse(before[0].settingsConfig), fixtureMarker: 'synthetic' }) };
  const pending = run.call('update_grok_provider', { provider });
  assert.equal(run.snapshot().saveCount, 1);
  assert.deepEqual(run.snapshot().providers, before);
  run.releaseSave();
  await pending;
  assert.deepEqual(run.snapshot().providers, [provider]);
  assert.throws(() => run.releaseSave(), /No pending synthetic save/);
});

const routerSource = readFileSync(new URL('./fixtures/routerBoundary.mjs', import.meta.url), 'utf8')
  .replace(/export /g, '');
function routing() {
  return new Function(routerSource + '\nreturn { router, routingSnapshot };')() as {
    router: Record<string, unknown>; routingSnapshot(): { unexpected: string[] };
  };
}

test('routing boundary rejects navigation without retaining its destination', () => {
  const run = routing();
  const navigate = run.router.navigate as (destination: string) => never;
  assert.throws(() => navigate('/synthetic-private-destination'), /Unexpected routing operation/);
  assert.deepEqual(run.routingSnapshot(), { unexpected: ['navigate'] });
});

test('routing boundary rejects property reads and all mutation paths', () => {
  const run = routing();
  for (const action of [() => run.router.state, () => { run.router.state = 'synthetic'; },
    () => Object.defineProperty(run.router, 'state', { value: 'synthetic' }), () => { delete run.router.state; }]) {
    assert.throws(action, /Unexpected routing operation/);
  }
  assert.deepEqual(run.routingSnapshot().unexpected,
    ['property-read', 'property-write', 'property-definition', 'property-delete']);
});

test('routing snapshots cannot erase recorded forbidden operations', () => {
  const run = routing();
  assert.throws(() => run.router.state);
  run.routingSnapshot().unexpected.length = 0;
  assert.deepEqual(run.routingSnapshot().unexpected, ['property-read']);
});

const batchSource = readFileSync(new URL('../../../../../../scripts/verify-grok-editor-and-catalog.mjs', import.meta.url), 'utf8')
  .replace('await import(specifier)', 'await load(specifier)');
function batch(load: (specifier: string) => Promise<void>): Promise<void> {
  return new Function('load', 'return (async () => {\n' + batchSource + '\n})();')(load);
}

test('supervised batch selects the six-case and existing four-case fixtures exactly once', async () => {
  const selected: string[] = [];
  await batch(async specifier => { selected.push(specifier); });
  assert.deepEqual(selected, ['./verify-grok-editor-readback.mjs', './verify-grok-fetched-models.mjs']);
});

test('editor failure still reaches catalog controls but never becomes a passing batch', async () => {
  const selected: string[] = [];
  await assert.rejects(batch(async specifier => {
    selected.push(specifier);
    if (selected.length === 1) throw new Error('synthetic editor failure');
  }), /Grok editor\/catalog fixture batch failed/);
  assert.equal(selected.length, 2);
});

test('a failed existing catalog control also rejects the combined batch', async () => {
  await assert.rejects(batch(async specifier => {
    if (specifier.includes('fetched-models')) throw new Error('synthetic catalog failure');
  }), /Grok editor\/catalog fixture batch failed/);
});
