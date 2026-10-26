/// <reference types="node" />
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import ts from 'typescript';

// Execute the actual page-to-card expression, not a duplicated fixture mapping.
const source = readFileSync(new URL('../../../../../features/coding/hermes/pages/HermesPage.tsx', import.meta.url), 'utf8');
const tree = ts.createSourceFile('HermesPage.tsx', source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function cardExpression(name: string): string {
  const matches: string[] = [];
  function visit(node: ts.Node) {
    if ((ts.isJsxSelfClosingElement(node) || ts.isJsxOpeningElement(node)) && node.tagName.getText(tree) === 'HermesProviderCard') {
      for (const attr of node.attributes.properties) {
        if (ts.isJsxAttribute(attr) && attr.name.getText(tree) === name && attr.initializer && ts.isJsxExpression(attr.initializer) && attr.initializer.expression) {
          matches.push(attr.initializer.expression.getText(tree));
        }
      }
    }
    ts.forEachChild(node, visit);
  }
  visit(tree);
  assert.equal(matches.length, 1, `one actual HermesProviderCard ${name} mapping`);
  return matches[0];
}
for (const isReadOnly of [false, true]) {
  test(`Hermes connectivity callback remains available for readOnly=${isReadOnly}`, () => {
    const calls: string[] = [];
    const callback = new Function('isReadOnly', 'provider', 'handleOpenConnectivityTest', `return (${cardExpression('onTestModels')});`)(isReadOnly, { providerKey: 'synthetic' }, (key: string) => calls.push(key));
    assert.equal(typeof callback, 'function');
    callback();
    assert.deepEqual(calls, ['synthetic']);
  });
}
test('Hermes connectivity still requires both URL and models', () => {
  const disabled = new Function('baseUrl', 'hasModels', `return (${cardExpression('testModelsDisabled')});`);
  assert.equal(disabled('https://example.invalid', true), false);
  assert.equal(disabled('', true), true);
  assert.equal(disabled('https://example.invalid', false), true);
});
test('readonly model removal keeps its existing fail-closed guard', () => {
  assert.match(source, /if \(!provider \|\| provider\.isReadOnly\)\s*\{\s*return;/);
});
