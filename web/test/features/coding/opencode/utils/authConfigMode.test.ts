import assert from 'node:assert/strict';
import test from 'node:test';
import { createAuthConfigMode } from '../../../../../features/coding/opencode/utils/authConfigMode.ts';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

test('auth editor is read-only before the first mode probe completes', () => {
  assert.equal(createAuthConfigMode().readOnly(), true);
});

test('auth editor enables saving only after a confirmed V1 probe', async () => {
  const mode = createAuthConfigMode();
  await mode.run(async () => false);
  assert.equal(mode.readOnly(), false);
  let saves = 0;
  await mode.save(async () => { saves++; });
  assert.equal(saves, 1);
});

test('auth editor refuses the save callback in V2', async () => {
  const mode = createAuthConfigMode();
  await mode.run(async () => true);
  let saves = 0;
  await assert.rejects(mode.save(async () => { saves++; }));
  assert.equal(saves, 0);
  assert.equal(mode.readOnly(), true);
});

test('a new probe locks a previously confirmed V1 editor immediately', async () => {
  const mode = createAuthConfigMode();
  await mode.run(async () => false);
  const probe = deferred<boolean>();
  const pending = mode.run(() => probe.promise);
  assert.equal(mode.readOnly(), true);
  probe.resolve(true);
  await pending;
});

test('failed first and refresh probes keep auth writes blocked', async () => {
  for (const previouslyV1 of [false, true]) {
    const mode = createAuthConfigMode();
    if (previouslyV1) await mode.run(async () => false);
    await assert.rejects(mode.run(async () => { throw new Error('synthetic probe failure'); }));
    assert.equal(mode.readOnly(), true);
    let saves = 0;
    await assert.rejects(mode.save(async () => { saves++; }));
    assert.equal(saves, 0);
  }
});

test('a stale V1 response cannot unlock a newer V2 result', async () => {
  const mode = createAuthConfigMode();
  const old = deferred<boolean>();
  const pending = mode.run(() => old.promise);
  await mode.run(async () => true);
  old.resolve(false);
  await pending;
  assert.equal(mode.readOnly(), true);
  assert.equal(mode.getSnapshot().enabled, true);
});

test('a stale success cannot unlock a newer failed probe', async () => {
  const mode = createAuthConfigMode();
  const old = deferred<boolean>();
  const pending = mode.run(() => old.promise);
  await assert.rejects(mode.run(async () => { throw new Error('synthetic failure'); }));
  old.resolve(false);
  await pending;
  assert.equal(mode.readOnly(), true);
});

test('cancelling an old probe cannot finish a newer pending request', async () => {
  const mode = createAuthConfigMode();
  let current = true;
  const old = deferred<boolean>();
  const pending = mode.run(() => old.promise, () => current);
  current = false;
  const newer = deferred<boolean>();
  const next = mode.run(() => newer.promise);
  old.resolve(false);
  await pending;
  assert.equal(mode.getSnapshot().loading, true);
  assert.equal(mode.readOnly(), true);
  newer.resolve(false);
  await next;
  assert.equal(mode.readOnly(), false);
});

test('a newer probe cannot unlock the editor during migration follow-up work', async () => {
  const mode = createAuthConfigMode();
  await mode.run(async () => false);
  const migration = deferred<boolean>();
  const changing = mode.change(() => migration.promise);
  await mode.run(async () => false);
  assert.equal(mode.readOnly(), true);
  assert.equal(mode.getSnapshot().loading, true);
  migration.resolve(true);
  await changing;
  assert.equal(mode.readOnly(), true, 'a post-migration probe must confirm the final disk mode');
  await mode.run(async () => true);
  assert.equal(mode.readOnly(), true);
  assert.equal(mode.getSnapshot().loading, false);
});

test('failed migration invalidates every overlapping V1 probe', async () => {
  const mode = createAuthConfigMode();
  const mutation = deferred<boolean>();
  const changing = mode.change(() => mutation.promise);
  const old = deferred<boolean>();
  const probing = mode.run(() => old.promise);
  mutation.reject(new Error('synthetic migration failure'));
  await assert.rejects(changing);
  old.resolve(false);
  await probing;
  assert.equal(mode.readOnly(), true);
  await mode.run(async () => false);
  assert.equal(mode.readOnly(), false, 'a fresh successful V1 retry restores editing');
});
