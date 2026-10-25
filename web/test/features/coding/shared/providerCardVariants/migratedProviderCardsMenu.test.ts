import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const source = readFileSync(new URL('./migratedProviderCardsBrowserChecks.mjs', import.meta.url), 'utf8');
function declaration(start: string, end: string): string {
  const first = source.indexOf(start);
  const last = source.indexOf(end, first);
  assert.ok(first >= 0 && last > first, 'Load the real browser-check helper, not a copied implementation');
  return source.slice(first, last);
}
const stableDeclaration = declaration('  const stableMenuTarget = async', '  const openDeleteMenu = async');
type Target = { x: number; y: number; width: number; height: number; inViewport: boolean; hit: boolean; opacity: number };
const visible = (changes: Partial<Target> = {}): Target => ({
  x: 1240, y: 450, width: 22, height: 22, inViewport: true, hit: true, opacity: 0, ...changes,
});
function setup(target: (reads: number, scrolls: number) => Target) {
  let reads = 0;
  let scrolls = 0;
  let waits = 0;
  const fixture = async (expression: string) => {
    if (expression === 'menuTarget("omo-other-card")') return target(++reads, scrolls);
    if (expression === 'scrollMenuTarget("omo-other-card")') { scrolls++; return undefined; }
    throw new Error('Unexpected action: ' + expression);
  };
  const delay = async (milliseconds: number) => { assert.equal(milliseconds, 50); waits++; };
  const factory = new Function('fixture', 'delay', stableDeclaration + '\nreturn stableMenuTarget;');
  const locate = factory(fixture, delay) as (name: string, revealed: boolean) => Promise<Target>;
  return { locate: (revealed = false) => locate('omo-other-card', revealed), stats: () => ({ reads, scrolls, waits }) };
}

test('resting target displaced below the viewport is repositioned before stable sampling', async () => {
  // The initial scroll already happened; an expanding catalog displaced the
  // button afterwards, matching the recorded y=942.375 / 900px viewport failure.
  const run = setup((_reads, scrolls) => scrolls ? visible() : visible({ y: 942.375, inViewport: false, hit: false }));
  assert.equal((await run.locate()).y, 450);
  assert.equal(run.stats().scrolls, 1);
  assert.ok(run.stats().reads >= 6, 'Keep five stationary intervals after repositioning');
});

test('already visible resting target keeps the original stable-sample requirement without scrolling', async () => {
  const run = setup(() => visible());
  await run.locate();
  assert.deepEqual(run.stats(), { reads: 6, scrolls: 0, waits: 5 });
});

test('continuous layout movement never passes the stationary check', async () => {
  const run = setup(reads => visible({ x: 1200 + (reads % 2) * 10 }));
  await assert.rejects(run.locate(), /never settled or was not hit-testable/);
  assert.deepEqual(run.stats(), { reads: 200, scrolls: 0, waits: 200 });
});

test('an occluded in-viewport target cannot be scrolled past the hit-test assertion', async () => {
  const run = setup(() => visible({ hit: false }));
  await assert.rejects(run.locate(), /never settled or was not hit-testable/);
  assert.deepEqual(run.stats(), { reads: 200, scrolls: 0, waits: 200 });
});

test('revealed target must really reach opacity one without forcing styles or scrolling', async () => {
  const run = setup(() => visible({ opacity: 0 }));
  await assert.rejects(run.locate(true), /never settled or was not hit-testable/);
  assert.deepEqual(run.stats(), { reads: 200, scrolls: 0, waits: 200 });
});

test('unreachable resting target still fails at the original 200-sample limit', async () => {
  const run = setup(() => visible({ y: 942.375, inViewport: false, hit: false }));
  await assert.rejects(run.locate(), /never settled or was not hit-testable/);
  assert.equal(run.stats().reads, 200);
  assert.equal(run.stats().waits, 200);
  assert.ok(run.stats().scrolls <= 200);
});

test('positioning never auto-scrolls after reveal has begun', async () => {
  const run = setup(() => visible({ y: 942.375, inViewport: false, hit: false }));
  await assert.rejects(run.locate(true), /never settled or was not hit-testable/);
  assert.deepEqual(run.stats(), { reads: 200, scrolls: 0, waits: 200 });
});

test('stable revealed target remains accepted without repositioning', async () => {
  const run = setup(() => visible({ opacity: 1 }));
  await run.locate(true);
  assert.deepEqual(run.stats(), { reads: 6, scrolls: 0, waits: 5 });
});

test('real menu-opening helper still performs exactly one pointer click after reveal', async () => {
  const block = declaration('  const openDeleteMenu = async', '  const checkSurfaces = async');
  const actions: string[] = [];
  const factory = new Function('waitFor', 'visibleMenus', 'evaluate', 'fixture', 'stableMenuTarget', 'mouse', 'assert', block + '\nreturn openDeleteMenu;');
  const open = factory(
    async () => undefined,
    'visibleMenus',
    async () => false,
    async (expression: string) => {
      actions.push(expression);
      if (expression === 'deleteMenuState()') return { disabled: false };
      assert.equal(expression, 'scrollMenuTarget("omo-other-card")');
    },
    async (_name: string, revealed: boolean) => { actions.push('stable:' + revealed); return visible({ opacity: revealed ? 1 : 0 }); },
    async (kind: string) => { actions.push(kind); },
    assert,
  ) as (name: string) => Promise<{ disabled: boolean }>;
  assert.equal((await open('omo-other-card')).disabled, false);
  assert.deepEqual(actions, [
    'scrollMenuTarget("omo-other-card")', 'stable:false', 'mouseMoved', 'stable:true',
    'mouseMoved', 'mousePressed', 'mouseReleased', 'deleteMenuState()',
  ]);
});
