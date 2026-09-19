import assert from 'node:assert/strict';
import test from 'node:test';

import {
  SESSION_TIME_RANGE_OPTIONS,
} from '../../../../../features/coding/shared/sessionManager/utils.ts';

test('session time range presets mirror the backend wire contract', () => {
  // Mirrors `SessionTimeRange::parse` in
  // `tauri/src/coding/session_manager/mod.rs`; both sides must change together.
  const backendPresets = ['all', 'today', '7d', '30d', 'older_30d'];

  assert.deepEqual(
    SESSION_TIME_RANGE_OPTIONS.map((option) => option.value),
    backendPresets,
  );
  assert.equal(SESSION_TIME_RANGE_OPTIONS[0].labelKey, 'sessionManager.timeRange.all');
});

test('session time range presets reference unique i18n label keys', () => {
  const labelKeys = SESSION_TIME_RANGE_OPTIONS.map((option) => option.labelKey);

  assert.equal(new Set(labelKeys).size, labelKeys.length);
  labelKeys.forEach((labelKey) => assert.match(labelKey, /^sessionManager\.timeRange\./));
});
