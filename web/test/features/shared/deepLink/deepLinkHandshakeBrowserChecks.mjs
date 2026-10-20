import assert from 'node:assert/strict';
export async function verifyDeepLinkHandshake({ send, evaluate, baseUrl }) {
  const checks = [];
  const failures = [];
  let caseId = 0;
  const call = (method, ...args) => evaluate('deepLinkHandshakeFixture.' + method + '(' + args.map(JSON.stringify).join(',') + ')');
  const wait = async predicate => {
    for (let i = 0; i < 100; i++) {
      if (await evaluate('window.deepLinkHandshakeFixture && (() => {const s=deepLinkHandshakeFixture.snapshot(); return ' + predicate + ';})()')) return;
      await new Promise(resolve => setTimeout(resolve, 20));
    }
    throw new Error('Handshake fixture timeout: ' + predicate);
  };
  const run = async (name, verify) => {
    try {
      await send('Page.navigate', { url: baseUrl + '/?case=' + caseId });
      await wait('s.readyCalls===1 && s.request==="none" && s.caseId===' + JSON.stringify(String(caseId)));
      assert.deepEqual((await call('snapshot')).subscriptions, ['deep-link-error', 'deep-link-import']);
      await verify();
      checks.push(name);
      console.log('PASS: ' + name);
    } catch (error) {
      failures.push({ name, error: String(error) });
      console.error('FAIL: ' + name + ': ' + error);
    } finally { caseId++; }
  };
  const expect = async name => { await wait('s.request===' + JSON.stringify(name)); };
  const settle = async () => {
    const before = (await call('snapshot')).barrier;
    await call('settle');
    const after = await call('snapshot');
    if (after.request !== null) assert.equal(after.barrier, before + 1, 'React commit barrier completed');
  };
  await run('cold pending renders', async () => {
    await call('resolve', 'cold'); await settle(); await expect('cold');
  });
  await run('live wins over delayed cold', async () => {
    await call('emit', 'live'); await expect('live'); await call('resolve', 'older'); await settle();
    assert.equal((await call('snapshot')).request, 'live');
  });
  await run('dismiss not reopened', async () => {
    await call('emit', 'live'); await expect('live');
    await evaluate('document.querySelector("[data-testid=dismiss]").click()'); await expect('none');
    await call('resolve', 'older'); await settle(); assert.equal((await call('snapshot')).request, 'none');
  });
  await run('latest live wins', async () => {
    await call('emit', 'one'); await expect('one'); await call('emit', 'two'); await expect('two');
    await call('resolve', 'older'); await settle(); assert.equal((await call('snapshot')).request, 'two');
  });
  await run('rejection preserves live', async () => {
    await call('emit', 'live'); await expect('live'); await call('reject'); await settle();
    assert.equal((await call('snapshot')).request, 'live');
  });
  await run('unmounted response ignored', async () => {
    await call('unmount'); await wait('s.request===null && s.subscriptions.length===0');
    await call('resolve', 'late'); await settle(); assert.equal((await call('snapshot')).request, null);
  });
  console.log(JSON.stringify({ passed: checks.length, failed: failures.length, checks, failures }));
  assert.equal(failures.length, 0, 'Deep-link handshake cases failed');
  return checks;
}
