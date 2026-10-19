import assert from 'node:assert/strict';

export async function verifyRestorePassword({ send, evaluate, navigate, waitFor, delay, record, screenshot, call }) {
  const passwordSelector = '.ant-modal-confirm input[type="password"]';
  const visibleCount = 'backupFixture.visibleModalCount()';
  const snapshot = () => evaluate('restorePasswordFixture.snapshot()');
  const key = async (name, code, text) => {
    await send('Input.dispatchKeyEvent', { type: 'keyDown', key: name, code: name,
      windowsVirtualKeyCode: code, nativeVirtualKeyCode: code, ...(text ? { text } : {}) });
    await send('Input.dispatchKeyEvent', { type: 'keyUp', key: name, code: name,
      windowsVirtualKeyCode: code, nativeVirtualKeyCode: code });
    await delay(100);
  };
  const begin = async (button = 'open-password', theme = 'light', language = 'en-US') => {
    await navigate({ restorePassword: '1', theme, language });
    await evaluate('document.querySelector(' + JSON.stringify('[data-testid="' + button + '"]') + ').click()');
    await waitFor('!!document.querySelector(' + JSON.stringify(passwordSelector) + ')');
    await waitFor(visibleCount + ' === 1');
  };
  const input = async value => {
    // CDP inserts real text into the focused input, exercising React onChange.
    await evaluate('document.querySelector(' + JSON.stringify(passwordSelector) + ').focus()');
    await send('Input.insertText', { text: value });
    await waitFor('document.querySelector(' + JSON.stringify(passwordSelector) + ').value === ' + JSON.stringify(value));
  };
  const confirm = () => call('click', 'common.confirm');
  const cancel = () => call('click', 'common.cancel');
  const closed = () => waitFor(visibleCount + ' === 0');
  const assertSubmittedOnce = async password => {
    await closed();
    const result = await snapshot();
    assert.deepEqual(result.submissions, [password]);
    assert.equal(result.callbackSettled, false, 'dialog closes without awaiting callback thenable');
    assert.equal(result.promptReturnedUndefined, true, 'prompt does not expose Modal control handle');
    await evaluate('restorePasswordFixture.finishCallback()');
    assert.deepEqual((await snapshot()).submissions, [password], 'callback settlement does not resubmit');
  };
  let completed = 0;

  await begin();
  // AntD 6.4.3 ActionButton re-rejects an onOk rejection after clearing its
  // loading state. Characterize that existing empty-input negative path only:
  // https://github.com/ant-design/ant-design/blob/6.4.3/components/_util/ActionButton.tsx
  // The shared runner must still fail on every unexpected browser exception.
  await evaluate(`(() => {
    const input = document.querySelector(${JSON.stringify(passwordSelector)});
    const button = input?.closest('.ant-modal-confirm').querySelector('.ant-btn-primary');
    if (!input || input.value !== '' || !button) throw new Error('Expected fresh empty password dialog');
    const expectation = { count: 0, armed: false };
    const arm = () => { expectation.armed = true; };
    const listener = event => {
      if (!expectation.armed || event.reason !== undefined || expectation.count !== 0 || !input.isConnected
        || input.value !== '' || restorePasswordFixture.state.submissions.length !== 0) return;
      expectation.count++;
      event.preventDefault();
      window.removeEventListener('unhandledrejection', listener);
    };
    expectation.cleanup = () => {
      window.removeEventListener('unhandledrejection', listener);
      button.removeEventListener('click', arm);
    };
    window.restorePasswordEmptyRejection = expectation;
    window.addEventListener('unhandledrejection', listener);
    button.addEventListener('click', arm, { once: true });
  })()`);
  try {
    await confirm();
    await waitFor('window.restorePasswordEmptyRejection.count === 1');
    assert.equal(await evaluate('window.restorePasswordEmptyRejection.count'), 1,
      'empty validation produces exactly one expected undefined rejection');
  } finally {
    await evaluate('window.restorePasswordEmptyRejection.cleanup()');
  }
  await waitFor('!!document.querySelector(".ant-message-warning")');
  assert.equal(await evaluate(visibleCount), 1, 'empty password rejection keeps dialog open');
  assert.deepEqual((await snapshot()).submissions, [], 'empty input never invokes submit');
  assert.equal(await evaluate('document.querySelector(".ant-message-warning").textContent.includes(restorePasswordFixture.text("settings.backupSettings.encryption.passwordRequiredPrompt"))'), true);
  assert.equal(await evaluate('!!document.querySelector(".ant-btn-loading")'), false, 'empty rejection clears button loading');
  await input('fixture-after-empty');
  await confirm();
  await assertSubmittedOnce('fixture-after-empty');
  record('restore password: empty warning keeps dialog open, then corrected input submits once', true);
  completed++;

  await begin();
  await input('fixture-cancelled-password');
  await cancel();
  await closed();
  assert.deepEqual((await snapshot()).submissions, []);
  record('restore password: cancel invokes no submit', true);
  completed++;

  await begin();
  await input('fixture-single-password');
  await confirm();
  await assertSubmittedOnce('fixture-single-password');
  record('restore password: nonempty callback remains fire-and-forget', true);
  completed++;

  await begin('open-wrong-password', 'dark', 'zh-CN');
  assert.equal(await evaluate('document.querySelector(".ant-modal-confirm .ant-typography-danger").textContent === restorePasswordFixture.text("settings.backupSettings.encryption.errors.passwordWrong")'), true);
  assert.equal(await evaluate('document.querySelector(' + JSON.stringify(passwordSelector) + ').value'), '', 'retry never retains previous secret');
  await screenshot('restore-password-wrong-dark');
  await cancel();
  await closed();
  assert.deepEqual((await snapshot()).submissions, []);
  record('restore password: wrong-attempt hint with fresh input', true);
  completed++;

  await begin();
  assert.equal(await evaluate('!!document.querySelector(".ant-modal-confirm .ant-typography-danger")'), false, 'first prompt has no wrong-attempt hint');
  await input('fixture-enter-input');
  await key('Enter', 13, '\r');
  assert.equal(await evaluate(visibleCount), 1, 'plain input Enter has no custom submit handler');
  assert.deepEqual((await snapshot()).submissions, []);
  await cancel();
  await closed();
  record('restore password: Enter in password input does not submit', true);
  completed++;

  await begin();
  await input('fixture-enter-button');
  await evaluate('document.querySelector(".ant-modal-confirm .ant-btn-primary").focus()');
  await key('Enter', 13, '\r');
  await assertSubmittedOnce('fixture-enter-button');
  record('restore password: Enter on focused confirm submits once', true);
  completed++;

  await begin();
  await input('fixture-escape-input');
  await key('Escape', 27);
  await closed();
  assert.deepEqual((await snapshot()).submissions, []);
  record('restore password: Escape cancels without submit', true);
  completed++;

  // Do not silently introduce trim semantics while moving existing code.
  await begin();
  await input('   ');
  await confirm();
  await assertSubmittedOnce('   ');
  record('restore password: nonempty whitespace is preserved', true);
  completed++;

  await navigate({ restorePassword: '1', theme: 'light', language: 'en-US' });
  await evaluate('document.querySelector("[data-testid=start-password-restore]").click()');
  await waitFor('restorePasswordFixture.state.calls.length === 1');
  await evaluate('restorePasswordFixture.settle(0, "passwordRequired")');
  await waitFor('!!document.querySelector(' + JSON.stringify(passwordSelector) + ')');
  await waitFor('restorePasswordFixture.state.loading.at(-1) === false');
  await evaluate('restorePasswordFixture.state.selection = { filename: "different-selection.zip.enc", sha: "different-selection-sha" }');
  await input('fixture-wrong-attempt');
  await confirm();
  await closed();
  await waitFor('restorePasswordFixture.state.calls.length === 2');
  assert.equal((await snapshot()).loading.at(-1), true, 'retry service stays pending after dialog closes');
  await evaluate('restorePasswordFixture.settle(1, "passwordWrong")');
  await waitFor('!!document.querySelector(".ant-modal-confirm .ant-typography-danger")');
  assert.equal(await evaluate('document.querySelector(' + JSON.stringify(passwordSelector) + ').value'), '');
  await input('fixture-final-password');
  await confirm();
  await closed();
  await waitFor('restorePasswordFixture.state.calls.length === 3');
  await evaluate('restorePasswordFixture.settle(2)');
  await waitFor('restorePasswordFixture.state.successes === 1');
  const result = await snapshot();
  assert.deepEqual(result.calls, [null, 'fixture-wrong-attempt', 'fixture-final-password'].map(password => ({
    filename: 'fixture-backup.zip.enc', sha: 'fixture-original-sha', password,
  })), 'production recursive flow preserves the same synthetic selected-file closure across retries');
  assert.deepEqual(result.loading, [true, false, true, false, true, false]);
  assert.deepEqual(result.errors, []);
  record('restore password: recursive retries preserve original filename/SHA closure', true);
  completed++;

  assert.equal(completed, 9, 'all restore password DOM scenarios must execute');
  console.log('Executed ' + completed + ' restore password DOM scenarios');
}
