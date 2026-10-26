import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const checksSource = readFileSync(new URL('./migratedProviderCardsBrowserChecks.mjs', import.meta.url), 'utf8');
const fixtureSource = readFileSync(new URL('./fixtures/MigratedProviderCardsFixture.jsx', import.meta.url), 'utf8');
const checksAst = ts.createSourceFile('checks.mjs', checksSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
const fixtureAst = ts.createSourceFile('fixture.jsx', fixtureSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.JSX);
function nodes(root: ts.Node, match: (node: ts.Node) => boolean): ts.Node[] {
  const found: ts.Node[] = [];
  function visit(node: ts.Node) { if (match(node)) found.push(node); node.forEachChild(visit); }
  visit(root);
  return found;
}
function variable(name: string): string {
  const matches = nodes(checksAst, node => ts.isVariableDeclaration(node) && node.name.getText(checksAst) === name);
  assert.equal(matches.length, 1, `Exactly one real ${name} declaration`);
  return `const ${matches[0].getText(checksAst)};`;
}
const originalSurfaces = ['claude-desktop', 'gemini', 'grok-official', 'grok-custom', 'omo-default', 'omo-other']
  .flatMap(name => ['list', 'card'].map(view => `${name}-${view}`));

function mountedFixtureIds(): string[] {
  const declarations = nodes(fixtureAst, node => ts.isFunctionDeclaration(node) && node.name?.text === 'FixtureHost');
  assert.equal(declarations.length, 1);
  // Evaluate the actual host's loops/conditionals/JSX, but never invoke product
  // components or mount a browser. This checks inventory, not rendered behavior.
  const ids: string[] = [];
  const scope = new Proxy({
    React: {
      useState: () => [null, () => undefined],
      Fragment: Symbol('Fragment'),
      createElement: (_type: unknown, props: Record<string, unknown> | null) => {
        if (props?.['data-fixture'] !== undefined) ids.push(String(props['data-fixture']));
        return null;
      },
    },
    theme: {}, resolvedTheme: 'light', desktopGatewayCase: false,
    omoProviders: [{ key: 'default' }, { key: 'other' }],
  } as Record<string | symbol, unknown>, {
    has: () => true,
    get: (target, key) => key === Symbol.unscopables ? undefined : target[key],
  });
  const compiled = ts.transpileModule(declarations[0].getText(fixtureAst), {
    compilerOptions: { jsx: ts.JsxEmit.React, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  new Function('scope', `with (scope) { ${compiled}; FixtureHost(); }`)(scope);
  assert.equal(ids.length, 30, 'Fixture host inventory stays exactly thirty');
  assert.equal(new Set(ids).size, ids.length, 'Fixture host identities are unique');
  return ids;
}

function surfaceCheck(mounted: string[]) {
  const declarations = checksSource.slice(checksSource.indexOf('const SCENARIO_COUNTS'), checksSource.indexOf('export async function verifyMigratedProviderCards'));
  const block = variable('checkSurfaces');
  const surfaces: string[] = [];
  const backgrounds: Record<string, string> = {};
  const helper = new Function('assert', 'evaluate', 'fixture', 'check', 'lightBackgrounds', declarations + block + '\nreturn checkSurfaces;')(
    assert,
    async (expression: string) => {
      assert.equal(expression, '[...document.querySelectorAll("[data-fixture]")].map(node => node.dataset.fixture).sort()');
      return [...mounted].sort();
    },
    async (expression: string) => {
      const match = /^surfaceStyle\("([^"]+)"\)$/.exec(expression);
      assert.ok(match); surfaces.push(match[1]);
      return { actualSurface: true, hasBox: true, background: 'rgb(255, 255, 255)', expectedBackground: 'rgb(255, 255, 255)' };
    },
    (_name: string, actual: unknown, expected: unknown) => assert.deepEqual(actual, expected),
    backgrounds,
  ) as (theme: string) => Promise<void>;
  return { run: () => helper('light'), surfaces };
}

test('actual host inventory accepts all thirty fixtures while preserving twelve original surface checks', async () => {
  const run = surfaceCheck(mountedFixtureIds());
  await run.run();
  assert.deepEqual(run.surfaces, originalSurfaces);
});

test('the former twelve-fixture-only inventory cannot silently omit the eighteen new fixtures', async () => {
  await assert.rejects(surfaceCheck(originalSurfaces).run(), /Complete (?:four-mapping\/two-view matrix|migrated-provider fixture inventory)/);
});

test('every missing fixture is rejected before any surface assertion', async () => {
  const ids = mountedFixtureIds();
  for (let index = 0; index < ids.length; index++) {
    const run = surfaceCheck(ids.filter((_id, i) => i !== index));
    await assert.rejects(run.run(), /Complete (?:four-mapping\/two-view matrix|migrated-provider fixture inventory)/);
    assert.equal(run.surfaces.length, 0);
  }
});

test('duplicate and unknown fixtures remain failures even when total length is thirty', async () => {
  const ids = mountedFixtureIds();
  for (const malformed of [[...ids, ids[0]], [...ids, 'unknown-extra'], [ids[0], ...ids.slice(0, -1)], ['unknown-extra', ...ids.slice(1)]]) {
    const run = surfaceCheck(malformed);
    await assert.rejects(run.run(), /Complete (?:four-mapping\/two-view matrix|migrated-provider fixture inventory)/);
    assert.equal(run.surfaces.length, 0);
  }
});

test('the exact total assertion guard requires all 134 migration and 28 routing assertions', () => {
  const guards = nodes(checksAst, node => ts.isCallExpression(node) && node.arguments.some(argument =>
    ts.isStringLiteral(argument) && argument.text === 'Complete migration and Desktop routing coverage'));
  assert.equal(guards.length, 1);
  const guard = new Function('assert', 'checks', 'routingChecks', `${guards[0].getText(checksAst)};`);
  guard(assert, Array(134), Array(28));
  for (const [migrated, routing] of [[133, 28], [134, 27], [135, 28], [134, 29], [132, 28], [86, 28], [0, 0]]) {
    assert.throws(() => guard(assert, Array(migrated), Array(routing)), /Complete migration and Desktop routing coverage/);
  }
  const counts = new Function(variable('SCENARIO_COUNTS') + variable('DESKTOP_ROUTING_COUNTS')
    + 'return [SCENARIO_COUNTS, DESKTOP_ROUTING_COUNTS];')() as Record<string, number>[];
  assert.deepEqual(counts.map(group => Object.values(group).reduce((sum, count) => sum + count, 0)), [134, 28]);
  assert.deepEqual(counts.map(group => Object.keys(group).length), [11, 8]);
});

function fixtureApi(): ts.ObjectLiteralExpression {
  const assignments = nodes(fixtureAst, node => ts.isBinaryExpression(node)
    && node.left.getText(fixtureAst) === 'window.migratedProviderCardsFixture') as ts.BinaryExpression[];
  assert.equal(assignments.length, 1);
  assert.ok(ts.isObjectLiteralExpression(assignments[0].right));
  return assignments[0].right;
}
function modelExpansion(options: { missing?: boolean; mounted?: boolean; expanded?: boolean } = {}) {
  const methods = fixtureApi().properties.filter(node => node.name?.getText(fixtureAst) === 'expandModels');
  assert.ok(methods.length > 0);
  let clicks = 0;
  const timers: Array<() => void> = [];
  const header = { click: () => { clicks++; }, getAttribute: (name: string) => {
    assert.equal(name, 'aria-expanded'); return options.expanded === false ? 'false' : 'true';
  } };
  const root = {
    querySelector: (selector: string) => { assert.equal(selector, '.ant-collapse-header'); return options.missing ? null : header; },
    querySelectorAll: (selector: string) => { assert.equal(selector, '[data-model-row]'); return options.mounted === false ? [] : [{}]; },
  };
  // Keep JavaScript's actual last-key-wins semantics, so the old duplicate's
  // undefined result is reproduced rather than testing the earlier dead method.
  const expand = new Function('cards', 'setTimeout', `return ({${methods.map(node => node.getText(fixtureAst)).join(',')}}).expandModels;`)(
    () => [root],
    (callback: () => void, milliseconds: number) => { assert.equal(milliseconds, 300); timers.push(callback); },
  ) as (name: string) => Promise<boolean>;
  return { expand: () => expand('hermes-list'), timers, clicks: () => clicks };
}

test('fixture API has no duplicate keys that silently shadow upstream or Next helpers', () => {
  const names = fixtureApi().properties.map(property => property.name?.getText(fixtureAst));
  assert.ok(names.every(name => name !== undefined));
  assert.equal(new Set(names).size, names.length);
});

test('the effective real expandModels helper clicks once and resolves true only after the existing wait', async () => {
  const run = modelExpansion();
  const pending = run.expand();
  assert.ok(pending instanceof Promise, 'Expansion must preserve its asynchronous result');
  let resolved = false;
  pending.then(() => { resolved = true; });
  await Promise.resolve();
  assert.equal(resolved, false);
  assert.equal(run.clicks(), 1);
  assert.equal(run.timers.length, 1);
  run.timers[0]();
  assert.equal(await pending, true);
  assert.equal(run.clicks(), 1, 'No retry click can toggle the fold closed');
});

test('missing real model header fails without clicking or scheduling a wait', async () => {
  const run = modelExpansion({ missing: true });
  await assert.rejects(Promise.resolve().then(run.expand), /Missing real model-list header/);
  assert.equal(run.clicks(), 0);
  assert.equal(run.timers.length, 0);
});

test('the helper does not report success when the fold stayed closed or rows never mounted', async () => {
  for (const options of [{ expanded: false }, { mounted: false }]) {
    const run = modelExpansion(options);
    const pending = run.expand();
    assert.equal(run.timers.length, 1);
    run.timers[0]();
    assert.equal(await pending, false);
    assert.equal(run.clicks(), 1);
  }
});

test('render-light inventory counts actual check calls, fixed loop iterations and original surfaces', () => {
  const main = checksAst.statements.find(node => ts.isFunctionDeclaration(node)
    && node.name?.text === 'verifyMigratedProviderCards') as ts.FunctionDeclaration;
  assert.ok(main?.body);
  const statements = [...main.body.statements];
  const first = statements.findIndex(node => node.getText(checksAst) === "await openFixture('theme=light');");
  const last = statements.findIndex(node => node.getText(checksAst) === "await finishScenario('render-light');");
  assert.ok(first >= 0 && last > first);
  let assertions = 0;
  let repeated = 0;
  for (const statement of statements.slice(first + 1, last)) {
    const calls = nodes(statement, node => ts.isCallExpression(node)
      && ts.isIdentifier(node.expression) && node.expression.text === 'check').length;
    const loops = nodes(statement, node => ts.isForOfStatement(node) || ts.isForStatement(node)
      || ts.isWhileStatement(node) || ts.isDoStatement(node));
    if (loops.length) {
      assert.equal(loops.length, 1, 'A new loop must have an explicitly reviewed assertion count');
      assert.ok(ts.isForOfStatement(statement) && ts.isArrayLiteralExpression(statement.expression));
      assert.equal(statement.expression.elements.length, 3);
      assert.equal(calls, 1);
      assertions += calls * statement.expression.elements.length;
      repeated += statement.expression.elements.length;
    } else {
      assertions += calls;
    }
    const surfaceCalls = nodes(statement, node => ts.isCallExpression(node)
      && ts.isIdentifier(node.expression) && node.expression.text === 'checkSurfaces');
    if (surfaceCalls.length) {
      assert.equal(surfaceCalls.length, 1);
      assert.equal(surfaceCalls[0].getText(checksAst), "checkSurfaces('light')");
      assertions += originalSurfaces.length;
    }
  }
  assert.equal(repeated, 3, 'Pi, OMP and DSH sidebar checks each execute');
  assert.equal(assertions, 87, 'The real light path includes all three loop iterations, not one');
  const counts = new Function(variable('SCENARIO_COUNTS') + 'return SCENARIO_COUNTS;')();
  assert.equal(counts['render-light'], assertions, 'Fixed inventory must match actual source execution cardinality');
});

// Project only assertion cardinality, not values or browser behavior. Preserve
// actual fixed loops/branch conditions and finish gates; unfamiliar counted
// control flow fails closed instead of silently omitting a path.
function projectedCounts(functionName: string): Record<string, number> {
  const declaration = checksAst.statements.find(node => ts.isFunctionDeclaration(node)
    && node.name?.text === functionName) as ts.FunctionDeclaration;
  assert.ok(declaration?.body);
  const isCounted = (node: ts.Node) => ts.isCallExpression(node) && ts.isIdentifier(node.expression)
    && ['check', 'checkSurfaces', 'finishScenario'].includes(node.expression.text);
  function project(node: ts.Statement): string {
    if (ts.isBlock(node)) return node.statements.map(project).join('\n').trim();
    if (ts.isVariableStatement(node)) {
      const selected = node.declarationList.declarations.filter(item => ['scenario', 'name'].includes(item.name.getText(checksAst)));
      if (selected.length) return 'const ' + selected.map(item => item.getText(checksAst)).join(',') + ';';
      // Local helpers are handled at their call sites, not as executed bodies.
      return '';
    }
    if (ts.isForOfStatement(node)) {
      const body = project(node.statement);
      if (!body) return '';
      assert.ok(ts.isArrayLiteralExpression(node.expression), 'Counted loops must have a fixed literal inventory');
      return `for (${node.initializer.getText(checksAst)} of ${node.expression.getText(checksAst)}) { ${body} }`;
    }
    if (ts.isIfStatement(node)) {
      const yes = project(node.thenStatement);
      const no = node.elseStatement ? project(node.elseStatement) : '';
      if (!yes && !no) return '';
      return `if (${node.expression.getText(checksAst)}) { ${yes} } else { ${no} }`;
    }
    if (ts.isTryStatement(node)) {
      assert.equal(node.catchClause ? nodes(node.catchClause, isCounted).length : 0, 0);
      assert.equal(node.finallyBlock ? nodes(node.finallyBlock, isCounted).length : 0, 0);
      return project(node.tryBlock);
    }
    if (ts.isExpressionStatement(node)) {
      const expression = ts.isAwaitExpression(node.expression) ? node.expression.expression : node.expression;
      if (ts.isCallExpression(expression)) {
        const call = expression.expression.getText(checksAst);
        if (call === 'check') return 'count++;';
        if (call === 'checkSurfaces') return `count += ${originalSurfaces.length};`;
        if (call === 'finishScenario') return `finish(${expression.arguments[0].getText(checksAst)});`;
        if (call === 'assert.equal' && expression.arguments[1]?.getText(checksAst) === 'DESKTOP_ROUTING_COUNTS[name]') return 'finish(name);';
      }
    }
    assert.equal(nodes(node, isCounted).length, 0, 'Unsupported counted statement: ' + node.getText(checksAst).slice(0, 100));
    return '';
  }
  const projection = declaration.body.statements.map(project).join('\n');
  return new Function('assert', `let count = 0; const totals = {}; const finish = name => {
    assert.ok(!Object.hasOwn(totals, name)); totals[name] = count; count = 0;
  }; ${projection}; assert.equal(count, 0); return totals;`)(assert);
}

test('every light, dark, interaction and Grok loop finish gate matches its actual source cardinality', () => {
  const expected = new Function(variable('SCENARIO_COUNTS') + 'return SCENARIO_COUNTS;')();
  assert.deepEqual(projectedCounts('verifyMigratedProviderCards'), expected);
  assert.equal(Object.keys(expected).length, 11);
  assert.equal(Object.values(expected as Record<string, number>).reduce((sum, value) => sum + value, 0), 134);
});

test('all eight Desktop routing branches retain their exact callback and availability assertion counts', () => {
  const expected = new Function(variable('DESKTOP_ROUTING_COUNTS') + 'return DESKTOP_ROUTING_COUNTS;')();
  assert.deepEqual(projectedCounts('verifyClaudeDesktopRouting'), expected);
  assert.equal(Object.keys(expected).length, 8);
  assert.equal(Object.values(expected as Record<string, number>).reduce((sum, value) => sum + value, 0), 28);
});
