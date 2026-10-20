import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';

export async function verifyGeminiAccountPending({ send, evaluate, baseUrl, artifactRoot, exceptions }) {
  const checks = [];
  const check = (name, actual, expected = true) => {
    assert.deepEqual(actual, expected, name);
    checks.push(name);
    console.log('PASS ' + name);
  };
  const waitFor = async expression => {
    for (let attempt = 0; attempt < 150; attempt++) {
      if (await evaluate(expression)) return;
      await new Promise(resolve => setTimeout(resolve, 50));
    }
    throw new Error('Timed out waiting for: ' + expression);
  };
  const runId = randomUUID();
  let before;
  let after;
  let callbacks;
  let passed = false;
  await send('Page.navigate', { url: baseUrl + '/?runId=' + runId });
  await waitFor('window.geminiAccountPendingFixture?.state.runId === ' + JSON.stringify(runId)
    + ' && Boolean(document.querySelector("[data-provider-id=fixture-gemini-pending]"))');
  try {
    await evaluate('geminiAccountPendingFixture.expandAccounts()');
    await waitFor('document.body.textContent.includes("pending-saved@example.test") && document.body.textContent.includes("pending-local@example.test")');
    before = await evaluate('geminiAccountPendingFixture.snapshot()');
    // Collect both behaviors before assertions so a loading failure still
    // records whether repeat clicks reached the synthetic callback.
    await evaluate('geminiAccountPendingFixture.repeatPendingClicks()');
    await evaluate('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
    after = await evaluate('geminiAccountPendingFixture.snapshot()');
    callbacks = await evaluate('geminiAccountPendingFixture.state.callbacks');
    check('the actual refresh and local-save actions are rendered',
      [before.refresh.label, before.save.label], ['刷新', '保存当前登录']);
    check('saved-account refresh stays loading with concurrent local save',
      before.refresh.loading && after.refresh.loading);
    check('local save stays loading with concurrent saved-account refresh',
      before.save.loading && after.save.loading);
    check('repeated clicks on both pending actions invoke no callback', callbacks, []);
    check('the fixture makes no backend calls', await evaluate('geminiAccountPendingFixture.state.unexpectedIPC'), []);
    check('no unhandled browser exception', exceptions.length, 0);
    assert.equal(checks.length, 6, 'The one dual-pending scenario must execute all six assertions');
    passed = true;
  } finally {
    // Diagnostics never consume or classify browser exceptions as expected.
    await writeFile(path.join(artifactRoot, 'dual-pending.json'), JSON.stringify({
      scenario: 'dual-pending', passed, checks, before, after, callbacks, browserExceptions: exceptions,
    }, null, 2));
  }
  assert.equal(passed, true);
  console.log(`Passed 1 Gemini dual-pending scenario (${checks.length} assertions).`);
  return checks;
}
