/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import { formatUpdateFailureReason } from '../../utils/updateFailureReason.ts';

// Tauri commands declared as `Result<T, String>` reject with the bare string.
// If that case is not handled, a failed auto-update shows a generic message and
// the backend reason is lost — which is exactly how #383 became undiagnosable.
test('reads the reason out of the bare string Tauri rejects with', () => {
  assert.equal(
    formatUpdateFailureReason('Failed to install update: updater is not a valid deb package'),
    'Failed to install update: updater is not a valid deb package',
  );
});

test('trims surrounding whitespace', () => {
  assert.equal(formatUpdateFailureReason('  failed  \n'), 'failed');
});

test('reads the reason out of an Error', () => {
  assert.equal(formatUpdateFailureReason(new Error('network unreachable')), 'network unreachable');
});

test('reads the reason out of an object with a message', () => {
  assert.equal(formatUpdateFailureReason({ message: 'temp dir not on same mount point' }), 'temp dir not on same mount point');
});

test('returns an empty string when nothing usable is available', () => {
  // Callers use the empty string to skip the detail line, so these must never
  // fall through to "undefined" or "[object Object]".
  assert.equal(formatUpdateFailureReason(undefined), '');
  assert.equal(formatUpdateFailureReason(null), '');
  assert.equal(formatUpdateFailureReason(42), '');
  assert.equal(formatUpdateFailureReason({ code: 'EPERM' }), '');
  assert.equal(formatUpdateFailureReason({ message: 42 }), '');
});