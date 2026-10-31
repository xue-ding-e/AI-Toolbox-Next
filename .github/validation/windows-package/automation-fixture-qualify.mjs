// Only qualifies Windows synthetic profile preparation against the pinned OLD
// GUI-only package. Never claims that the new CLI/MCP bundle passed acceptance.
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, statSync, realpathSync, symlinkSync, unlinkSync, rmSync } from 'node:fs';
import { join, isAbsolute, resolve } from 'node:path';
import { run, digest, boundedUtf8 } from './automation-smoke.mjs';
import { snapshot, initializeShellProfile } from './automation-profile.mjs';
assert.equal(process.platform, 'win32');
assert.equal(process.env.GITHUB_ACTIONS, 'true');
const [previous, evidence, ...extra] = process.argv.slice(2);
assert.equal(extra.length, 0); assert(isAbsolute(previous) && isAbsolute(evidence));
const previousHash = '1451c7cf8eec647edaaf284d6ab451a65b2433d2867cb201d4554310f590a658';
assert.equal(digest(previous), previousHash); assert.equal(statSync(previous).size, 15501542);
const root = mkdtempSync(join(process.env.RUNNER_TEMP, 'automation-fixture-'));
const home = join(root, 'home'), install = join(root, 'Installed App 中文');
const env = Object.fromEntries(['SystemRoot', 'WINDIR', 'COMSPEC', 'TEMP', 'TMP'].filter(k => process.env[k]).map(k => [k, process.env[k]]));
Object.assign(env, { HOME: home, USERPROFILE: home, APPDATA: join(root, 'roaming'), LOCALAPPDATA: join(root, 'local'), AI_TOOLBOX_DATA_DIR: join(root, 'data'), AI_TOOLBOX_CACHE_DIR: join(root, 'cache'), CODEX_HOME: join(root, 'codex') });
const keys = ['HOME', 'APPDATA', 'LOCALAPPDATA', 'AI_TOOLBOX_DATA_DIR', 'AI_TOOLBOX_CACHE_DIR', 'CODEX_HOME'];
const registry = join(env.SystemRoot, 'System32', 'reg.exe');
async function query(key, value, more = []) { return run(registry, ['query', key, '/v', value, ...more], env); }
async function paths() {
  const values = [];
  for (const key of ['HKCU\\Environment', 'HKLM\\SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment']) {
    const result = await query(key, 'Path'); assert([0, 1].includes(result.code)); values.push(result);
  }
  return values;
}
console.log(JSON.stringify({ phase: 'registry-path-snapshot', state: 'start' }));
const pathsBefore = await paths();
// Native CREATE-boundary negatives stay inside this one disposable fixture.
const outside = join(root, 'boundary-sentinel'); mkdirSync(outside);
writeFileSync(join(outside, 'preserve.fixture'), 'outside-home-preserve');
const outsideBefore = snapshot(outside);
async function rejectBoundary(phase, candidateHome, actualHome = candidateHome) {
  console.log(JSON.stringify({ phase, state: 'start' }));
  let spawnCount = 0;
  await assert.rejects(initializeShellProfile({ ...env, HOME: candidateHome, USERPROFILE: actualHome }, root, (...args) => {
    spawnCount++; return run(...args);
  }));
  assert.equal(spawnCount, 0, 'Unsafe profile must be rejected before any shell starts');
  assert.deepEqual(snapshot(outside), outsideBefore);
  console.log(JSON.stringify({ phase, state: 'rejected-before-spawn', spawn_count: spawnCount, outside_tree_unchanged: true }));
}
const dangling = join(root, 'dangling-home');
symlinkSync(join(root, 'absent-target'), dangling, 'junction');
await rejectBoundary('reject-dangling-home', dangling);
rmSync(dangling); assert.equal(existsSync(join(root, 'absent-target')), false);
const ancestor = join(root, 'linked-ancestor');
symlinkSync(outside, ancestor, 'junction');
await rejectBoundary('reject-ancestor-junction', join(ancestor, 'child'));
rmSync(ancestor);
await rejectBoundary('reject-mismatched-home', join(root, 'mismatched-home'), outside);
assert.equal(existsSync(join(root, 'mismatched-home')), false);
const baseline = await initializeShellProfile(env, root);
for (const key of keys.filter(key => key !== 'HOME')) assert.equal(snapshot(env[key]), null, `Shell initialization wrote outside HOME: ${key}`);
for (const key of keys.filter(key => key !== 'HOME')) {
  mkdirSync(env[key]); writeFileSync(join(env[key], 'preserve.fixture'), 'synthetic-preserve\n');
}
const frozen = Object.fromEntries(keys.map(key => [key, snapshot(env[key])]));
function preserved() { for (const key of keys) assert.deepEqual(snapshot(env[key]), frozen[key], key); }
// Native negative control: an unrecognized write is detected, not adopted into
// the baseline. Remove only this exact test-owned marker, never reset snapshots.
const unexpected = join(home, 'unexpected.fixture');
writeFileSync(unexpected, 'negative-control');
assert.throws(preserved);
unlinkSync(unexpected); preserved();
const desktopMetadata = join(home, 'Desktop', 'desktop.ini');
const originalMetadata = readFileSync(desktopMetadata);
const mutatedMetadata = Buffer.from(originalMetadata); mutatedMetadata[0] ^= 1;
writeFileSync(desktopMetadata, mutatedMetadata); assert.throws(preserved);
writeFileSync(desktopMetadata, originalMetadata); preserved();
let webview = false;
for (const key of ['HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'HKCU\\SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}']) {
  const result = await query(key, 'pv', ['/reg:64']); assert([0, 1].includes(result.code));
  const version = result.out.match(/pv\s+REG_SZ\s+([0-9]+\.[0-9]+\.[0-9]+\.[0-9]+)\s*$/m)?.[1];
  if (result.code === 0 && version && version !== '0.0.0.0') webview = true;
}
assert(webview, 'Preinstalled WebView2 required; never launch bootstrapper');
console.log(JSON.stringify({ phase: 'old-gui-install', state: 'start' }));
const installed = await run(previous, ['/S', '/NS', `/D=${install}`], env, '', 180000, true);
assert.equal(installed.code, 0); assert.equal(installed.signal, null); preserved();
assert.deepEqual(await paths(), pathsBefore);
const gui = join(install, 'ai-toolbox.exe'), uninstaller = join(install, 'uninstall.exe');
for (const path of [gui, uninstaller]) assert.equal(realpathSync(path).toLowerCase(), path.toLowerCase());
assert.equal(readFileSync(gui).subarray(0, 2).toString(), 'MZ');
assert.equal(existsSync(join(install, 'ai-toolbox-cli.exe')), false);
for (const [field, expected] of [['InstallLocation', install], ['UninstallString', uninstaller]]) {
  const result = await query('HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\AI Toolbox Next', field);
  assert.equal(result.code, 0);
  const value = result.out.match(new RegExp(`${field}\\s+REG_SZ\\s+"([^"\\r\\n]+)"\\s*$`, 'm'))?.[1];
  assert.equal(value?.toLowerCase(), expected.toLowerCase());
}
const guiIdentity = { sha256: digest(gui), bytes: statSync(gui).size };
const foreign = join(install, 'not-owned.fixture'); writeFileSync(foreign, 'preserve\n');
console.log(JSON.stringify({ phase: 'old-gui-uninstall', state: 'start' }));
const removed = await run(uninstaller, ['/S', `_?=${install}`], env, '', 180000, true);
assert.equal(removed.code, 0); assert.equal(removed.signal, null);
assert.equal(existsSync(gui), false); assert.equal(readFileSync(foreign, 'utf8'), 'preserve\n');
preserved(); assert.deepEqual(await paths(), pathsBefore);
writeFileSync(evidence, JSON.stringify({
  source_commit: process.env.SOURCE_SHA, control_commit: process.env.GITHUB_SHA,
  workflow_run: process.env.GITHUB_RUN_ID, workflow_attempt: process.env.GITHUB_RUN_ATTEMPT,
  fixture_only: true, bundled_cli_mcp_qualified: false, previous_installer_sha256: previousHash,
  shell_baseline: baseline, previous_gui: guiIdentity, unexpected_write_negative_control: true, desktop_metadata_negative_control: true,
  dangling_home_rejected: true, ancestor_junction_rejected: true, mismatched_home_rejected: true,
  old_gui_install_uninstall_verified: true, external_roots_preserved: true, path_preserved: true,
  foreign_file_preserved: true,
}, null, 2) + '\n');
