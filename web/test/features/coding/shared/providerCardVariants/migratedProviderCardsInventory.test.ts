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

test('the exact total assertion guard requires all 132 migration and 28 routing assertions', () => {
  const guards = nodes(checksAst, node => ts.isCallExpression(node) && node.arguments.some(argument =>
    ts.isStringLiteral(argument) && argument.text === 'Complete migration and Desktop routing coverage'));
  assert.equal(guards.length, 1);
  const guard = new Function('assert', 'checks', 'routingChecks', `${guards[0].getText(checksAst)};`);
  guard(assert, Array(132), Array(28));
  for (const [migrated, routing] of [[131, 28], [132, 27], [133, 28], [132, 29], [86, 28], [0, 0]]) {
    assert.throws(() => guard(assert, Array(migrated), Array(routing)), /Complete migration and Desktop routing coverage/);
  }
  const counts = new Function(variable('SCENARIO_COUNTS') + variable('DESKTOP_ROUTING_COUNTS')
    + 'return [SCENARIO_COUNTS, DESKTOP_ROUTING_COUNTS];')() as Record<string, number>[];
  assert.deepEqual(counts.map(group => Object.values(group).reduce((sum, count) => sum + count, 0)), [132, 28]);
  assert.deepEqual(counts.map(group => Object.keys(group).length), [11, 8]);
});
