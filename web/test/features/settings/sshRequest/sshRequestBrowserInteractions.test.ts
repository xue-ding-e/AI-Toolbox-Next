import assert from 'node:assert/strict';
import test, { type TestContext } from 'node:test';
import vm from 'node:vm';

interface MouseEvent {
  type: 'mousePressed' | 'mouseReleased';
  button: string;
  clickCount: number;
  x: number;
  y: number;
}

interface InteractionOptions {
  send: (method: string, event: MouseEvent) => Promise<void>;
  evaluate: (expression: string) => Promise<unknown>;
  snapshot: () => Promise<{ calls: Array<{ id: string; settled: boolean }> }>;
  drain: () => Promise<void>;
  onPhase: (phase: string, category: string) => void;
}

const helperUrl = new URL('./sshRequestBrowserInteractions.mjs', import.meta.url);
const { createSSHFixtureInteractions } = await import(helperUrl.href) as {
  createSSHFixtureInteractions: (options: InteractionOptions) => {
    click: (selector: string, exactText?: string) => Promise<void>;
    chooseB: () => Promise<void>;
  };
};

interface Scenario {
  invisible?: 'geometry' | 'visibility' | 'opacity';
  occluded?: boolean;
  disabled?: boolean;
  finiteMotion?: 'running' | 'pending';
  unstable?: boolean;
  transient?: boolean;
  emitB?: boolean;
}

function createHarness(context: TestContext, scenario: Scenario = {}) {
  // Only accelerate the driver's polling delay in this isolated Node test file.
  // The VM executes the actual serialized sampler, not a copy of its predicate.
  context.mock.method(globalThis, 'setTimeout', (callback: () => void) => {
    queueMicrotask(callback);
    return 0;
  });
  let samples = 0;
  let drains = 0;
  const events: MouseEvent[] = [];
  const phases: Array<{ phase: string; category: string }> = [];
  const animation = (endTime: number, pending = false) => ({
    playState: pending ? 'paused' : 'running', pending,
    effect: { getComputedTiming: () => ({ endTime }) },
  });
  const ancestor = {
    parentElement: null,
    getAnimations(...args: unknown[]) {
      assert.equal(args.length, 0, 'do not include unrelated descendant animations');
      if (scenario.finiteMotion || (scenario.transient && samples <= 5)) {
        return [animation(100, scenario.finiteMotion === 'pending')];
      }
      return [animation(Infinity)];
    },
  };
  const target = {
    parentElement: ancestor,
    textContent: 'Connection B',
    closest: () => scenario.disabled ? {} : null,
    contains: (value: unknown): boolean => value === target,
    getAnimations(...args: unknown[]) {
      assert.equal(args.length, 0, 'do not include unrelated descendant animations');
      return [animation(Infinity)];
    },
    getBoundingClientRect: () => ({
      x: scenario.unstable ? samples : scenario.transient && samples < 7 ? 10 : 11,
      y: 20,
      width: scenario.invisible === 'geometry' || (scenario.transient && samples <= 2) ? 0 : 100,
      height: 20,
    }),
  };
  const sandbox = vm.createContext({
    requestAnimationFrame: (callback: () => void) => { samples++; callback(); },
    innerWidth: 1280,
    innerHeight: 900,
    getComputedStyle: (element: unknown) => ({
      visibility: element === ancestor && (scenario.invisible === 'visibility' || (scenario.transient && samples === 3)) ? 'hidden' : 'visible',
      opacity: element === ancestor && scenario.invisible === 'opacity' ? '0' : '1',
      pointerEvents: 'auto',
    }),
    document: {
      querySelectorAll: () => [target],
      elementFromPoint: () => scenario.occluded ? null : target,
      getAnimations: () => { throw Error('must not wait on whole-document motion'); },
    },
  });
  const actions = createSSHFixtureInteractions({
    evaluate: (expression) => vm.runInContext(expression, sandbox),
    send: async (method, event) => {
      assert.equal(method, 'Input.dispatchMouseEvent');
      events.push(event);
    },
    snapshot: async () => ({ calls: scenario.emitB !== false && events.length === 4
      ? [{ id: 'B', settled: false }] : [{ id: 'A', settled: false }] }),
    drain: async () => { drains++; },
    onPhase: (phase, category) => phases.push({ phase, category }),
  });
  return { actions, events, phases, get samples() { return samples; }, get drains() { return drains; } };
}

for (const invisible of ['geometry', 'visibility', 'opacity'] as const) {
  test(`SSH click rejects ${invisible} invisibility without dispatch`, async (context) => {
    const harness = createHarness(context, { invisible });
    await assert.rejects(harness.actions.click('.ant-modal .ant-select'), /connection-select: click readiness deadline \(visible target count 0\)/);
    assert.equal(harness.samples, 200);
    assert.equal(harness.events.length, 0);
  });
}

for (const [scenario, reason] of [
  [{ occluded: true }, /target occluded or outside viewport/],
  [{ disabled: true }, /target disabled or pointer events blocked/],
  [{ finiteMotion: 'running' }, /target or ancestor finite motion/],
  [{ finiteMotion: 'pending' }, /target or ancestor finite motion/],
  [{ unstable: true }, /click point not stable across three animation frames/],
] as Array<[Scenario, RegExp]>) {
  test(`SSH click rejects ${Object.keys(scenario)[0]} ${Object.values(scenario)[0]} without dispatch`, async (context) => {
    const harness = createHarness(context, scenario);
    await assert.rejects(harness.actions.click('.ant-modal .ant-select'), reason);
    assert.equal(harness.samples, 200);
    assert.equal(harness.events.length, 0);
  });
}

test('SSH click waits for transient readiness and three stable frames before one CDP click', async (context) => {
  const harness = createHarness(context, { transient: true });
  await harness.actions.click('.ant-select-item-option-content > div > span', 'Connection B');
  assert.equal(harness.samples, 9);
  assert.deepEqual(harness.events.map(event => event.type), ['mousePressed', 'mouseReleased']);
  assert.ok(harness.events.every(event => event.x === 61 && event.y === 30 && event.button === 'left' && event.clickCount === 1));
  assert.equal(harness.phases[harness.phases.length - 1]?.category, 'connection-option');
});

test('SSH click ignores infinite spinners and never queries unrelated document animations', async (context) => {
  const harness = createHarness(context);
  await harness.actions.click('.ant-modal .ant-select');
  assert.equal(harness.samples, 3);
  assert.deepEqual(harness.events.map(event => event.type), ['mousePressed', 'mouseReleased']);
});

test('SSH selection requires a B request after exactly one click per control', async (context) => {
  const harness = createHarness(context);
  await harness.actions.chooseB();
  assert.deepEqual(harness.events.map(event => event.type), ['mousePressed', 'mouseReleased', 'mousePressed', 'mouseReleased']);
  assert.equal(harness.drains, 1);
  assert.deepEqual(harness.phases[harness.phases.length - 1], { phase: 'wait-B-request', category: 'connection-option' });
});

test('SSH selection with no B request still fails and never retries its click', async (context) => {
  const harness = createHarness(context, { emitB: false });
  await assert.rejects(harness.actions.chooseB(), /Connection B request after one click deadline/);
  assert.deepEqual(harness.events.map(event => event.type), ['mousePressed', 'mouseReleased', 'mousePressed', 'mouseReleased']);
  assert.equal(harness.drains, 1);
  assert.deepEqual(harness.phases[harness.phases.length - 1], { phase: 'wait-B-request', category: 'connection-option' });
});
