import { spawn } from 'node:child_process';
import { constants } from 'node:fs';
import * as fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

export const GROK_FIXTURES = Object.freeze([
  'verify-grok-editor-readback.mjs', 'verify-grok-fetched-models.mjs',
]);
const root = fileURLToPath(new URL('../../', import.meta.url));
const MAX_RECORD = 1024;
const ACK_WAIT_MS = 5000;
const positiveInteger = value => Number.isSafeInteger(value) && value > 0;
const require = (condition, message) => { if (!condition) throw new Error(message); };
const sameIdentity = (left, right) => left.dev === right.dev && left.ino === right.ino;
const sameFile = (left, right) => ['dev', 'ino', 'size', 'mtimeNs', 'ctimeNs', 'mode', 'uid']
  .every(key => left[key] === right[key]);

export function canonicalRecord(record) {
  return JSON.stringify(Object.fromEntries(Object.keys(record).sort().map(key => [key, record[key]]))) + '\n';
}

function validRequest(request) {
  require(request.schema === 1 && /^[0-9a-f]{32}$/.test(request.session)
    && (request.index === 0 || request.index === 1) && positiveInteger(request.pid)
    && Number.isInteger(request.exitCode) && request.exitCode >= 0 && request.exitCode <= 255,
  'Invalid Grok lifecycle request');
}

export function parseAcknowledgement(bytes, request) {
  require(bytes.length <= MAX_RECORD, 'Oversized Grok lifecycle acknowledgement');
  const ack = JSON.parse(bytes.toString('utf8'));
  require(ack && typeof ack === 'object' && !Array.isArray(ack), 'Invalid acknowledgement object');
  const expected = { ...request, startTicks: ack.startTicks, status: 'quiescent' };
  require(positiveInteger(ack.startTicks) && canonicalRecord(ack) === canonicalRecord(expected)
    && bytes.equals(Buffer.from(canonicalRecord(expected))), 'Grok lifecycle acknowledgement mismatch');
  return ack;
}

// A private directory and session prevent accidental cross-run/stale records,
// not malicious same-UID fixture impersonation. The sealed supervisor separately
// observes previously registered PID generations and the complete owned tree.
export async function openBatchControl({ directory, session, io = fs, uid = process.getuid(),
  now = () => performance.now(), delay = ms => new Promise(resolve => setTimeout(resolve, ms)) }) {
  require(typeof directory === 'string' && path.isAbsolute(directory) && path.resolve(directory) === directory,
    'Grok lifecycle control directory must be an absolute normalized path');
  require(typeof session === 'string' && /^[0-9a-f]{32}$/.test(session), 'Missing Grok lifecycle session');
  require(Number.isSafeInteger(uid) && uid >= 0, 'Grok lifecycle requires a numeric uid');
  const held = await io.open(directory, constants.O_RDONLY | constants.O_DIRECTORY | constants.O_NOFOLLOW);
  let nextIndex = 0;
  let identity;
  try { identity = await held.stat({ bigint: true }); }
  catch (error) { await held.close(); throw error; }
  const regular = stat => stat.isFile() && stat.uid === BigInt(uid) && (stat.mode & 0o777n) === 0o600n;
  async function checkDirectory() {
    const current = await io.lstat(directory, { bigint: true });
    const pinned = await held.stat({ bigint: true });
    require(current.isDirectory() && pinned.isDirectory() && sameIdentity(current, identity)
      && sameIdentity(pinned, identity) && current.uid === BigInt(uid) && pinned.uid === BigInt(uid)
      && (current.mode & 0o777n) === 0o700n && (pinned.mode & 0o777n) === 0o700n,
    'Grok lifecycle directory identity or ownership changed');
  }
  async function requireAbsent(name) {
    try { await io.lstat(path.join(directory, name), { bigint: true }); }
    catch (error) { if (error.code === 'ENOENT') return; throw error; }
    throw new Error('Preexisting Grok lifecycle record');
  }
  async function readAck(name, index) {
    const filename = path.join(directory, name);
    // NONBLOCK makes a substituted FIFO fail the regular-file check, not hang.
    let handle;
    try { handle = await io.open(filename, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK); }
    catch (error) { if (error.code === 'ENOENT') return null; throw error; }
    try {
      // The supervisor publishes by link then unlink. Inspect its fixed staging
      // alias only AFTER opening ack: unlink legitimately changes inode ctime.
      // No stable-file snapshot/read begins until that publication is complete.
      let staging;
      try { staging = await io.lstat(path.join(directory, `.ack-${index}.tmp`), { bigint: true }); }
      catch (error) { if (error.code !== 'ENOENT') throw error; }
      if (staging) {
        require(regular(staging) && staging.size <= BigInt(MAX_RECORD), 'Invalid acknowledgement staging file');
        return null;
      }
      const before = await handle.stat({ bigint: true });
      require(regular(before) && before.size <= BigInt(MAX_RECORD), 'Invalid acknowledgement file');
      const buffer = Buffer.alloc(MAX_RECORD + 1);
      const { bytesRead } = await handle.read(buffer, 0, buffer.length, 0);
      const after = await handle.stat({ bigint: true });
      const named = await io.lstat(filename, { bigint: true });
      require(regular(after) && regular(named) && sameFile(before, after) && sameFile(after, named)
        && bytesRead === Number(after.size) && bytesRead <= MAX_RECORD,
      'Grok lifecycle acknowledgement identity changed');
      return buffer.subarray(0, bytesRead);
    } finally { await handle.close(); }
  }
  try {
    await checkDirectory();
    require((await io.readdir(directory)).length === 0, 'Grok lifecycle control directory is not fresh');
  } catch (error) { await held.close(); throw error; }
  return {
    async checkpoint(outcome, index) {
      require(index === nextIndex && index < GROK_FIXTURES.length, 'Duplicate or out-of-order Grok lifecycle checkpoint');
      const request = { schema: 1, session, index, pid: outcome.pid, exitCode: outcome.exitCode };
      validRequest(request);
      await checkDirectory();
      await requireAbsent(`ack-${index}.json`);
      const temporary = path.join(directory, `.request-${index}.tmp`);
      const published = path.join(directory, `request-${index}.json`);
      const handle = await io.open(temporary,
        constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW, 0o600);
      let written;
      try {
        const before = await handle.stat({ bigint: true });
        require(regular(before), 'Invalid lifecycle request file');
        await handle.writeFile(canonicalRecord(request));
        await handle.sync();
        written = await handle.stat({ bigint: true });
        require(regular(written) && sameIdentity(before, written)
          && written.size === BigInt(Buffer.byteLength(canonicalRecord(request))), 'Lifecycle request identity changed');
      } finally { await handle.close(); }
      await checkDirectory();
      require(sameFile(written, await io.lstat(temporary, { bigint: true })), 'Lifecycle request path changed');
      // link is an atomic, no-overwrite publication; rename would replace a
      // preexisting record. The supervisor ignores the finite temporary name.
      await io.link(temporary, published);
      // link changes ctime, so compare the two names to each other after linking.
      const linked = await io.lstat(temporary, { bigint: true });
      require(regular(linked) && sameIdentity(written, linked)
        && sameFile(linked, await io.lstat(published, { bigint: true })), 'Lifecycle request publication changed');
      await io.unlink(temporary);
      await checkDirectory();
      const expires = now() + ACK_WAIT_MS;
      while (now() < expires) {
        await checkDirectory();
        const bytes = await readAck(`ack-${index}.json`, index);
        if (bytes) {
          const ack = parseAcknowledgement(bytes, request);
          await checkDirectory();
          nextIndex += 1;
          return ack;
        }
        await delay(25);
      }
      throw new Error('Grok lifecycle acknowledgement deadline');
    },
    async close() { await held.close(); },
  };
}

export async function runFixtureBatch({ launch, checkpoint }) {
  const failures = [];
  for (const [index, fixture] of GROK_FIXTURES.entries()) {
    const outcome = await launch(fixture);
    require(positiveInteger(outcome.pid) && Number.isInteger(outcome.exitCode)
      && outcome.exitCode >= 0 && outcome.exitCode <= 255, 'Unknown or signaled Grok fixture outcome');
    // close establishes only this child's exit. The fresh supervisor ACK is
    // required before another child is started; ACK is not an assertion PASS.
    await checkpoint(outcome, index);
    if (outcome.exitCode !== 0) failures.push(new Error(`${fixture} exited ${outcome.exitCode}`));
  }
  if (failures.length) throw new AggregateError(failures, 'Grok editor/catalog fixture batch failed');
}

export async function runSupervisedGrokBatch({ argv = process.argv.slice(2), env = process.env,
  spawnChild = spawn, io = fs, uid = process.getuid(), now, delay } = {}) {
  const { values } = parseArgs({ args: argv, options: { browser: { type: 'string' }, output: { type: 'string' } } });
  require(typeof values.browser === 'string' && path.isAbsolute(values.browser)
    && typeof values.output === 'string' && path.isAbsolute(values.output), 'Explicit browser and output paths required');
  const control = await openBatchControl({ directory: env.GROK_BATCH_CONTROL_DIR,
    session: env.GROK_BATCH_SESSION, io, uid, now, delay });
  const childEnv = { ...env };
  delete childEnv.GROK_BATCH_CONTROL_DIR;
  delete childEnv.GROK_BATCH_SESSION;
  try {
    await runFixtureBatch({ checkpoint: control.checkpoint, launch: fixture => new Promise((resolve, reject) => {
      const child = spawnChild(process.execPath,
        [path.join(root, 'scripts', fixture), '--browser', values.browser, '--output', values.output],
        { cwd: root, env: childEnv, stdio: ['ignore', 'inherit', 'inherit'], shell: false, detached: false, windowsHide: true });
      child.once('error', reject);
      child.once('close', (code, signal) => {
        if (signal || !Number.isInteger(code)) reject(new Error('Unknown or signaled Grok fixture outcome'));
        else resolve({ pid: child.pid, exitCode: code });
      });
    }) });
  } finally { await control.close(); }
}
