import React from 'react';
import { Button } from 'antd';
import { confirmRestore } from '@/features/settings/components/confirmRestore';
import { promptRestorePassword } from '@/features/settings/components/promptRestorePassword';
import { executeRestoreFlow } from '@/features/settings/utils/restoreFlow';
import i18n from '@/i18n';

// The confirmation, option holder and password retry all use production code.
// Only service/feedback boundaries are synthetic; no native restore API is used.
const state = {
  calls: [],
  executions: 0,
  executeThenReads: 0,
  loading: [],
  prompts: [],
  successes: 0,
  errors: [],
  selection: { filename: 'fixture-confirm-backup.zip.enc', sha: 'fixture-confirm-original-sha' },
};
const t = key => i18n.t(key);
const description = 'Synthetic selected backup: no files will be restored.';
const deferred = () => {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};

function openConfirmation() {
  // Like the page, retain the selected object in the service closure. Replacing
  // state.selection later must not redirect this confirmation or its retries.
  // This is not a replica/test of the page's local/WebDAV/repository routing.
  const file = state.selection;
  state.confirmReturnedUndefined = confirmRestore(t, description, options => {
    const request = {
      filename: file.filename, sha: file.sha, ...options,
      ...deferred(), settled: false,
    };
    state.calls.push(request);
    return request.promise;
  }, restore => {
    state.executions++;
    const run = executeRestoreFlow(restore, {
      setLoading: value => state.loading.push(value),
      translate: t,
      buildSuccessContent: () => 'Synthetic restore result',
      showSuccess: () => { state.successes++; },
      showWarnings: () => {},
      promptPassword: (onSubmit, wrongAttempt) => {
        state.prompts.push(wrongAttempt);
        promptRestorePassword(t, onSubmit, wrongAttempt);
      },
      restart: () => {},
      logError: (label, error) => state.errors.push(label + ' ' + String(error)),
      showError: error => state.errors.push(error),
    });
    // Expose a pending thenable to detect accidental return/await assimilation
    // by the confirmation adapter, without replacing its production onOk.
    return {
      get then() {
        state.executeThenReads++;
        return run.then.bind(run);
      },
    };
  }) === undefined;
}

window.restoreConfirmFixture = {
  state,
  text: t,
  description,
  snapshot() {
    return {
      calls: state.calls.map(({ filename, sha, skipCliCustomRoots, restorePassword, settled }) => ({
        filename, sha, skipCliCustomRoots, restorePassword: restorePassword ?? null, settled,
      })),
      executions: state.executions,
      executeThenReads: state.executeThenReads,
      loading: [...state.loading],
      prompts: [...state.prompts],
      successes: state.successes,
      errors: [...state.errors],
      confirmReturnedUndefined: state.confirmReturnedUndefined,
    };
  },
  settle(index, errorType) {
    const request = state.calls[index];
    if (!request || request.settled) throw new Error('Missing or settled synthetic restore: ' + index);
    request.settled = true;
    if (errorType) request.reject(JSON.stringify({ type: errorType }));
    else request.resolve({ warnings: [] });
  },
};

export default function RestoreConfirmFixture() {
  return <Button data-testid="open-restore-confirm" onClick={openConfirmation}>Confirm synthetic restore</Button>;
}
