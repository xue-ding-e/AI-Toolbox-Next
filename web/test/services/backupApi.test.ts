import assert from 'node:assert/strict';
import test from 'node:test';
import {
  backupDatabase, selectBackupFile, restoreDatabase,
  listRepositoryBackups, restoreFromRepository, deleteRepositoryBackup,
} from '../../services/backupApi';

// Exercise the real API and dialog adapter with synthetic IPC only. These tests
// do not execute the page's confirmation flow or any native restore operation.
const originalWindow = (globalThis as { window?: unknown }).window;
type Call = { command: string; args?: Record<string, unknown> };
function stubInvoke(reply: (call: Call) => Promise<unknown>) {
  const calls: Call[] = [];
  (globalThis as { window?: unknown }).window = {
    __TAURI_INTERNALS__: { invoke: async (command: string, args?: Record<string, unknown>) => {
      const call = { command, args };
      calls.push(call);
      return reply(call);
    } },
  };
  return calls;
}
test.afterEach(() => { (globalThis as { window?: unknown }).window = originalWindow; });
const result = { warnings: [], willReapplyApplied: true };

test('cancelled backup picker returns null and only invokes the dialog', async () => {
  const calls = stubInvoke(async ({ command }) => {
    assert.equal(command, 'plugin:dialog|open');
    return null;
  });
  assert.equal(await selectBackupFile(), null);
  assert.deepEqual(calls, [{ command: 'plugin:dialog|open', args: { options: {
    multiple: false,
    filters: [{ name: 'Backup Files', extensions: ['zip', 'enc'] }],
    title: 'Select Backup File',
  } } }]);
});

test('picker preserves selected plain and encrypted paths without restoring them', async () => {
  for (const path of ['/synthetic/backup.zip', '/synthetic/备份.zip.enc']) {
    const calls = stubInvoke(async ({ command }) => {
      assert.equal(command, 'plugin:dialog|open');
      return path;
    });
    assert.equal(await selectBackupFile(), path);
    assert.equal(calls.length, 1);
  }
});

test('picker failures propagate without trying a restore', async () => {
  const error = new Error('Synthetic dialog failure');
  const calls = stubInvoke(async () => { throw error; });
  await assert.rejects(selectBackupFile(), (actual) => actual === error);
  assert.deepEqual(calls.map(({ command }) => command), ['plugin:dialog|open']);
});

test('local restore forwards options once and does not retain a one-shot password', async () => {
  const calls = stubInvoke(async ({ command }) => {
    assert.equal(command, 'restore_database');
    return result;
  });
  assert.deepEqual(await restoreDatabase('/synthetic/backup.zip.enc', {
    skipCliCustomRoots: true, restorePassword: 'synthetic-test-only',
  }), result);
  assert.deepEqual(await restoreDatabase('/synthetic/backup.zip'), result);
  assert.deepEqual(calls, [
    { command: 'restore_database', args: { zipFilePath: '/synthetic/backup.zip.enc', skipCliCustomRoots: true, restorePassword: 'synthetic-test-only' } },
    { command: 'restore_database', args: { zipFilePath: '/synthetic/backup.zip', skipCliCustomRoots: false, restorePassword: null } },
  ]);
});

test('restore rejection reaches the caller without automatic retry', async () => {
  const error = { code: 'passwordRequired', message: 'Synthetic password required' };
  const calls = stubInvoke(async () => { throw error; });
  await assert.rejects(restoreDatabase('/synthetic/backup.zip.enc'), (actual) => actual === error);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].command, 'restore_database');
});

test('repository list preserves empty results and listed SHA for explicit restore and delete', async () => {
  const file = { filename: 'synthetic.zip.enc', size: 12, encrypted: true, sha: 'synthetic-listed-sha' };
  let listed = false;
  const calls = stubInvoke(async ({ command }) => {
    if (command === 'list_repository_backups') {
      if (!listed) { listed = true; return []; }
      return [file];
    }
    if (command === 'restore_from_repository') return result;
    assert.equal(command, 'delete_repository_backup');
  });
  assert.deepEqual(await listRepositoryBackups(), []);
  const [selected] = await listRepositoryBackups();
  assert.deepEqual(selected, file);
  assert.deepEqual(await restoreFromRepository(selected.filename, selected.sha!), result);
  await deleteRepositoryBackup(selected.filename, selected.sha!);
  assert.deepEqual(calls.slice(2), [
    { command: 'restore_from_repository', args: { filename: file.filename, sha: file.sha, skipCliCustomRoots: false, restorePassword: null } },
    { command: 'delete_repository_backup', args: { filename: file.filename, sha: file.sha } },
  ]);
});

test('missing local backup destination rejects before invoking native code', async () => {
  const calls = stubInvoke(async () => { throw new Error('Unexpected IPC'); });
  await assert.rejects(backupDatabase(''), /Backup path is not configured/);
  assert.deepEqual(calls, []);
});
