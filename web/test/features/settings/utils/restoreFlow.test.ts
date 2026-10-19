import assert from 'node:assert/strict';
import test from 'node:test';
import { executeRestoreFlow } from '../../../../features/settings/utils/restoreFlow.ts';

type Result = { warnings: string[] };
type Adapters = Parameters<typeof executeRestoreFlow<Result, Result>>[1];
const success: Result = { warnings: [] };

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

function harness() {
  const events: unknown[][] = [];
  const dialogs: Parameters<Adapters['showSuccess']>[0][] = [];
  const prompts: { submit: (password: string) => void; wrong: boolean }[] = [];
  const finished: (() => void)[] = [];
  const adapters: Adapters = {
    setLoading: (value) => {
      events.push(['loading', value]);
      if (!value) finished.shift()?.();
    },
    translate: (key) => `translated:${key}`,
    buildSuccessContent: (result) => { events.push(['content', result]); return result; },
    showSuccess: (dialog) => { events.push(['success']); dialogs.push(dialog); },
    showWarnings: (result) => { events.push(['warnings', result]); },
    promptPassword: (submit, wrong) => { events.push(['prompt', wrong]); prompts.push({ submit, wrong }); },
    restart: () => { events.push(['restart']); return Promise.resolve(); },
    logError: (label, error) => { events.push(['log', label, error]); },
    showError: (message) => { events.push(['error', message]); },
  };
  return {
    adapters, events, dialogs, prompts,
    nextFinish: () => new Promise<void>((resolve) => finished.push(resolve)),
  };
}

test('pending restore stays loading; success presents locked restart dialog before warnings', async () => {
  const h = harness();
  const pending = deferred<Result>();
  const result = { warnings: ['fallback path'] };
  const run = executeRestoreFlow((password) => {
    h.events.push(['restore', password]);
    return pending.promise;
  }, h.adapters);
  assert.ok(run instanceof Promise);
  assert.deepEqual(h.events, [['loading', true], ['restore', undefined]]);
  assert.equal(h.dialogs.length, 0);
  pending.resolve(result);
  await run;
  assert.deepEqual(h.events.slice(2), [['content', result], ['success'], ['warnings', result], ['loading', false]]);
  const dialog = h.dialogs[0];
  assert.deepEqual({ ...dialog, onOk: undefined }, {
    title: 'translated:settings.backupSettings.restoreSuccess', content: result,
    okText: 'translated:common.restart', closable: false, maskClosable: false, keyboard: false, onOk: undefined,
  });
  assert.equal(dialog.onOk(), undefined, 'restart callback remains fire-and-forget');
  assert.deepEqual(h.events.at(-1), ['restart']);
});

for (const [label, error, suffix] of [
  ['plain Error', new Error('disk unavailable'), 'Error: disk unavailable'],
  ['malformed JSON', '{broken', '{broken'],
  ['JSON suggestion', JSON.stringify({ type: 'io', suggestion: 'errors.disk' }), 'translated:errors.disk'],
  ['JSON without suggestion', JSON.stringify({ type: 'io' }), '{"type":"io"}'],
  ['null JSON', 'null', 'null'],
] as const) {
  test(`restore rejection handles ${label} and releases loading`, async () => {
    const h = harness();
    const pending = deferred<Result>();
    const run = executeRestoreFlow(() => pending.promise, h.adapters);
    assert.deepEqual(h.events, [['loading', true]]);
    pending.reject(error);
    await run;
    assert.deepEqual(h.events, [
      ['loading', true], ['log', 'Restore failed:', error],
      ['error', `translated:settings.backupSettings.restoreFailed: ${suffix}`], ['loading', false],
    ]);
    assert.equal(h.dialogs.length, 0);
    assert.equal(h.prompts.length, 0);
  });
}

test('password-required then wrong-password retries the same closure only on later submit', async () => {
  const h = harness();
  const attempts = [deferred<Result>(), deferred<Result>(), deferred<Result>()];
  const holder = { value: true };
  const selection = { filename: 'selected.zip.enc', sha: 'selected-sha' };
  const calls: unknown[] = [];
  const restore = (password?: string) => {
    calls.push({ selection, skipCliCustomRoots: holder.value, password });
    return attempts[calls.length - 1].promise;
  };
  const first = executeRestoreFlow(restore, h.adapters);
  attempts[0].reject(JSON.stringify({ type: 'passwordRequired' }));
  await first;
  assert.deepEqual(h.events, [['loading', true], ['prompt', false], ['loading', false]]);
  assert.equal(calls.length, 1, 'opening a prompt does not retry');
  // Simulate a later user interaction, never synchronous submission inside the prompt adapter.
  const secondFinished = h.nextFinish();
  assert.equal(h.prompts[0].submit('wrong'), undefined);
  assert.equal(calls.length, 2);
  assert.deepEqual(h.events.at(-1), ['loading', true]);
  attempts[1].reject(JSON.stringify({ type: 'passwordWrong' }));
  await secondFinished;
  assert.equal(h.prompts[1].wrong, true);
  assert.equal(calls.length, 2);
  assert.deepEqual(h.events.slice(-2), [['prompt', true], ['loading', false]]);
  const thirdFinished = h.nextFinish();
  assert.equal(h.prompts[1].submit('correct'), undefined);
  attempts[2].resolve(success);
  await thirdFinished;
  assert.deepEqual(calls, [undefined, 'wrong', 'correct'].map((password) => ({
    selection, skipCliCustomRoots: true, password,
  })));
  for (const call of calls as { selection: typeof selection }[]) assert.equal(call.selection, selection);
  assert.equal(h.dialogs.length, 1);
  assert.equal(h.events.some(([kind]) => kind === 'error' || kind === 'log'), false);
  assert.deepEqual(h.events.filter(([kind]) => kind === 'loading'), [
    ['loading', true], ['loading', false], ['loading', true], ['loading', false], ['loading', true], ['loading', false],
  ]);
});

for (const type of ['passwordRequired', 'passwordWrong']) {
  test(`leaving ${type} prompt without submitting does not retry`, async () => {
    const h = harness();
    let calls = 0;
    await executeRestoreFlow(async () => { calls += 1; throw JSON.stringify({ type }); }, h.adapters);
    // Cancellation has no helper callback; the real modal simply never calls submit.
    await new Promise<void>((resolve) => setImmediate(resolve));
    assert.equal(calls, 1);
    assert.equal(h.prompts[0].wrong, type === 'passwordWrong');
    assert.deepEqual(h.events.at(-1), ['loading', false]);
    assert.equal(h.dialogs.length, 0);
  });
}

test('overlapping calls characterize existing per-attempt loading, not an operation counter', async () => {
  const h = harness();
  const first = deferred<Result>();
  const second = deferred<Result>();
  const a = executeRestoreFlow(() => first.promise, h.adapters);
  const b = executeRestoreFlow(() => second.promise, h.adapters);
  assert.deepEqual(h.events, [['loading', true], ['loading', true]]);
  first.resolve(success);
  await a;
  assert.deepEqual(h.events.at(-1), ['loading', false], 'first completion clears loading while second is pending');
  assert.equal(h.dialogs.length, 1);
  second.resolve(success);
  await b;
  assert.equal(h.dialogs.length, 2);
  assert.deepEqual(h.events.filter(([kind]) => kind === 'loading'), [
    ['loading', true], ['loading', true], ['loading', false], ['loading', false],
  ]);
});
