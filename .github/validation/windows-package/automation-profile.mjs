import assert from 'node:assert/strict';
import { readdirSync, lstatSync } from 'node:fs';
import { join, resolve, isAbsolute, relative, dirname } from 'node:path';
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
  const expected = ['add-type:start', 'add-type:complete', 'initial-links:start', 'initial-links:complete', 'initial-absence:start', 'initial-absence:complete', 'probe:start', 'probe:complete', 'create:start', 'create:complete'];
  const lines = stderr.replace(/\r\n/g, '\n').split('\n');
  assert.equal(lines.pop(), '', 'Shell progress must end in exactly one newline');
  assert.equal(lines.length, expected.length, 'Unexpected shell progress line count');
  let previous = -1;
  lines.forEach((line, index) => {
    const match = line.match(/^shell-fixture:([a-z-]+:(?:start|complete)) elapsed_ms=(0|[1-9][0-9]*)$/);
    assert(match, 'Malformed shell progress');
    assert.equal(match[1], expected[index], 'Unexpected shell progress order');
    const elapsed = Number(match[2]);
    assert(Number.isSafeInteger(elapsed) && elapsed >= previous, 'Non-monotonic shell progress');
    previous = elapsed;
  });
}
export function assertFreshShellHome(env, fixtureRoot) {
  assert(isAbsolute(fixtureRoot) && resolve(fixtureRoot) === fixtureRoot, 'Non-canonical fixture root');
  assert(isAbsolute(env.HOME) && resolve(env.HOME) === env.HOME, 'Non-canonical synthetic HOME');
  assert(!fixtureRoot.startsWith('\\\\') && !env.HOME.startsWith('\\\\'), 'Network profiles are not allowed');
  assert.equal(env.USERPROFILE, env.HOME, 'USERPROFILE must equal synthetic HOME');
  assert(lstatSync(fixtureRoot).isDirectory(), 'Fixture root must already exist');
  for (const key of ['HOME', 'APPDATA', 'LOCALAPPDATA', 'AI_TOOLBOX_DATA_DIR', 'AI_TOOLBOX_CACHE_DIR', 'CODEX_HOME']) {
    const path = env[key];
    assert(typeof path === 'string' && isAbsolute(path) && resolve(path) === path, 'Non-canonical synthetic profile path');
    const child = relative(fixtureRoot, path);
    assert(child && child !== '..' && !child.startsWith(`..${process.platform === 'win32' ? '\\' : '/'}`) && !isAbsolute(child), 'Profile path must remain inside the explicit fresh fixture root');
    for (let cursor = path; ;) {
      let info;
      try { info = lstatSync(cursor); } catch (error) { if (error.code !== 'ENOENT') throw error; }
      if (info) {
        assert(!info.isSymbolicLink(), 'Synthetic profile contains a link or junction');
        assert(info.isDirectory(), 'Synthetic profile ancestor is not a directory');
        if (key === 'HOME') assert.notEqual(cursor, path, 'Never reset an existing profile baseline');
      }
      const parent = dirname(cursor); if (parent === cursor) break; cursor = parent;
    }
  }
}
// Run 41203361227 hit the 30s child ceiling inside Add-Type. This is ONE legal
// helper invocation; the unchanged outer Job's 240s fixture deadline is total,
// not a promise that helper + installer + uninstaller receive their full limits.
export const SHELL_INITIALIZATION_TIMEOUT_MS = 90000;
export async function initializeShellProfile(env, fixtureRoot, invoke = run) {
  assert.equal(process.platform, 'win32');
  // A shell may initialize profile directories BEFORE executing its script.
  // Therefore reject every unsafe HOME/ancestor before starting ANY shell.
  assertFreshShellHome(env, fixtureRoot);
  const command = join(env.SystemRoot, 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe');
  console.log(JSON.stringify({ phase: 'shell-profile-initialize', state: 'start' }));
  let result;
  try {
    result = await invoke(command, ['-NoProfile', '-NonInteractive', '-File', resolve('.github/validation/windows-package/windows-shell-fixture.ps1'), '-SyntheticHome', env.HOME], env, '', SHELL_INITIALIZATION_TIMEOUT_MS, false, true);
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
