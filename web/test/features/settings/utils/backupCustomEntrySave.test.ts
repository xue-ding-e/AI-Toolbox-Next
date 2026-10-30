import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';
import { normalizeBackupEntryPath } from '../../../../features/settings/utils/normalizeBackupEntryPath.ts';

// Execute the real component closures with synthetic boundaries, not a copied
// save algorithm. This checks handler semantics, not DOM/native acceptance.
const source = ts.createSourceFile('BackupSettingsModal.tsx', readFileSync(
  new URL('../../../../features/settings/components/BackupSettingsModal.tsx', import.meta.url), 'utf8',
), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function closure(name: string): string {
  const matches: string[] = [];
  function visit(node: ts.Node) {
    if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name)
      && node.name.text === name && node.initializer) matches.push(node.initializer.getText(source));
    ts.forEachChild(node, visit);
  }
  visit(source);
  assert.equal(matches.length, 1, `exactly one production ${name}`);
  return matches[0];
}
const handler = ts.transpileModule(`
const describeBackendError = ${closure('describeBackendError')};
return ${closure('handleSaveCustomEntry')};
`, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;

function fixture(failAt?: 'validation' | 'source' | 'restore') {
  const messages: string[] = [];
  const paths: string[] = [];
  let entries = [{ id: 'existing', source_path: 'unchanged' }];
  let open = true;
  const failure = failAt === 'validation' ? { errorFields: [{ name: ['name'] }] }
    : JSON.stringify({ message: 'Synthetic normalization rejected', suggestion: 'retry' });
  const run = new Function('customEntryForm', 'normalizeCustomEntryPath', 'editingCustomEntry',
    'setCurrentBackupCustomEntries', 'setCustomEntryModalOpen', 'message', 't', handler)(
    { validateFields: async () => {
      if (failAt === 'validation') throw failure;
      return { name: ' Synthetic ', sourcePath: ' /source ', restorePath: ' /restore ', entryType: 'file' };
    } },
    (path: string) => normalizeBackupEntryPath(path, async (value) => {
      paths.push(value);
      if (value === `/${failAt}`) throw failure;
      return value;
    }),
    null,
    (update: (previous: unknown[]) => typeof entries) => { entries = update(entries); },
    (value: boolean) => { open = value; },
    { error: (value: string) => messages.push(value) },
    (key: string) => key,
  );
  return { run, messages, paths, state: () => ({ entries, open }) };
}

for (const stage of ['source', 'restore'] as const) {
  test(`${stage} normalization rejection displays detail and preserves draft`, async () => {
    const f = fixture(stage);
    await f.run();
    assert.deepEqual(f.messages, ['settings.backupSettings.saveFailed: Synthetic normalization rejected — retry']);
    assert.deepEqual(f.paths, stage === 'source' ? ['/source'] : ['/source', '/restore']);
    assert.deepEqual(f.state(), { entries: [{ id: 'existing', source_path: 'unchanged' }], open: true });
  });
}
test('field validation leaves feedback to the form without calling normalization', async () => {
  const f = fixture('validation');
  await f.run();
  assert.deepEqual(f.messages, []);
  assert.deepEqual(f.paths, []);
  assert.equal(f.state().open, true);
  assert.equal(f.state().entries.length, 1);
});
test('successful normalized entry is added once and closes its dialog', async () => {
  const f = fixture();
  await f.run();
  assert.deepEqual(f.messages, []);
  assert.deepEqual(f.paths, ['/source', '/restore']);
  assert.equal(f.state().open, false);
  assert.equal(f.state().entries.length, 2);
  assert.equal(f.state().entries[1].source_path, '/source');
});

const pickerHandler = ts.transpileModule(`
const describeBackendError = ${closure('describeBackendError')};
return ${closure('handleSelectCustomEntrySource')};
`, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
for (const outcome of ['cancel', 'picker-error', 'normalize-error', 'success']) {
  test(`source picker ${outcome} preserves cancellation and reports actual errors`, async () => {
    const messages: string[] = [];
    const writes: unknown[] = [];
    let normalizationCalls = 0;
    const run = new Function('open', 'customEntryType', 'customEntryForm',
      'normalizeCustomEntryPath', 'message', 't', pickerHandler)(
      async () => {
        if (outcome === 'picker-error') throw new Error('Synthetic picker failure');
        return outcome === 'cancel' ? null : '/synthetic-selection';
      }, 'file', { setFieldsValue: (value: unknown) => writes.push(value) },
      (path: string) => normalizeBackupEntryPath(path, async () => {
        normalizationCalls += 1;
        if (outcome === 'normalize-error') throw 'Synthetic path rejected';
        return '~/normalized';
      }), { error: (value: string) => messages.push(value) }, (key: string) => key,
    );
    await run();
    assert.deepEqual(writes, outcome === 'success' ? [{ sourcePath: '~/normalized' }] : []);
    assert.equal(normalizationCalls, ['success', 'normalize-error'].includes(outcome) ? 1 : 0);
    assert.deepEqual(messages, outcome === 'picker-error' ? ['Error: Synthetic picker failure']
      : outcome === 'normalize-error' ? ['Synthetic path rejected'] : []);
  });
}
