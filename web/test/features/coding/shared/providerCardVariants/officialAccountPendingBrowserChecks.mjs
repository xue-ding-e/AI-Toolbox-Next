import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';

// Independent expectations for all five rendered row/action pairs.
const CASES = {
  'legacy-null': [false, false, false, false, false],
  'legacy-save': [false, true, false, false, false],
  'legacy-apply': [false, false, false, true, false],
  'legacy-delete': [false, false, false, false, true],
  'array-different-rows': [false, true, true, false, false],
  'array-same-row': [true, true, false, false, false],
  'array-save-only': [false, true, false, false, false],
  'array-empty': [false, false, false, false, false],
};
const TARGETS = [
  { accountId: 'local', action: 'refresh' }, { accountId: 'local', action: 'save' },
  { accountId: 'saved', action: 'refresh' }, { accountId: 'saved', action: 'apply' },
  { accountId: 'saved', action: 'delete' },
];

export async function verifyOfficialAccountPending({ send, evaluate, baseUrl, artifactRoot, exceptions }) {
  const checks = [];
  const completed = [];
  const observations = [];
  const check = (name, actual, expected) => {
    assert.deepEqual(actual, expected, name);
    checks.push(name);
    console.log('PASS ' + name);
  };
  const fixture = expression => evaluate('officialAccountPendingFixture.' + expression);
  const waitFor = async expression => {
    for (let attempt = 0; attempt < 150; attempt++) {
      if (await evaluate(expression)) return;
      await new Promise(resolve => setTimeout(resolve, 50));
    }
    throw new Error('Timed out waiting for: ' + expression);
  };
  const runId = randomUUID();
  await send('Page.navigate', { url: baseUrl + '/?runId=' + runId });
  await waitFor('window.officialAccountPendingFixture?.state.runId === ' + JSON.stringify(runId)
    + ' && officialAccountPendingFixture.state.activeCase === "legacy-null"');
  try {
    for (const [name, loading] of Object.entries(CASES)) {
      const start = checks.length;
      await fixture('selectCase(' + JSON.stringify(name) + ')');
      await waitFor('officialAccountPendingFixture.state.activeCase === ' + JSON.stringify(name));
      await fixture('resetCallbacks()');
      const snapshot = await fixture('snapshot()');
      observations.push({ name, snapshot });
      check(name + ': exact row/action loading matrix', snapshot,
        TARGETS.map((target, index) => ({ ...target, loading: loading[index] })));
      const pending = TARGETS.filter((_, index) => loading[index]);
      const idle = TARGETS.filter((_, index) => !loading[index]);
      await fixture('repeatClicks(' + JSON.stringify(pending) + ')');
      check(name + ': repeated pending clicks invoke no callback', await fixture('state.callbacks'), []);
      await fixture('repeatClicks(' + JSON.stringify(idle) + ')');
      check(name + ': idle actions still deliver exact identities', await fixture('state.callbacks'),
        idle.flatMap(target => [target, target]));
      check(name + ': no backend invocation', await fixture('state.unexpectedIPC'), []);
      check(name + ': no unhandled browser exception', exceptions.length, 0);
      assert.equal(checks.length - start, 5, 'Every pending state must run five assertions');
      completed.push(name);
    }
    assert.deepEqual(completed, Object.keys(CASES));
    assert.equal(completed.length, 8, 'Fixed scenario count prevents zero-test success');
    assert.equal(checks.length, 40, 'Fixed assertion count prevents partial success');
  } finally {
    await writeFile(path.join(artifactRoot, 'pending-states.json'), JSON.stringify({
      completed, checks, observations, browserExceptions: exceptions,
    }, null, 2));
  }
  console.log(`Passed ${completed.length} shared account-pending scenarios (${checks.length} assertions).`);
  return checks;
}
