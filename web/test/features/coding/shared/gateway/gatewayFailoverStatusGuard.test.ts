import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';
import vm from 'node:vm';

const source = readFileSync(
  new URL('../../../../../features/coding/shared/gateway/GatewayFailoverButton.tsx', import.meta.url),
  'utf8',
);

const sourceFile = ts.createSourceFile(
  'GatewayFailoverButton.tsx',
  source,
  ts.ScriptTarget.Latest,
  true,
  ts.ScriptKind.TSX,
);

let guardInitializer: string | undefined;
const visit = (node: ts.Node) => {
  if (
    ts.isVariableDeclaration(node)
    && ts.isIdentifier(node.name)
    && node.name.text === 'createGatewayStatusRevisionGuard'
    && node.initializer
  ) {
    guardInitializer = node.initializer.getText(sourceFile);
  }
  ts.forEachChild(node, visit);
};
visit(sourceFile);
assert.ok(guardInitializer, 'Gateway status guard should remain a source-level helper');

const compiled = ts.transpileModule(
  `const createGatewayStatusRevisionGuard = ${guardInitializer};\n({ createGatewayStatusRevisionGuard });`,
  { compilerOptions: { target: ts.ScriptTarget.ES2022 } },
).outputText;
const { createGatewayStatusRevisionGuard } = vm.runInNewContext(compiled) as {
  createGatewayStatusRevisionGuard: () => {
    beginRequest: () => { request: number; revision: number };
    invalidate: () => void;
    isCurrent: (token: { request: number; revision: number }) => boolean;
  };
};

test('a newer status request supersedes an older response', () => {
  const guard = createGatewayStatusRevisionGuard();
  const first = guard.beginRequest();
  const second = guard.beginRequest();

  assert.equal(guard.isCurrent(first), false);
  assert.equal(guard.isCurrent(second), true);
});

test('an authoritative status revision invalidates an in-flight response', () => {
  const guard = createGatewayStatusRevisionGuard();
  const pending = guard.beginRequest();

  guard.invalidate();

  assert.equal(guard.isCurrent(pending), false);
  assert.equal(guard.isCurrent(guard.beginRequest()), true);
});
