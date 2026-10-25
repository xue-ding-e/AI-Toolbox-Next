import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { constants } from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { canonicalRecord, GROK_FIXTURES, openBatchControl, parseAcknowledgement,
  runFixtureBatch, runSupervisedGrokBatch } from './grok-batch-lifecycle.mjs';

// Pure fake I/O/processes: these tests neither open a real directory nor spawn a
// child, read proc/cgroup metadata, build a fixture, or contact a browser/socket.
const DIRECTORY = '/synthetic-private/control';
const SESSION = 'a'.repeat(32);
const error = code => Object.assign(new Error(`synthetic ${code}`), { code });
const request = (index = 0) => ({ schema: 1, session: SESSION, index, pid: 100 + index, exitCode: 0 });
const ackFor = r => ({ ...r, startTicks: 1000 + r.index, status: 'quiescent' });

function fakeIO(options = {}) {
  const files = new Map(); const calls = []; let ino = 10n; let clock = 0;
  const entry = (kind, data = '') => ({ kind, data: Buffer.from(data), dev: 1n, ino: ino++,
    mode: kind === 'dir' ? 0o40700n : 0o100600n, uid: 123n, mtimeNs: 1n, ctimeNs: 1n });
  files.set(DIRECTORY, entry('dir'));
  const stat = value => ({ ...value, size: BigInt(value.data.length),
    isDirectory: () => value.kind === 'dir', isFile: () => value.kind === 'file' });
  const find = name => { if (!files.has(name)) throw error('ENOENT'); return files.get(name); };
  const io = {
    async lstat(name) { calls.push(['lstat', name]); return stat(find(name)); },
    async readdir() { return [...files.keys()].filter(name => name !== DIRECTORY).map(name => path.basename(name)); },
    async open(name, flags, mode) {
      calls.push(['open', name, flags, mode]);
      if (name.includes('/ack-') && options.deniedAckOpen) throw error('EACCES');
      if (flags & constants.O_CREAT) {
        if (files.has(name) && flags & constants.O_EXCL) throw error('EEXIST');
        if (!files.has(name)) files.set(name, entry('file'));
      }
      const item = find(name);
      if (item.kind === 'symlink' && flags & constants.O_NOFOLLOW) throw error('ELOOP');
      return {
        async stat() { calls.push(['fstat', name]); return stat(item); },
        async writeFile(data) { item.data = Buffer.from(data); item.mtimeNs += 1n; item.ctimeNs += 1n; },
        async sync() { calls.push(['sync', name]); },
        async read(buffer, offset, length) {
          calls.push(['read', name]);
          if (options.readENOENT) throw error('ENOENT');
          item.data.copy(buffer, offset, 0, length);
          if (options.replaceAckAfterRead) files.set(name, { ...item, ino: ino++ });
          return { bytesRead: Math.min(length, item.data.length) };
        },
        async close() { calls.push(['close', name]); },
      };
    },
    async link(from, to) {
      calls.push(['link', from, to]);
      if (files.has(to)) throw error('EEXIST');
      if (options.replaceTemporaryOnLink) files.set(from, entry('file', '{}\n'));
      const item = find(from); item.ctimeNs += 1n; files.set(to, item);
      if (options.replaceDirectoryOnPublish) files.set(DIRECTORY, entry('dir'));
      if (options.autoAck !== false) {
        const r = JSON.parse(item.data.toString());
        const ack = entry(options.ackKind ?? 'file', options.ackBytes ?? canonicalRecord(ackFor(r)));
        if (options.ackUid !== undefined) ack.uid = options.ackUid;
        files.set(path.join(DIRECTORY, `ack-${r.index}.json`), ack);
      }
    },
    async unlink(name) { calls.push(['unlink', name]); find(name); files.delete(name); },
  };
  return { io, files, calls, entry, now: () => clock, delay: async ms => { clock += ms; },
    options: { directory: DIRECTORY, session: SESSION, io, uid: 123,
      now: () => clock, delay: async ms => { clock += ms; } } };
}

test('supervised batch selects the six-case and existing four-case fixtures exactly once', async () => {
  const selected = [], checked = [];
  await runFixtureBatch({ launch: async fixture => { selected.push(fixture); return request(selected.length - 1); },
    checkpoint: async (outcome, index) => { checked.push([outcome.pid, index]); } });
  assert.deepEqual(selected, [...GROK_FIXTURES]); assert.deepEqual(checked, [[100, 0], [101, 1]]);
});

test('editor failure still reaches catalog controls but never becomes a passing batch', async () => {
  const selected = [], checked = [];
  await assert.rejects(runFixtureBatch({ launch: async fixture => {
    selected.push(fixture); return { ...request(selected.length - 1), exitCode: selected.length === 1 ? 1 : 0 };
  }, checkpoint: async (_, index) => checked.push(index) }), /Grok editor\/catalog fixture batch failed/);
  assert.deepEqual(selected, [...GROK_FIXTURES]); assert.deepEqual(checked, [0, 1]);
});

test('a failed existing catalog control also rejects the combined batch', async () => {
  await assert.rejects(runFixtureBatch({ launch: async fixture => ({ ...request(),
    exitCode: fixture.includes('fetched-models') ? 1 : 0 }), checkpoint: async () => {} }), /batch failed/);
});

test('the next child never starts until the prior supervisor acknowledgement', async () => {
  const selected = []; let release;
  const pending = runFixtureBatch({ launch: async fixture => { selected.push(fixture); return request(); },
    checkpoint: async (_, index) => { if (index === 0) await new Promise(resolve => { release = resolve; }); } });
  await Promise.resolve(); await Promise.resolve(); assert.equal(selected.length, 1);
  release(); await pending; assert.equal(selected.length, 2);
});

test('spawn errors, signaled outcomes and failed acknowledgements stop before a second child', async () => {
  for (const kind of ['spawn', 'signal', 'ack']) {
    let launched = 0;
    await assert.rejects(runFixtureBatch({ launch: async () => {
      launched += 1; if (kind === 'spawn') throw error('ENOENT');
      return kind === 'signal' ? { pid: 100, exitCode: null } : request();
    }, checkpoint: async () => { if (kind === 'ack') throw error('EACCES'); } }));
    assert.equal(launched, 1);
  }
});

test('request publication is exclusive and acknowledgement binds both batches to observed generations', async () => {
  const f = fakeIO(); const control = await openBatchControl(f.options);
  assert.deepEqual(await control.checkpoint(request(0), 0), ackFor(request(0)));
  assert.deepEqual(await control.checkpoint(request(1), 1), ackFor(request(1)));
  await control.close();
  assert.deepEqual([...f.files.keys()].filter(name => name !== DIRECTORY).map(name => path.basename(name)).sort(),
    ['ack-0.json', 'ack-1.json', 'request-0.json', 'request-1.json']);
  for (const c of f.calls.filter(x => x[0] === 'open')) {
    assert(c[2] & constants.O_NOFOLLOW);
    if (c[1].includes('.tmp')) { assert(c[2] & constants.O_EXCL); assert.equal(c[3], 0o600); }
  }
});

test('duplicates, skipped checkpoints and preexisting acknowledgement cannot advance a batch', async () => {
  const f = fakeIO(); const control = await openBatchControl(f.options);
  await assert.rejects(control.checkpoint(request(1), 1), /out-of-order/);
  await control.checkpoint(request(), 0);
  await assert.rejects(control.checkpoint(request(), 0), /Duplicate/);
  f.files.set(path.join(DIRECTORY, 'ack-1.json'), f.entry('file', canonicalRecord(ackFor(request(1)))));
  await assert.rejects(control.checkpoint(request(1), 1), /Preexisting/);
  assert.equal(f.calls.filter(x => x[0] === 'link').length, 1); await control.close();
});

test('an existing request or temporary request is never overwritten', async () => {
  for (const name of ['request-0.json', '.request-0.tmp']) {
    const f = fakeIO(); const control = await openBatchControl(f.options);
    const existing = f.entry('file', 'synthetic existing record\n');
    f.files.set(path.join(DIRECTORY, name), existing);
    await assert.rejects(control.checkpoint(request(), 0), /EEXIST/);
    assert.equal(f.files.get(path.join(DIRECTORY, name)), existing); await control.close();
  }
});

test('canonical acknowledgement rejects extra keys, duplicate JSON, mismatched identity and unsafe numbers', () => {
  const r = request(); const a = ackFor(r); const raws = [
    canonicalRecord({ ...a, extra: true }), canonicalRecord({ ...a, index: 1 }),
    canonicalRecord({ ...a, session: 'b'.repeat(32) }), canonicalRecord({ ...a, pid: 99 }),
    canonicalRecord({ ...a, exitCode: 1 }), canonicalRecord({ ...a, status: 'passed' }),
    canonicalRecord({ ...a, startTicks: 0 }), canonicalRecord({ ...a, startTicks: Number.MAX_SAFE_INTEGER + 1 }),
    canonicalRecord(a).replace('{', '{"schema":1,'), JSON.stringify(a), '[]\n', 'x'.repeat(1025),
  ];
  for (const raw of raws) assert.throws(() => parseAcknowledgement(Buffer.from(raw), r));
  assert.deepEqual(parseAcknowledgement(Buffer.from(canonicalRecord(a)), r), a);
});

test('invalid outcome/schema indices cannot publish requests', async () => {
  const f = fakeIO(); const control = await openBatchControl(f.options);
  for (const outcome of [{ pid: 0, exitCode: 0 }, { pid: 1, exitCode: -1 },
    { pid: 1, exitCode: 256 }, { pid: Number.MAX_SAFE_INTEGER + 1, exitCode: 0 }]) {
    await assert.rejects(control.checkpoint(outcome, 0), /Invalid/);
  }
  assert.equal(f.calls.filter(x => x[0] === 'link').length, 0); await control.close();
});

test('only not-yet-open ENOENT may wait; denied or disappearing opened acknowledgements are not retried', async () => {
  for (const options of [{ deniedAckOpen: true }, { readENOENT: true }]) {
    const f = fakeIO(options); const control = await openBatchControl(f.options);
    await assert.rejects(control.checkpoint(request(), 0), options.readENOENT ? /ENOENT/ : /EACCES/);
    assert.equal(f.calls.filter(x => x[0] === 'open' && x[1].endsWith('/ack-0.json')).length, 1);
    assert.equal(f.now(), 0); await control.close();
  }
});

test('missing acknowledgement reaches the bounded deadline without launching another fixture', async () => {
  const f = fakeIO({ autoAck: false }); const control = await openBatchControl(f.options); let launches = 0;
  await assert.rejects(runFixtureBatch({ launch: async () => { launches += 1; return request(); },
    checkpoint: control.checkpoint }), /acknowledgement deadline/);
  assert.equal(launches, 1); assert.equal(f.now(), 5000); await control.close();
});

test('directory replacement, request replacement and acknowledgement replacement all fail closed', async () => {
  for (const options of [{ replaceDirectoryOnPublish: true }, { replaceTemporaryOnLink: true }, { replaceAckAfterRead: true }]) {
    const f = fakeIO(options); const control = await openBatchControl(f.options);
    await assert.rejects(control.checkpoint(request(), 0), /identity|changed/); await control.close();
  }
});

test('nonregular, symlink, oversized and foreign-owned acknowledgement files are refused', async () => {
  for (const options of [{ ackKind: 'fifo' }, { ackKind: 'symlink' }, { ackBytes: ' '.repeat(1025) }, { ackUid: 999n }]) {
    const f = fakeIO(options); const control = await openBatchControl(f.options);
    await assert.rejects(control.checkpoint(request(), 0)); await control.close();
  }
});

test('relative, missing-session, foreign-owned, symlink and nonempty directories fail before child work', async () => {
  for (const change of [f => ({ ...f.options, directory: './relative' }), f => ({ ...f.options, session: '' }),
    f => { f.files.get(DIRECTORY).uid = 999n; return f.options; },
    f => { f.files.get(DIRECTORY).mode = 0o40755n; return f.options; },
    f => { f.files.get(DIRECTORY).kind = 'symlink'; return f.options; },
    f => { f.files.set(DIRECTORY + '/ack-0.json', f.entry('file')); return f.options; }]) {
    const f = fakeIO(); await assert.rejects(openBatchControl(change(f)));
  }
});

test('production entry uses fixed fresh Node children and strips only the private control channel', async () => {
  const f = fakeIO(); const spawned = [];
  await runSupervisedGrokBatch({ argv: ['--browser', '/synthetic/chrome', '--output', '/synthetic/output'],
    env: { HOME: '/synthetic/home', GROK_BATCH_CONTROL_DIR: DIRECTORY, GROK_BATCH_SESSION: SESSION },
    ...f.options, spawnChild(executable, args, options) {
      const child = new EventEmitter(); child.pid = 100 + spawned.length;
      spawned.push({ executable, args, options }); queueMicrotask(() => child.emit('close', 0, null)); return child;
    } });
  assert.equal(spawned.length, 2);
  for (const [index, child] of spawned.entries()) {
    assert.equal(child.executable, process.execPath); assert.equal(path.basename(child.args[0]), GROK_FIXTURES[index]);
    assert.deepEqual(child.args.slice(1), ['--browser', '/synthetic/chrome', '--output', '/synthetic/output']);
    assert.deepEqual(child.options.env, { HOME: '/synthetic/home' });
    assert.equal(child.options.shell, false); assert.equal(child.options.detached, false);
    assert.deepEqual(child.options.stdio, ['ignore', 'inherit', 'inherit']);
  }
});

test('production spawn error or signal never publishes a successful checkpoint or starts another child', async () => {
  for (const kind of ['error', 'signal']) {
    const f = fakeIO(); let spawned = 0;
    await assert.rejects(runSupervisedGrokBatch({ argv: ['--browser', '/chrome', '--output', '/output'],
      env: { GROK_BATCH_CONTROL_DIR: DIRECTORY, GROK_BATCH_SESSION: SESSION }, ...f.options,
      spawnChild() { spawned += 1; const child = new EventEmitter(); child.pid = 100;
        queueMicrotask(() => kind === 'error' ? child.emit('error', error('EACCES')) : child.emit('close', null, 'SIGTERM'));
        return child; } }));
    assert.equal(spawned, 1); assert.equal(f.calls.filter(x => x[0] === 'link').length, 0);
  }
});
