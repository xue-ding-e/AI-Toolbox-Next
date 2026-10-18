import assert from 'node:assert/strict';
import test from 'node:test';
import { useMcpStore } from '../../../../../features/coding/mcp/stores/mcpStore.ts';
import type { McpServer } from '../../../../../features/coding/mcp/types/index.ts';

const originalWindow = (globalThis as { window?: unknown }).window;
const initialState = useMcpStore.getInitialState();
const sampleServer: McpServer = {
  id: 'synthetic-server', name: 'Synthetic server', server_type: 'stdio',
  server_config: { command: 'synthetic-command', args: [] },
  enabled_tools: [], sync_details: [], description: null, user_group: null,
  user_note: null, tags: [], timeout: null, sort_index: 0,
  management_enabled: true, disabled_previous_tools: [], created_at: 1, updated_at: 1,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

function mockInvoke(invoke: (command: string) => Promise<unknown>) {
  (globalThis as { window?: unknown }).window = { __TAURI_INTERNALS__: { invoke } };
}

test.beforeEach(() => useMcpStore.setState(initialState, true));
test.afterEach(() => { (globalThis as { window?: unknown }).window = originalWindow; });

test('fetchServers loads backend records and clears loading', async () => {
  const response = deferred<McpServer[]>();
  mockInvoke(async (command) => {
    assert.equal(command, 'mcp_list_servers');
    return response.promise;
  });
  const fetch = useMcpStore.getState().fetchServers();
  assert.equal(useMcpStore.getState().loading, true);
  response.resolve([sampleServer]);
  await fetch;
  assert.deepEqual(useMcpStore.getState().servers, [sampleServer]);
  assert.equal(useMcpStore.getState().loading, false);
});

test('failed refresh preserves existing servers and clears loading', async (context) => {
  useMcpStore.setState({ servers: [sampleServer] });
  context.mock.method(console, 'error', () => {});
  mockInvoke(async () => { throw new Error('synthetic read failure'); });
  await useMcpStore.getState().fetchServers();
  assert.deepEqual(useMcpStore.getState().servers, [sampleServer]);
  assert.equal(useMcpStore.getState().loading, false);
});

test('opening then cancelling import modal state does not write or discard servers', () => {
  let calls = 0;
  mockInvoke(async () => { calls += 1; });
  useMcpStore.setState({ servers: [sampleServer] });
  useMcpStore.getState().setImportModalOpen(true);
  assert.equal(useMcpStore.getState().isImportModalOpen, true);
  useMcpStore.getState().setImportModalOpen(false);
  assert.equal(useMcpStore.getState().isImportModalOpen, false);
  assert.deepEqual(useMcpStore.getState().servers, [sampleServer]);
  assert.equal(calls, 0);
});

test('older server refresh cannot overwrite a newer completed refresh', async () => {
  const older = deferred<McpServer[]>();
  const newer = deferred<McpServer[]>();
  let reads = 0;
  mockInvoke(() => ++reads === 1 ? older.promise : newer.promise);
  const first = useMcpStore.getState().fetchServers();
  const second = useMcpStore.getState().fetchServers();
  newer.resolve([]);
  await second;
  older.resolve([sampleServer]);
  await first;
  assert.deepEqual(useMcpStore.getState().servers, []);
});

test('an earlier refresh completing does not clear loading for the latest pending refresh', async () => {
  const older = deferred<McpServer[]>();
  const newer = deferred<McpServer[]>();
  let reads = 0;
  mockInvoke(() => ++reads === 1 ? older.promise : newer.promise);
  const first = useMcpStore.getState().fetchServers();
  const second = useMcpStore.getState().fetchServers();
  older.resolve([sampleServer]);
  await first;
  const loadingWhileLatestPending = useMcpStore.getState().loading;
  newer.resolve([]);
  await second;
  assert.equal(loadingWhileLatestPending, true);
  assert.equal(useMcpStore.getState().loading, false);
});

test('the latest failure preserves current data and an older response stays invalid', async (context) => {
  const older = deferred<McpServer[]>();
  let reads = 0;
  const current = { ...sampleServer, id: 'current-server' };
  useMcpStore.setState({ servers: [current] });
  context.mock.method(console, 'error', () => {});
  mockInvoke(() => ++reads === 1 ? older.promise : Promise.reject(new Error('latest failed')));
  const first = useMcpStore.getState().fetchServers();
  await useMcpStore.getState().fetchServers();
  assert.deepEqual(useMcpStore.getState().servers, [current]);
  assert.equal(useMcpStore.getState().loading, false);
  older.resolve([sampleServer]);
  await first;
  assert.deepEqual(useMcpStore.getState().servers, [current]);
  mockInvoke(async () => []);
  await useMcpStore.getState().fetchServers();
  assert.deepEqual(useMcpStore.getState().servers, []);
});

test('the latest successful refresh finishes loading without waiting for an obsolete response', async () => {
  const older = deferred<McpServer[]>();
  let reads = 0;
  mockInvoke(() => ++reads === 1 ? older.promise : Promise.resolve([]));
  const first = useMcpStore.getState().fetchServers();
  await useMcpStore.getState().fetchServers();
  const loadingAfterLatest = useMcpStore.getState().loading;
  older.resolve([sampleServer]);
  await first;
  assert.equal(loadingAfterLatest, false);
  assert.deepEqual(useMcpStore.getState().servers, []);
});

test('an older failed request cannot finish loading for the latest request', async (context) => {
  const older = deferred<void>();
  const newer = deferred<McpServer[]>();
  let reads = 0;
  context.mock.method(console, 'error', () => {});
  mockInvoke(() => ++reads === 1
    ? older.promise.then(() => { throw new Error('older failed'); })
    : newer.promise);
  const first = useMcpStore.getState().fetchServers();
  const second = useMcpStore.getState().fetchServers();
  older.resolve();
  await first;
  const loadingAfterOldFailure = useMcpStore.getState().loading;
  newer.resolve([sampleServer]);
  await second;
  assert.equal(loadingAfterOldFailure, true);
  assert.equal(useMcpStore.getState().loading, false);
  assert.deepEqual(useMcpStore.getState().servers, [sampleServer]);
});

test('scanning and fetching servers keep independent results in either completion order', async () => {
  for (const scanFirst of [true, false]) {
    useMcpStore.setState({ servers: [], scanResult: null });
    const serverResponse = deferred<McpServer[]>();
    const scanResult = { total_tools_scanned: 1, total_servers_found: 0, servers: [] };
    const scanResponse = deferred<typeof scanResult>();
    mockInvoke((command) => {
      if (command === 'mcp_list_servers') return serverResponse.promise;
      assert.equal(command, 'mcp_scan_servers');
      return scanResponse.promise;
    });
    const fetch = useMcpStore.getState().fetchServers();
    const scan = useMcpStore.getState().loadScanResult();
    if (scanFirst) {
      scanResponse.resolve(scanResult);
      await scan;
      serverResponse.resolve([sampleServer]);
    } else {
      serverResponse.resolve([sampleServer]);
      await fetch;
      scanResponse.resolve(scanResult);
    }
    await Promise.all([fetch, scan]);
    assert.deepEqual(useMcpStore.getState().servers, [sampleServer]);
    assert.deepEqual(useMcpStore.getState().scanResult, scanResult);
    assert.equal(useMcpStore.getState().loading, false);
  }
});

// The actions hook applies successful API results through these store methods.
// Drive that boundary without rendering React or performing native writes.
for (const mutation of ['create', 'edit', 'delete'] as const) {
  test(`old list cannot undo successful ${mutation}`, async () => {
    const api = await import('../../../../../features/coding/mcp/services/mcpApi.ts');
    const oldList = deferred<McpServer[]>();
    const changed = { ...sampleServer, name: 'Updated server' };
    useMcpStore.setState({ servers: mutation === 'create' ? [] : [sampleServer] });
    mockInvoke(async (command) => {
      if (command === 'mcp_list_servers') return oldList.promise;
      assert.equal(command, `mcp_${mutation === 'edit' ? 'update' : mutation}_server`);
      return changed;
    });
    const read = useMcpStore.getState().fetchServers();
    if (mutation === 'create') {
      const created = await api.createMcpServer({ name: changed.name, server_type: 'stdio', server_config: changed.server_config });
      useMcpStore.getState().addServer(created);
    } else if (mutation === 'edit') {
      const updated = await api.updateMcpServer(sampleServer.id, { name: changed.name });
      useMcpStore.getState().updateServer(updated);
    } else {
      await api.deleteMcpServer(sampleServer.id);
      useMcpStore.getState().removeServer(sampleServer.id);
    }
    const expected = mutation === 'delete' ? [] : [changed];
    assert.deepEqual(useMcpStore.getState().servers, expected);
    oldList.resolve(mutation === 'create' ? [] : [sampleServer]);
    await read;
    assert.deepEqual(useMcpStore.getState().servers, expected);
    assert.equal(useMcpStore.getState().loading, false);
  });
}

test('failed CRUD does not invalidate an in-flight list', async () => {
  const api = await import('../../../../../features/coding/mcp/services/mcpApi.ts');
  const response = deferred<McpServer[]>();
  mockInvoke(async (command) => {
    if (command === 'mcp_list_servers') return response.promise;
    assert.equal(command, 'mcp_delete_server');
    throw new Error('Synthetic delete failure');
  });
  const read = useMcpStore.getState().fetchServers();
  await assert.rejects(api.deleteMcpServer(sampleServer.id), /Synthetic delete failure/);
  response.resolve([sampleServer]);
  await read;
  assert.deepEqual(useMcpStore.getState().servers, [sampleServer]);
  assert.equal(useMcpStore.getState().loading, false);
});

test('new read after a mutation applies while older completion keeps its loading', async () => {
  const oldResponse = deferred<McpServer[]>();
  const newResponse = deferred<McpServer[]>();
  let calls = 0;
  mockInvoke(async () => ++calls === 1 ? oldResponse.promise : newResponse.promise);
  const oldRead = useMcpStore.getState().fetchServers();
  useMcpStore.getState().addServer(sampleServer);
  const newRead = useMcpStore.getState().fetchServers();
  oldResponse.resolve([]);
  await oldRead;
  assert.deepEqual(useMcpStore.getState().servers, [sampleServer]);
  assert.equal(useMcpStore.getState().loading, true);
  const latest = { ...sampleServer, description: 'New backend state' };
  newResponse.resolve([latest]);
  await newRead;
  assert.deepEqual(useMcpStore.getState().servers, [latest]);
  assert.equal(useMcpStore.getState().loading, false);
});

test('explicit reorder is not reverted by a pending old list', async () => {
  const other = { ...sampleServer, id: 'other-server' };
  const response = deferred<McpServer[]>();
  useMcpStore.setState({ servers: [sampleServer, other] });
  mockInvoke(async () => response.promise);
  const read = useMcpStore.getState().fetchServers();
  useMcpStore.getState().setServers([other, sampleServer]);
  response.resolve([sampleServer, other]);
  await read;
  assert.deepEqual(useMcpStore.getState().servers, [other, sampleServer]);
  assert.equal(useMcpStore.getState().loading, false);
});

test('successive writes preserve each other while invalidating the same old read', async () => {
  const edited = { ...sampleServer, name: 'Edited existing' };
  const deleted = { ...sampleServer, id: 'deleted-server' };
  const added = { ...sampleServer, id: 'added-server' };
  const response = deferred<McpServer[]>();
  useMcpStore.setState({ servers: [sampleServer, deleted] });
  mockInvoke(async () => response.promise);
  const read = useMcpStore.getState().fetchServers();
  useMcpStore.getState().addServer(added);
  useMcpStore.getState().updateServer(edited);
  useMcpStore.getState().removeServer(deleted.id);
  response.resolve([sampleServer, deleted]);
  await read;
  assert.deepEqual(useMcpStore.getState().servers, [edited, added]);
  assert.equal(useMcpStore.getState().loading, false);
});
