import assert from 'node:assert/strict';

export async function verifyRestoreSuccess({ send, evaluate, navigate, waitFor, delay, record, screenshot }) {
  const visibleCount = 'backupFixture.visibleModalCount()';
  const modalSelector = '.ant-modal-confirm-info';
  const buttonSelector = modalSelector + ' .ant-btn-primary';
  const snapshot = () => evaluate('restoreSuccessFixture.snapshot()');
  const clickAt = async ({ x, y }) => {
    await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
    await send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', buttons: 1, clickCount: 1 });
    await send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', buttons: 0, clickCount: 1 });
  };
  const key = async (name, code, text) => {
    // CDP invokes Chromium's default keyboard action; synthetic KeyboardEvent
    // would not test activation of the focused confirmation button.
    await send('Input.dispatchKeyEvent', { type: 'keyDown', key: name, code: name,
      windowsVirtualKeyCode: code, nativeVirtualKeyCode: code, ...(text ? { text } : {}) });
    await send('Input.dispatchKeyEvent', { type: 'keyUp', key: name, code: name,
      windowsVirtualKeyCode: code, nativeVirtualKeyCode: code });
  };
  const begin = async (language = 'en-US') => {
    await navigate({ restoreSuccess: '1', theme: 'light', language });
    await evaluate('document.querySelector("[data-testid=start-success-restore]").click()');
    await waitFor('restoreSuccessFixture.snapshot().restoreCalls === 1');
  };
  const beforeConfirm = async () => {
    assert.deepEqual(await snapshot(), {
      restoreCalls: 1, loading: [true, false], successes: 1, warnings: 1,
      prompts: 0, errors: [], restartCalls: 0, restartSettled: false, restartThenReads: 0,
    }, 'completed restore releases flow loading without restarting before confirmation');
    assert.equal(await evaluate(visibleCount), 1, 'the success dialog remains visible');
  };
  const finishRestore = async () => {
    await evaluate('restoreSuccessFixture.finishRestore()');
    await waitFor('!!document.querySelector(' + JSON.stringify(buttonSelector) + ')');
    await waitFor(visibleCount + ' === 1');
    await delay(350);
    await beforeConfirm();
  };
  const openSuccess = async (language = 'en-US') => {
    await begin(language);
    await finishRestore();
  };
  const focusConfirm = () => evaluate('document.querySelector(' + JSON.stringify(buttonSelector) + ').focus()');
  const assertConfirmedOnce = async () => {
    await waitFor(visibleCount + ' === 0');
    const result = await snapshot();
    assert.equal(result.restartCalls, 1, 'one confirmation invokes the restart boundary once');
    assert.equal(result.restartSettled, false, 'dialog closes while the synthetic restart remains pending');
    assert.equal(result.restartThenReads, 0, 'void onOk does not expose the callback thenable to AntD');
    assert.deepEqual(result.loading, [true, false], 'restart does not reenter restore loading');
    assert.equal(result.restoreCalls, 1, 'confirmation never repeats the restore');
    assert.deepEqual(result.errors, []);
    await evaluate('restoreSuccessFixture.finishRestart()');
    await delay(100);
    assert.deepEqual(await snapshot(), { ...result, restartSettled: true }, 'later settlement has no additional flow effects');
    assert.equal(await evaluate(visibleCount), 0, 'later settlement never reopens the dialog');
  };
  let completed = 0;

  await begin();
  assert.deepEqual(await snapshot(), {
    restoreCalls: 1, loading: [true], successes: 0, warnings: 0,
    prompts: 0, errors: [], restartCalls: 0, restartSettled: false, restartThenReads: 0,
  }, 'pending restore has neither a success dialog nor an early restart');
  assert.equal(await evaluate(visibleCount), 0);
  await finishRestore();
  const dialog = await evaluate(`(() => {
    const modal = document.querySelector(${JSON.stringify(modalSelector)});
    const button = modal.querySelector('.ant-btn-primary');
    return {
      title: modal.querySelector('.ant-modal-confirm-title').textContent,
      expectedTitle: restoreSuccessFixture.text('settings.backupSettings.restoreSuccess'),
      content: modal.querySelector('.ant-modal-confirm-content').textContent,
      expectedContent: restoreSuccessFixture.content,
      buttonLabel: button.textContent.replace(/\\s/g, ''),
      expectedLabel: restoreSuccessFixture.text('common.restart').replace(/\\s/g, ''),
      buttons: modal.querySelectorAll('button').length,
      closeButtons: modal.querySelectorAll('.ant-modal-close').length,
      loading: button.classList.contains('ant-btn-loading'), disabled: button.disabled,
    };
  })()`);
  assert.equal(dialog.title, dialog.expectedTitle);
  assert.equal(dialog.content, dialog.expectedContent);
  assert.equal(dialog.buttonLabel, dialog.expectedLabel);
  assert.equal(dialog.buttons, 1, 'only the restart confirmation is offered; no cancel button');
  assert.equal(dialog.closeButtons, 0, 'there is no close affordance');
  assert.equal(dialog.loading, false, 'restart is initially actionable without promise loading');
  assert.equal(dialog.disabled, false);
  await screenshot('restore-success-awaiting-confirmation');
  record('restore success: pending flow and completed locked dialog preserve loading boundaries', true);
  completed++;

  await openSuccess();
  await focusConfirm();
  await evaluate(`(() => {
    window.restoreSuccessEscapeEvents = [];
    document.querySelector(${JSON.stringify(modalSelector)}).closest('.ant-modal-wrap')
      .addEventListener('keydown', event => {
        if (event.key === 'Escape') window.restoreSuccessEscapeEvents.push(event.isTrusted);
      }, { once: true });
  })()`);
  await key('Escape', 27);
  await delay(400);
  assert.deepEqual(await evaluate('window.restoreSuccessEscapeEvents'), [true], 'real Escape reaches the modal');
  await beforeConfirm();
  record('restore success: Escape cannot close or restart', true);
  completed++;

  await openSuccess();
  const maskPoint = await evaluate(`(() => {
    const modal = document.querySelector(${JSON.stringify(modalSelector)});
    const wrap = modal.closest('.ant-modal-wrap');
    const mask = wrap.parentElement.querySelector('.ant-modal-mask');
    if (!mask || !mask.getClientRects().length) throw new Error('Missing visible success mask');
    const bounds = wrap.getBoundingClientRect();
    const x = Math.max(1, bounds.left + 4), y = Math.max(1, bounds.top + 4);
    if (document.elementFromPoint(x, y) !== wrap) throw new Error('Mask click would miss the modal wrapper');
    window.restoreSuccessMaskEvents = [];
    wrap.addEventListener('click', event => {
      if (event.target === wrap) window.restoreSuccessMaskEvents.push(event.isTrusted);
    }, { once: true });
    return { x, y };
  })()`);
  await clickAt(maskPoint);
  await delay(400);
  assert.deepEqual(await evaluate('window.restoreSuccessMaskEvents'), [true], 'real pointer click reaches the mask area');
  await beforeConfirm();
  record('restore success: mask click cannot close or restart', true);
  completed++;

  await openSuccess();
  const buttonPoint = await evaluate(`(() => {
    const button = document.querySelector(${JSON.stringify(buttonSelector)});
    window.restoreSuccessConfirmedButton = button;
    const bounds = button.getBoundingClientRect();
    return { x: bounds.left + bounds.width / 2, y: bounds.top + bounds.height / 2 };
  })()`);
  await clickAt(buttonPoint);
  await assertConfirmedOnce();
  assert.equal(await evaluate('window.restoreSuccessConfirmedButton.classList.contains("ant-btn-loading")'), false,
    'void restart callback does not put the confirmation button in promise loading');
  record('restore success: mouse confirmation invokes restart once without awaiting its thenable', true);
  completed++;

  await openSuccess('zh-CN');
  await focusConfirm();
  await key('Enter', 13, '\r');
  await assertConfirmedOnce();
  record('restore success: Enter on the focused restart button confirms once', true);
  completed++;

  await openSuccess();
  // Deliver two actual DOM click events in one turn, before the static Modal's
  // scheduled close unmounts the button. This characterizes AntD's existing
  // duplicate-click behavior, not page-level restore concurrency or a new lock.
  const repeated = await evaluate(`(() => {
    const button = document.querySelector(${JSON.stringify(buttonSelector)});
    let events = 0;
    button.addEventListener('click', () => { events++; });
    button.click();
    const afterFirst = restoreSuccessFixture.snapshot().restartCalls;
    const connectedBeforeSecond = button.isConnected;
    button.click();
    return { events, afterFirst, connectedBeforeSecond,
      afterSecond: restoreSuccessFixture.snapshot().restartCalls,
      loading: button.classList.contains('ant-btn-loading') };
  })()`);
  assert.deepEqual(repeated, { events: 2, afterFirst: 1, connectedBeforeSecond: true, afterSecond: 1, loading: false },
    'AntD handles two same-turn click events with one fire-and-forget callback');
  await assertConfirmedOnce();
  record('restore success: same-turn repeated confirmation clicks invoke restart once', true);
  completed++;

  assert.equal(completed, 6, 'all restore success DOM scenarios must execute');
  console.log('Executed ' + completed + ' restore success DOM scenarios');
}
