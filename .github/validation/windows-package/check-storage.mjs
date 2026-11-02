import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { validateNativeResult } from './package.mjs';

export function validateList(log, exit, count, required = []) {
  assert.equal(exit, 0, 'Storage test listing failed');
  assert(Number.isSafeInteger(count) && count > 0, 'Invalid expected test count');
  const names = [...log.matchAll(/^([\w:]+): test\r?$/gm)].map(match => match[1]);
  assert.equal(names.length, count, 'Storage test selection count mismatch');
  assert.equal(new Set(names).size, count, 'Duplicate storage test selection');
  for (const name of required) assert(names.includes(name), 'Required storage regression not selected');
  return names;
}

export function validateRun(listLog, log, exit, count, required = []) {
  const selected = validateList(listLog, 0, count, required);
  validateNativeResult(log, exit, '', count);
  const passed = [...log.matchAll(/^test ([\w:]+) \.\.\. ok\r?$/gm)].map(match => match[1]);
  assert.deepEqual(passed.sort(), selected.sort(), 'Executed tests differ from the listed target');
  assert(log.includes(`${count} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;`), 'Storage result was filtered or ignored');
}

function main([mode, listFile, ...args]) {
  if (mode === 'list') {
    const [exit, count, ...required] = args;
    validateList(readFileSync(listFile, 'utf8'), Number(exit), Number(count), required);
  } else if (mode === 'result') {
    const [resultFile, exit, count, ...required] = args;
    validateRun(readFileSync(listFile, 'utf8'), readFileSync(resultFile, 'utf8'), Number(exit), Number(count), required);
  } else {
    throw new Error('Unknown storage validation mode');
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
