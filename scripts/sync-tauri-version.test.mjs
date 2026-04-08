import test from 'node:test';
import assert from 'node:assert/strict';

import {
  updateCargoLockVersion,
  updateCargoTomlVersion,
  updatePackageJsonVersion,
} from './sync-tauri-version.mjs';

test('updateCargoLockVersion keeps a single trailing newline unchanged', () => {
  const before = [
    '[[package]]',
    'name = "ai-toolbox"',
    'version = "0.7.6"',
    'dependencies = [',
    ' "anyhow",',
    ']',
    '',
  ].join('\n');

  const result = updateCargoLockVersion(before, '0.7.7');

  assert.equal(result.changed, true);
  assert.equal(result.previousVersion, '0.7.6');
  assert.equal(result.content, before.replace('0.7.6', '0.7.7'));
});

test('updateCargoTomlVersion preserves multiple trailing newlines exactly', () => {
  const before = '[package]\nname = "ai-toolbox"\nversion = "0.7.6"\n\n\n';

  const result = updateCargoTomlVersion(before, '0.7.7');

  assert.equal(result.changed, true);
  assert.equal(result.content, '[package]\nname = "ai-toolbox"\nversion = "0.7.7"\n\n\n');
});

test('updatePackageJsonVersion preserves missing trailing newline', () => {
  const before = '{\n  "name": "ai-toolbox",\n  "version": "0.7.6"\n}';

  const result = updatePackageJsonVersion(before, '0.7.7');

  assert.equal(result.changed, true);
  assert.equal(result.content, '{\n  "name": "ai-toolbox",\n  "version": "0.7.7"\n}');
});

test('updateCargoLockVersion is a no-op when version is already synced', () => {
  const before = [
    '[[package]]',
    'name = "ai-toolbox"',
    'version = "0.7.7"',
    '',
  ].join('\n');

  const result = updateCargoLockVersion(before, '0.7.7');

  assert.equal(result.changed, false);
  assert.equal(result.previousVersion, '0.7.7');
  assert.equal(result.content, before);
});
