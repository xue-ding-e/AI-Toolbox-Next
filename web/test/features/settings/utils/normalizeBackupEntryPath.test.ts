import assert from 'node:assert/strict';
import test from 'node:test';
import { normalizeBackupEntryPath } from '../../../../features/settings/utils/normalizeBackupEntryPath.ts';

test('empty optional backup paths do not invoke the backend', async () => {
  let calls = 0;
  const result = await normalizeBackupEntryPath(' \t ', async () => {
    calls += 1;
    return '/unexpected';
  });
  assert.equal(result, '');
  assert.equal(calls, 0);
});

test('nonempty paths are trimmed and use the backend result exactly once', async () => {
  const inputs: string[] = [];
  const result = await normalizeBackupEntryPath(' ~/synthetic-backup ', async (path) => {
    inputs.push(path);
    return '/synthetic-home/synthetic-backup';
  });
  assert.deepEqual(inputs, ['~/synthetic-backup']);
  assert.equal(result, '/synthetic-home/synthetic-backup');
});

for (const failure of [
  'Synthetic path exceeds service limit',
  new Error('Synthetic normalization unavailable'),
]) {
  test(`backend rejection is not converted to a successful raw path: ${String(failure)}`, async () => {
    await assert.rejects(
      normalizeBackupEntryPath(' ~/synthetic-backup ', async () => { throw failure; }),
      (error: unknown) => error === failure,
    );
  });
}
