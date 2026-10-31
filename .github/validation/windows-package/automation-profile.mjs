import assert from 'node:assert/strict';
import { readdirSync, lstatSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { digest, run, boundedUtf8 } from './automation-smoke.mjs';

export function snapshot(path) {
  let info;
  try { info = lstatSync(path); } catch (error) { if (error.code === 'ENOENT') return null; throw error; }
  assert(!info.isSymbolicLink(), 'Synthetic tree must not contain links');
  if (info.isFile()) return digest(path);
  assert(info.isDirectory(), 'Unsupported synthetic file type');
  return Object.fromEntries(readdirSync(path).sort().map(name => [name, snapshot(join(path, name))]));
}

// Identity of OS-created metadata observed on windows-2022 in run 41194174006.
// These are NOT ignored files: initialization must produce this exact tree,
// and every subsequent installer/CLI/uninstaller snapshot must remain equal.
export const shellBaseline = {
  AppData: { Roaming: { Microsoft: { Windows: { 'Start Menu': {
    'desktop.ini': '8e00a24ae458effe00a55344f7f34189b4594613284745ff7d406856a196c531',
    Programs: { 'desktop.ini': '768b6fde6149d9ebbed1e339a72e8cc8c535e5c61d7c82752f7dff50923b7aed' },
  } } } } },
  Desktop: { 'desktop.ini': '4b9d687ac625690fd026ed4b236dad1cac90ef69e7ad256cc42766a065b50026' },
};
export function assertShellBaseline(value) { assert.deepEqual(value, shellBaseline, 'Unexpected Windows shell initialization'); }
export function assertShellProgress(stderr) {
  const expected = ['shell-fixture:add-type:start', 'shell-fixture:add-type:complete', 'shell-fixture:probe:start', 'shell-fixture:probe:complete', 'shell-fixture:create:start', 'shell-fixture:create:complete'].join('\n') + '\n';
  assert.equal(stderr.replace(/\r\n/g, '\n'), expected, 'Unexpected shell progress output');
}
export async function initializeShellProfile(env) {
  assert.equal(process.platform, 'win32');
  assert.equal(snapshot(env.HOME), null, 'Never reset an existing profile baseline');
  const command = join(env.SystemRoot, 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe');
  console.log(JSON.stringify({ phase: 'shell-profile-initialize', state: 'start' }));
  let result;
  try {
    result = await run(command, ['-NoProfile', '-NonInteractive', '-File', resolve('.github/validation/windows-package/windows-shell-fixture.ps1'), '-SyntheticHome', env.HOME], env, '', 30000, false, true);
  } catch (error) {
    console.error(JSON.stringify({ phase: 'shell-profile-initialize', error: error.message, process: error.processEvidence }));
    throw error;
  }
  console.log(JSON.stringify({ phase: 'shell-profile-initialize', state: 'exit', code: result.code, signal: result.signal, stderr: boundedUtf8(result.err) }));
  assert.equal(result.code, 0); assert.equal(result.signal, null);
  assertShellProgress(result.err);
  assert.deepEqual(JSON.parse(result.out), { initialized: true, home: env.HOME, csidl: [16, 11, 2] });
  const baseline = snapshot(env.HOME);
  assertShellBaseline(baseline);
  return baseline;
}
