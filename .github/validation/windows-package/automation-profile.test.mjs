import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, rmSync, symlinkSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { snapshot, shellBaseline, assertShellBaseline, assertShellProgress, assertFreshShellHome, SHELL_INITIALIZATION_TIMEOUT_MS } from './automation-profile.mjs';

test('exact independently observed shell metadata is accepted, not ignored', () => {
  assertShellBaseline(structuredClone(shellBaseline));
});
for (const [name, mutate] of [
  ['unexpected root file', tree => { tree['settings.json'] = 'unknown'; }],
  ['unexpected shortcut', tree => { tree.Desktop['AI Toolbox Next.lnk'] = 'unknown'; }],
  ['changed desktop.ini', tree => { tree.Desktop['desktop.ini'] = 'unknown'; }],
  ['missing Programs metadata', tree => { delete tree.AppData.Roaming.Microsoft.Windows['Start Menu'].Programs; }],
]) test(`shell baseline rejects ${name}`, () => {
  const changed = structuredClone(shellBaseline); mutate(changed);
  assert.throws(() => assertShellBaseline(changed));
});
test('snapshot observes changed or removed sentinels and unexpected files', () => {
  const root = mkdtempSync(join(tmpdir(), 'automation-profile-test-'));
  try {
    writeFileSync(join(root, 'sentinel'), 'preserve');
    const original = snapshot(root);
    writeFileSync(join(root, 'sentinel'), 'changed');
    assert.notDeepEqual(snapshot(root), original);
    writeFileSync(join(root, 'sentinel'), 'preserve');
    writeFileSync(join(root, 'unexpected'), 'state');
    assert.notDeepEqual(snapshot(root), original);
    rmSync(join(root, 'sentinel'));
    assert.notDeepEqual(snapshot(root), original);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
test('snapshot refuses links instead of following outside fixture', () => {
  const root = mkdtempSync(join(tmpdir(), 'automation-profile-link-'));
  try {
    symlinkSync(root, join(root, 'escape'), 'junction');
    assert.throws(() => snapshot(root), /links/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('snapshot refuses dangling links rather than treating them as absent', () => {
  const root = mkdtempSync(join(tmpdir(), 'automation-profile-dangling-'));
  try {
    symlinkSync(join(root, 'absent'), join(root, 'dangling'), 'junction');
    assert.throws(() => snapshot(root), /links/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('shell progress requires exact ordered phases and monotonic integer timings', () => {
  const phases = ['add-type:start', 'add-type:complete', 'initial-links:start', 'initial-links:complete', 'initial-absence:start', 'initial-absence:complete', 'probe:start', 'probe:complete', 'create:start', 'create:complete'];
  const good = phases.map((phase, index) => `shell-fixture:${phase} elapsed_ms=${index}`).join('\n') + '\n';
  assertShellProgress(good); assertShellProgress(good.replaceAll('\n', '\r\n'));
  for (const bad of ['\n' + good, good + '\n', good + 'unknown\n', good.trimEnd(), good.replace('probe:start', 'unknown'), good.replace('elapsed_ms=5', 'elapsed_ms=0'), good.replace('elapsed_ms=5', 'elapsed_ms=NaN'), good.replace('elapsed_ms=5', 'elapsed_ms=9007199254740992')]) assert.throws(() => assertShellProgress(bad));
});

test('parent pre-spawn guard rejects unsafe HOME and linked ancestors', () => {
  const root = mkdtempSync(join(tmpdir(), 'automation-profile-parent-'));
  const fresh = join(root, 'home');
  const environment = home => ({ HOME: home, USERPROFILE: home, APPDATA: join(root, 'roaming'), LOCALAPPDATA: join(root, 'local'), AI_TOOLBOX_DATA_DIR: join(root, 'data'), AI_TOOLBOX_CACHE_DIR: join(root, 'cache'), CODEX_HOME: join(root, 'codex') });
  try {
    assertFreshShellHome(environment(fresh), root);
    assert.throws(() => assertFreshShellHome({ HOME: fresh, USERPROFILE: root }, root), /USERPROFILE/);
    assert.throws(() => assertFreshShellHome(environment(root), root), /inside/);
    assert.throws(() => assertFreshShellHome(environment(join(root, '..', 'outside')), root), /inside/);
    mkdirSync(fresh);
    assert.throws(() => assertFreshShellHome(environment(fresh), root), /existing/);
    rmSync(fresh, { recursive: true });
    const outside = join(root, 'sentinel'); mkdirSync(outside);
    writeFileSync(join(outside, 'keep'), 'preserve');
    const before = snapshot(outside);
    const link = join(root, 'junction'); symlinkSync(outside, link, 'junction');
    assert.throws(() => assertFreshShellHome(environment(join(link, 'child')), root), /link or junction/);
    const dangling = join(root, 'dangling'); symlinkSync(join(root, 'absent'), dangling, 'junction');
    assert.throws(() => assertFreshShellHome(environment(dangling), root), /link or junction/);
    assert.throws(() => assertFreshShellHome({ ...environment(fresh), APPDATA: join(link, 'roaming') }, root), /link or junction/);
    assert.deepEqual(snapshot(outside), before);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test('cold shell compilation uses one bounded 90 second helper ceiling', () => {
  assert.equal(SHELL_INITIALIZATION_TIMEOUT_MS, 90000);
});
