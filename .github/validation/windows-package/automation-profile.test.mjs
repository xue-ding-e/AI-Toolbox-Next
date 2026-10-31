import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, rmSync, symlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { snapshot, shellBaseline, assertShellBaseline, assertShellProgress } from './automation-profile.mjs';

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

test('shell progress accepts only exact six markers and one terminal newline', () => {
  const good = ['shell-fixture:add-type:start', 'shell-fixture:add-type:complete', 'shell-fixture:probe:start', 'shell-fixture:probe:complete', 'shell-fixture:create:start', 'shell-fixture:create:complete'].join('\n') + '\n';
  assertShellProgress(good); assertShellProgress(good.replaceAll('\n', '\r\n'));
  for (const bad of ['\n' + good, good + '\n', good + 'unknown\n', good.trimEnd(), good.replace('probe:start', 'unknown')]) assert.throws(() => assertShellProgress(bad));
});
