import test from 'node:test';
import assert from 'node:assert/strict';

import {
  createKimiDeviceAuthStatusClassifier,
  isTerminalKimiDeviceAuthStatus,
} from '../../../../../features/coding/kimi/utils/deviceAuthStatus.ts';

test('device auth: non-terminal statuses always classify as progress', () => {
  const classify = createKimiDeviceAuthStatusClassifier();
  assert.equal(classify('waiting'), 'progress');
  assert.equal(classify('waiting'), 'progress');
  assert.equal(classify('processing'), 'progress');
  assert.equal(classify(''), 'progress');
});

test('device auth: a failure terminal state notifies exactly once despite repeated polling', () => {
  const classify = createKimiDeviceAuthStatusClassifier();
  assert.equal(classify('failed'), 'notify-error');
  // The 5s poll keeps firing until the modal unmounts; every repeat must be
  // silent instead of stacking message.error toasts.
  assert.equal(classify('failed'), 'terminal-silent');
  assert.equal(classify('failed'), 'terminal-silent');
});

test('device auth: each terminal flavor notifies once and only the first one', () => {
  const classify = createKimiDeviceAuthStatusClassifier();
  assert.equal(classify('expired'), 'notify-error');
  // The event listener may deliver a different terminal status right after the
  // poll observed one; it must not produce a second notification.
  assert.equal(classify('error'), 'terminal-silent');
  assert.equal(classify('access_denied'), 'terminal-silent');
});

test('device auth: completed notifies success once, then stays silent', () => {
  const classify = createKimiDeviceAuthStatusClassifier();
  assert.equal(classify('completed'), 'notify-success');
  assert.equal(classify('completed'), 'terminal-silent');
});

test('device auth: cancelled is a silent terminal state (user-initiated)', () => {
  const classify = createKimiDeviceAuthStatusClassifier();
  assert.equal(classify('cancelled'), 'terminal-silent');
  assert.equal(classify('cancelled'), 'terminal-silent');
});

test('device auth: a fresh classifier resets the one-shot guard per session', () => {
  const first = createKimiDeviceAuthStatusClassifier();
  assert.equal(first('failed'), 'notify-error');
  assert.equal(first('failed'), 'terminal-silent');

  const second = createKimiDeviceAuthStatusClassifier();
  assert.equal(second('failed'), 'notify-error', 'a new auth session must notify again');
});

test('device auth: terminal set membership helper', () => {
  for (const status of ['completed', 'cancelled', 'expired', 'denied', 'access_denied', 'failed', 'error']) {
    assert.equal(isTerminalKimiDeviceAuthStatus(status), true, `${status} must be terminal`);
  }
  assert.equal(isTerminalKimiDeviceAuthStatus('waiting'), false);
});
