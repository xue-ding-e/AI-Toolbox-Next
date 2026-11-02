import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import { validateList, validateRun } from './check-storage.mjs';

const list = 'module::first: test\nsecond: test\n\n2 tests, 0 benchmarks\n';
const result = 'running 2 tests\ntest module::first ... ok\ntest second ... ok\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.1s\n';

test('Windows storage gates remain serial between existing native tests and packaging', () => {
  const workflow = readFileSync(new URL('../../workflows/windows-manual-build.yml', import.meta.url), 'utf8');
  const windows = workflow.split('  windows:')[1].split('  publish:')[0];
  assert(windows.indexOf('test-native.ps1') < windows.indexOf('test-storage.ps1'));
  assert(windows.indexOf('test-storage.ps1') < windows.indexOf('pnpm tauri build'));
  assert.match(windows, /Windows file-backed storage and desktop adapter regressions\n        timeout-minutes: 25/);
  assert.match(windows, /storage-tests-\*\.log/);
  assert.match(windows, /storage\.test\.mjs/);
});

test('accepts the exact selected and executed target including Windows line endings', () => {
  assert.deepEqual(validateList(list, 0, 2, ['second']), ['module::first', 'second']);
  validateRun(list.replaceAll('\n', '\r\n'), result.replaceAll('\n', '\r\n'), 0, 2, ['second']);
});

for (const [name, input, exit, count, required] of [
  ['failed listing', list, 101, 2, []],
  ['short selection', list.replace('second: test\n', ''), 0, 2, []],
  ['duplicate selection', list.replace('second: test', 'module::first: test'), 0, 2, []],
  ['missing regression', list, 0, 2, ['absent']],
  ['invalid count', list, 0, Number.NaN, []],
]) test(`rejects ${name}`, () => assert.throws(() => validateList(input, exit, count, required)));

for (const [name, input, exit] of [
  ['failed process', result, 101],
  ['different executed test', result.replace('test second ... ok', 'test other ... ok'), 0],
  ['ignored test', result.replace('0 ignored', '1 ignored'), 0],
  ['filtered target', result.replace('0 filtered out', '1 filtered out'), 0],
  ['missing summary', result.slice(0, result.indexOf('test result:')), 0],
  ['interrupted execution', result.replace('test second ... ok\n', ''), 0],
]) test(`rejects ${name}`, () => assert.throws(() => validateRun(list, input, exit, 2)));
