import React from 'react';
import { Button } from 'antd';
import { promptRestorePassword } from '@/features/settings/components/promptRestorePassword';
import { executeRestoreFlow } from '@/features/settings/utils/restoreFlow';
import i18n from '@/i18n';

// Only callback/service boundaries are doubled. The dialog, input, validation,
// cancellation and recursive retry all come from production implementations.
const state = {
  submissions: [],
  calls: [],
  loading: [],
  successes: 0,
  errors: [],
  callbackSettled: false,
  selection: { filename: 'fixture-backup.zip.enc', sha: 'fixture-original-sha' },
};
const t = key => i18n.t(key);
const deferred = () => {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
const callback = deferred();
function openPrompt(wrongAttempt) {
  state.promptReturnedUndefined = promptRestorePassword(t, password => {
    state.submissions.push(password);
    // Even a thenable returned by a callback must not hold the dialog open:
    // production intentionally invokes this void callback without returning it.
    return callback.promise;
  }, wrongAttempt) === undefined;
}
function startRestore() {
  const { filename, sha } = state.selection;
  // This synthetic service closure models a selected revision. It does not
  // claim to cover GeneralSettingsPage's selection routing or a real backend.
  const restore = password => {
    const request = { filename, sha, password, ...deferred() };
    state.calls.push(request);
    return request.promise;
  };
  void executeRestoreFlow(restore, {
    setLoading: value => state.loading.push(value),
    translate: t,
    buildSuccessContent: () => 'fixture restore result',
    showSuccess: () => { state.successes++; },
    showWarnings: () => {},
    promptPassword: (onSubmit, wrongAttempt) => promptRestorePassword(t, onSubmit, wrongAttempt),
    restart: () => {},
    logError: () => {},
    showError: error => state.errors.push(error),
  });
}
window.restorePasswordFixture = {
  state,
  text: t,
  snapshot() {
    return {
      submissions: [...state.submissions],
      calls: state.calls.map(({ filename, sha, password }) => ({ filename, sha, password: password ?? null })),
      loading: [...state.loading],
      successes: state.successes,
      errors: [...state.errors],
      callbackSettled: state.callbackSettled,
      promptReturnedUndefined: state.promptReturnedUndefined,
    };
  },
  settle(index, errorType) {
    const request = state.calls[index];
    if (!request || request.settled) throw new Error('Missing or settled restore request: ' + index);
    request.settled = true;
    if (errorType) request.reject(JSON.stringify({ type: errorType }));
    else request.resolve({ warnings: [] });
  },
  finishCallback() {
    state.callbackSettled = true;
    callback.resolve();
  },
};
export default function RestorePasswordFixture() {
  return <>
    <Button data-testid="open-password" onClick={() => openPrompt(false)}>Open password</Button>
    <Button data-testid="open-wrong-password" onClick={() => openPrompt(true)}>Open wrong password</Button>
    <Button data-testid="start-password-restore" onClick={startRestore}>Start synthetic restore</Button>
  </>;
}
