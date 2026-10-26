/// <reference types="node" />
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import ts from 'typescript';
import { CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID, canDeleteClaudeDesktopProvider, isClaudeDesktopPinnedOfficialProvider } from '../../../../../features/coding/claudedesktop/utils/officialRestore.ts';

const page = ts.createSourceFile('page.tsx', readFileSync(new URL('../../../../../features/coding/claudedesktop/pages/ClaudeDesktopPage.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const card = ts.createSourceFile('card.tsx', readFileSync(new URL('../../../../../features/coding/claudedesktop/components/ClaudeDesktopProviderCard.tsx', import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function select(tree: ts.SourceFile, predicate: (node: ts.Node) => boolean): ts.Node {
  const found: ts.Node[] = [];
  const visit = (node: ts.Node) => { if (predicate(node)) found.push(node); ts.forEachChild(node, visit); };
  visit(tree); assert.equal(found.length, 1); return found[0];
}
function callback(name: string): string {
  const declaration = select(page, node => ts.isVariableDeclaration(node) && node.name.getText(page) === name) as ts.VariableDeclaration;
  const initializer = declaration.initializer!;
  return (ts.isCallExpression(initializer) ? initializer.arguments[0] : initializer).getText(page);
}
function evaluate(expression: string, values: Record<string, unknown> = {}): any {
  const bindings = { canDeleteClaudeDesktopProvider, isClaudeDesktopPinnedOfficialProvider, ...values };
  const code = ts.transpileModule(`return (${expression});`, { compilerOptions: { target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.React } }).outputText;
  return new Function(...Object.keys(bindings), code)(...Object.values(bindings));
}
const rows = [
  { id: CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID, category: 'custom', name: 'Seed with edited metadata' },
  { id: 'imported-official', category: 'official', name: 'Imported with synthetic credentials' },
  { id: 'copied-official', category: 'official', name: 'Copy' },
  { id: 'custom', category: 'custom', name: 'Custom' },
  { id: '', category: 'official', name: 'Unnamed' },
];
test('canonical pin ID matches the Rust constant and never follows category or ordering', () => {
  const rust = readFileSync(new URL('../../../../../../tauri/src/coding/claude_desktop/constants.rs', import.meta.url), 'utf8');
  assert.match(rust, new RegExp(`OFFICIAL_PROVIDER_ID: &str = "${CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID}"`));
  assert.deepEqual(rows.map(isClaudeDesktopPinnedOfficialProvider), [true, false, false, false, false]);
  assert.deepEqual(rows.map(canDeleteClaudeDesktopProvider), [false, true, true, true, false]);
});
test('actual Page batch selection and reason agree for the seed, imports, copies and unnamed rows', () => {
  const canSelect = evaluate(callback('canBatchDeleteProvider'));
  const reason = evaluate(callback('batchBlockReason'));
  assert.deepEqual(rows.map(canSelect), [false, true, true, true, false]);
  assert.deepEqual(rows.map(reason), ['official', undefined, undefined, undefined, 'unnamed']);
});
test('actual Page single-delete handler stops before confirmation for the seed', () => {
  let confirmations = 0;
  const remove = evaluate(callback('handleDeleteProvider'), { Modal: { confirm: () => confirmations++ }, t: (key: string) => key, React: { createElement: () => ({}) }, ExclamationCircleOutlined: () => null });
  for (const row of rows) remove(row);
  assert.equal(confirmations, 3);
});
test('actual shared-card delete action and detail footer use the same canonical guard', () => {
  const property = select(card, node => ts.isPropertyAssignment(node) && node.name.getText(card) === 'onDelete') as ts.PropertyAssignment;
  const footer = select(card, node => ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.AmpersandAmpersandToken && node.right.getText(card).includes("{t('common.delete')}")) as ts.BinaryExpression;
  for (const row of rows) {
    let calls = 0;
    const action = evaluate(property.initializer.getText(card), { provider: row, onDelete: () => calls++ });
    assert.equal(typeof action === 'function', canDeleteClaudeDesktopProvider(row));
    action?.(); assert.equal(calls, canDeleteClaudeDesktopProvider(row) ? 1 : 0);
    assert.equal(evaluate(footer.left.getText(card), { provider: row }), canDeleteClaudeDesktopProvider(row));
  }
});
