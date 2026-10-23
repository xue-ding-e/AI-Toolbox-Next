import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { buildInventory, extractHandlers, serializeInventory } from '../../../scripts/lib/operation-inventory.mjs';

const registry = (body) => `tauri::generate_handler![${body}]`;
const names = (source) => extractHandlers(source).map((entry) => entry.handler);

test('extracts paths, whitespace, trailing comma, raw identifiers and stable IDs', () => {
  const result = extractHandlers(registry('z::last, root, a :: r#type,'));
  assert.deepEqual(result.map((entry) => entry.handler), ['a::r#type', 'root', 'z::last']);
  assert.equal(result[0].source_id, 'tauri:a::r#type');
  assert.equal(extractHandlers(`\n${registry('root')}`)[0].line, 2);
});

test('ignores nested comments, documentation, strings, raw strings and chars', () => {
  const fake = registry('fake');
  const source = `// ${fake}\n/* outer /* ${fake} */ end */\nconst A: &str = "${fake}";\nconst B: &str = br##"${fake}"##;\nconst C: char = '\\'';\nfn f<'a>(s: &'a str) {}\n${registry('real /* nested /* x */ y */,')}`;
  assert.deepEqual(names(source), ['real']);
});

test('rejects missing, multiple, duplicate and unsupported registries instead of silently skipping', () => {
  for (const source of ['', `${registry('a')} ${registry('b')}`, registry('a, a'), registry('#[cfg(windows)] a'), registry('a b'), registry('a::'), 'tauri::generate_handler![a,']) {
    assert.throws(() => extractHandlers(source));
  }
  assert.throws(() => extractHandlers('/* unclosed'));
  assert.throws(() => extractHandlers('const X = r#"unclosed'));
});

test('adding or removing a handler changes inventory; every implementation remains pending', () => {
  assert.notEqual(serializeInventory(registry('a')), serializeInventory(registry('a,b')));
  const value = buildInventory(registry('a'));
  assert.equal(value.scope.business_coverage, 'incomplete');
  assert.equal(value.scope.tray, 'not_inventoried');
  assert.equal(value.summary.cli_implemented, 0);
  assert.equal(value.summary.mcp_implemented, 0);
  assert.equal(value.handlers[0].operation_id, null);
  assert.equal(value.unreviewed_contract.effects, 'unknown');
  assert.equal(value.unreviewed_contract.input_schema, null);
});

test('checked-in inventory exactly matches current source with deterministic generation', async () => {
  const source = await readFile(new URL('../../../tauri/src/lib.rs', import.meta.url), 'utf8');
  const saved = await readFile(new URL('../../../scripts/generated/operation-coverage.json', import.meta.url), 'utf8');
  assert.equal(saved, serializeInventory(source));
  assert.equal(serializeInventory(source), serializeInventory(source));
});

test('integrity check succeeds independently of failing complete-product gate', () => {
  const script = new URL('../../../scripts/generate-operation-coverage.mjs', import.meta.url);
  const check = spawnSync(process.execPath, [fileURLToPath(script), '--check'], { encoding: 'utf8', timeout: 5000 });
  assert.equal(check.status, 0, check.stderr);
  const complete = spawnSync(process.execPath, [fileURLToPath(script), '--require-complete'], { encoding: 'utf8', timeout: 5000 });
  assert.equal(complete.status, 2);
  assert.match(complete.stderr, /INCOMPLETE/);
});
