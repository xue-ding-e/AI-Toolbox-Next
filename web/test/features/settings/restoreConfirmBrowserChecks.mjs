import assert from 'node:assert/strict';

export async function verifyRestoreConfirm({ send, evaluate, navigate, waitFor, delay, record, screenshot, call }) {
  const visibleCount = 'backupFixture.visibleModalCount()';
  const checkboxSelector = '.ant-modal-confirm input[type="checkbox"]';
  const passwordSelector = '.ant-modal-confirm input[type="password"]';
  const snapshot = () => evaluate('restoreConfirmFixture.snapshot()');
  const confirm = () => call('click', 'common.confirm');
  const cancel = () => call('click', 'common.cancel');
  const closed = () => waitFor(visibleCount + ' === 0');
  const request = (skipCliCustomRoots, restorePassword = null, settled = false) => ({
    filename: 'fixture-confirm-backup.zip.enc', sha: 'fixture-confirm-original-sha',
    skipCliCustomRoots, restorePassword, settled,
  });
  const assertNoRestore = async () => {
    assert.deepEqual(await snapshot(), {
      calls: [], executions: 0, executeThenReads: 0, loading: [], prompts: [],
      successes: 0, errors: [], confirmReturnedUndefined: true,
    }, 'opening/toggling/cancelling a confirmation never executes a restore');
  };
  const open = async () => {
    await evaluate('document.querySelector("[data-testid=open-restore-confirm]").click()');
    await waitFor('!!document.querySelector(' + JSON.stringify(checkboxSelector) + ')');
    await waitFor(visibleCount + ' === 1');
    assert.equal(await evaluate('document.querySelector(' + JSON.stringify(checkboxSelector) + ').checked'), false,
      'every confirmation starts unchecked');
    await assertNoRestore();
  };
  const begin = async (theme = 'light', language = 'en-US') => {
    await navigate({ restoreConfirm: '1', theme, language });
    await open();
  };
  const toggle = async expected => {
    // A real checkbox click exercises AntD and React onChange; never mutate
    // checked/holder directly, which would bypass the production option adapter.
    await evaluate('document.querySelector(' + JSON.stringify(checkboxSelector) + ').click()');
    await waitFor('document.querySelector(' + JSON.stringify(checkboxSelector) + ').checked === ' + expected);
    await assertNoRestore();
  };
  const assertPending = async expected => {
    await closed();
    const result = await snapshot();
    assert.deepEqual(result.calls, [request(expected)]);
    assert.equal(result.executions, 1);
    assert.equal(result.executeThenReads, 0, 'void onOk never exposes the executor thenable to AntD');
    assert.deepEqual(result.loading, [true], 'dialog closes while the service is still pending');
    assert.deepEqual(result.prompts, []);
    assert.equal(result.successes, 0);
    assert.deepEqual(result.errors, []);
  };
  const finish = async expected => {
    await evaluate('restoreConfirmFixture.settle(0)');
    await waitFor('restoreConfirmFixture.snapshot().successes === 1');
    const result = await snapshot();
    assert.deepEqual(result.calls, [request(expected, null, true)]);
    assert.deepEqual(result.loading, [true, false]);
    assert.equal(result.executions, 1, 'later settlement cannot repeat the confirmation');
    assert.equal(result.executeThenReads, 0);
    assert.equal(await evaluate(visibleCount), 0);
  };
  const inputPassword = async value => {
    await evaluate('document.querySelector(' + JSON.stringify(passwordSelector) + ').focus()');
    await send('Input.insertText', { text: value });
    await waitFor('document.querySelector(' + JSON.stringify(passwordSelector) + ').value === ' + JSON.stringify(value));
  };
  const requirePassword = async (index, errorType) => {
    await evaluate('restoreConfirmFixture.settle(' + index + ', ' + JSON.stringify(errorType) + ')');
    await waitFor('!!document.querySelector(' + JSON.stringify(passwordSelector) + ')');
    await waitFor(visibleCount + ' === 1');
    await waitFor('restoreConfirmFixture.snapshot().loading.at(-1) === false');
    assert.equal(await evaluate('document.querySelector(' + JSON.stringify(passwordSelector) + ').value'), '',
      'each password prompt starts with fresh input');
    assert.equal(await evaluate('!!document.querySelector(' + JSON.stringify(checkboxSelector) + ')'), false,
      'password retry does not recreate the restore confirmation/options');
  };
  let completed = 0;

  for (const checked of [false, true]) {
    await begin();
    if (checked) await toggle(true);
    await cancel();
    await closed();
    await assertNoRestore();
    record('restore confirm: cancel with option ' + checked + ' invokes zero services', true);
    completed++;
  }

  for (const mode of ['default', 'checked', 'unchecked']) {
    await begin(mode === 'checked' ? 'dark' : 'light', mode === 'checked' ? 'zh-CN' : 'en-US');
    const copy = await evaluate(`(() => {
      const modal = document.querySelector('.ant-modal-confirm');
      return {
        title: modal.querySelector('.ant-modal-confirm-title').textContent,
        expectedTitle: restoreConfirmFixture.text('settings.backupSettings.confirmRestore'),
        content: modal.querySelector('.ant-modal-confirm-content').textContent,
        description: restoreConfirmFixture.description,
        option: restoreConfirmFixture.text('settings.backupSettings.skipCliCustomRoots'),
        help: restoreConfirmFixture.text('settings.backupSettings.skipCliCustomRootsDesc'),
      };
    })()`);
    assert.equal(copy.title, copy.expectedTitle);
    for (const text of [copy.description, copy.option, copy.help]) assert.ok(copy.content.includes(text));
    if (mode !== 'default') await toggle(true);
    if (mode === 'unchecked') await toggle(false);
    if (mode === 'checked') await screenshot('restore-confirm-checked-dark');
    await evaluate('window.restoreConfirmButton = document.querySelector(".ant-modal-confirm .ant-btn-primary")');
    await confirm();
    const expected = mode === 'checked';
    await assertPending(expected);
    assert.equal(await evaluate('window.restoreConfirmButton.classList.contains("ant-btn-loading")'), false,
      'confirmation never enters promise loading while the service remains pending');
    await finish(expected);
    record('restore confirm: ' + mode + ' option reaches service; onOk stays fire-and-forget', true);
    completed++;
  }

  await begin();
  await toggle(true);
  await cancel();
  await closed();
  await open();
  await confirm();
  await assertPending(false);
  await finish(false);
  record('restore confirm: cancel/reopen allocates a new unchecked holder', true);
  completed++;

  for (const mode of ['default', 'checked', 'unchecked']) {
    await begin();
    if (mode !== 'default') await toggle(true);
    if (mode === 'unchecked') await toggle(false);
    // Replacement while the dialog is open cannot change its selected object.
    await evaluate('restoreConfirmFixture.state.selection = { filename: "replacement-before-confirm.zip.enc", sha: "replacement-before-confirm-sha" }');
    await confirm();
    const expected = mode === 'checked';
    await assertPending(expected);
    await requirePassword(0, 'passwordRequired');
    assert.deepEqual((await snapshot()).calls, [request(expected, null, true)], 'opening password prompt never retries');
    await evaluate('restoreConfirmFixture.state.selection = { filename: "replacement-before-retry.zip.enc", sha: "replacement-before-retry-sha" }');
    await inputPassword('fixture-confirm-wrong');
    await confirm();
    await closed();
    await waitFor('restoreConfirmFixture.snapshot().calls.length === 2');
    await requirePassword(1, 'passwordWrong');
    assert.equal(await evaluate('document.querySelector(".ant-modal-confirm .ant-typography-danger").textContent === restoreConfirmFixture.text("settings.backupSettings.encryption.errors.passwordWrong")'), true);
    await inputPassword('fixture-confirm-correct');
    await confirm();
    await closed();
    await waitFor('restoreConfirmFixture.snapshot().calls.length === 3');
    await evaluate('restoreConfirmFixture.settle(2)');
    await waitFor('restoreConfirmFixture.snapshot().successes === 1');
    const result = await snapshot();
    assert.deepEqual(result.calls, [null, 'fixture-confirm-wrong', 'fixture-confirm-correct'].map(password =>
      request(expected, password, true)), 'every retry retains the selected filename/SHA and final checkbox option');
    assert.deepEqual(result.loading, [true, false, true, false, true, false]);
    assert.deepEqual(result.prompts, [false, true]);
    assert.equal(result.executions, 1, 'retry recurses through the flow without reopening/reexecuting confirmation');
    assert.equal(result.executeThenReads, 0);
    assert.deepEqual(result.errors, []);
    record('restore confirm: ' + mode + ' survives required/wrong-password retries with filename/SHA', true);
    completed++;
  }

  await begin();
  await toggle(true);
  await confirm();
  await assertPending(true);
  await requirePassword(0, 'passwordRequired');
  await inputPassword('fixture-confirm-cancelled');
  await cancel();
  await closed();
  await delay(100);
  assert.deepEqual(await snapshot(), {
    calls: [request(true, null, true)], executions: 1, executeThenReads: 0,
    loading: [true, false], prompts: [false], successes: 0, errors: [], confirmReturnedUndefined: true,
  }, 'password cancellation makes no further service call after the initial password-required response');
  record('restore confirm: cancelling password prompt makes no retry', true);
  completed++;

  assert.equal(completed, 10, 'all restore confirmation DOM scenarios must execute');
  console.log('Executed ' + completed + ' restore confirmation DOM scenarios');
}
