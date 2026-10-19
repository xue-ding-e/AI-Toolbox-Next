import React from 'react';
import { Button, Modal } from 'antd';
import { executeRestoreFlow } from '@/features/settings/utils/restoreFlow';
import i18n from '@/i18n';

// Exercise the production flow's exact dialog configuration and onOk callback.
// Only service, content and feedback boundaries are synthetic; no native restore
// or restart API is imported, and no dialog options are recreated in this fixture.
const state = {
  restoreCalls: 0,
  loading: [],
  successes: 0,
  warnings: 0,
  prompts: 0,
  errors: [],
  restartCalls: 0,
  restartSettled: false,
  restartThenReads: 0,
};
const t = key => i18n.t(key);
const content = 'Synthetic restore result: no files were changed.';
const deferred = () => {
  let resolve;
  const promise = new Promise(yes => { resolve = yes; });
  return { promise, resolve };
};
const restore = deferred();
const restart = deferred();
let restoreSettled = false;
const restartThenable = {
  get then() {
    state.restartThenReads++;
    return restart.promise.then.bind(restart.promise);
  },
};

function startRestore() {
  void executeRestoreFlow(() => {
    state.restoreCalls++;
    return restore.promise;
  }, {
    setLoading: value => state.loading.push(value),
    translate: t,
    buildSuccessContent: result => result.content,
    showSuccess: dialog => {
      state.successes++;
      // This is the page's Modal.info adapter, receiving the original object.
      // Do not wrap onOk or copy its locking flags into a test-only dialog.
      Modal.info(dialog);
    },
    showWarnings: () => { state.warnings++; },
    promptPassword: () => { state.prompts++; },
    restart: () => {
      state.restartCalls++;
      // The production onOk is void: even a pending callback thenable must not
      // make AntD wait, enter promise loading, or invoke restart a second time.
      return restartThenable;
    },
    logError: (label, error) => state.errors.push(label + ' ' + String(error)),
    showError: error => state.errors.push(error),
  });
}

window.restoreSuccessFixture = {
  text: t,
  content,
  snapshot: () => ({ ...state, loading: [...state.loading], errors: [...state.errors] }),
  finishRestore() {
    if (restoreSettled) throw new Error('Synthetic restore was already settled');
    restoreSettled = true;
    restore.resolve({ content });
  },
  finishRestart() {
    if (state.restartSettled) throw new Error('Synthetic restart was already settled');
    state.restartSettled = true;
    restart.resolve();
  },
};

export default function RestoreSuccessFixture() {
  // This is only a flow launcher, not a replica of GeneralSettingsPage's UI.
  return <Button data-testid="start-success-restore" onClick={startRestore}>Start synthetic restore</Button>;
}
