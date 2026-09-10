import assert from 'node:assert/strict';
import test from 'node:test';
import {
  backupProvidersBeforeDelete,
  reconcileProviderSelection,
} from '../../../../../features/coding/shared/providerList/providerBatchOperations.ts';

test('batch deletion backs up every provider before removing any runtime record', async () => {
  const providers = [{ id: 'first', key: 'first-key' }, { id: 'second', key: 'second-key' }];
  const runtime = new Map(providers.map((provider) => [provider.id, provider]));
  const favorites = new Map<string, (typeof providers)[number]>();
  const operations: string[] = [];

  await backupProvidersBeforeDelete(providers, async (provider) => {
    assert.equal(runtime.size, 2);
    favorites.set(provider.id, structuredClone(provider));
    operations.push(`backup:${provider.id}`);
  }, (provider) => `Backup failed: ${provider.id}`);
  for (const provider of providers) {
    runtime.delete(provider.id);
    operations.push(`delete:${provider.id}`);
  }

  assert.deepEqual(operations, ['backup:first', 'backup:second', 'delete:first', 'delete:second']);
  assert.equal(runtime.size, 0);
  runtime.set('second', structuredClone(favorites.get('second')!));
  assert.equal(runtime.get('second')?.key, 'second-key');
});

test('a failed backup aborts deletion and identifies the provider', async (context) => {
  context.mock.method(console, 'error', () => {});
  let deletionStarted = false;
  await assert.rejects(async () => {
    await backupProvidersBeforeDelete(['first', 'second'], async (id) => {
      if (id === 'second') throw new Error('storage unavailable');
    }, (id) => `Could not back up ${id}; deletion cancelled`);
    deletionStarted = true;
  }, /Could not back up second; deletion cancelled/);
  assert.equal(deletionStarted, false);
});

test('selection retains unprocessed records after a partial deletion', () => {
  const selection = new Set(['deleted', 'failed', 'pending']);
  const remaining = reconcileProviderSelection(selection, new Set(['failed', 'pending', 'unselected']));
  assert.deepEqual([...remaining], ['failed', 'pending']);
  assert.deepEqual([...selection], ['deleted', 'failed', 'pending']);
});

test('selection drops filtered and newly protected records', () => {
  const remaining = reconcileProviderSelection(
    new Set(['now-default', 'hidden', 'deletable']),
    new Set(['deletable']),
  );
  assert.deepEqual([...remaining], ['deletable']);
});
