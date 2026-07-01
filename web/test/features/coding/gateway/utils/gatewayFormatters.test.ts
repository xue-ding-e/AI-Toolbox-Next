import assert from 'node:assert/strict';
import test from 'node:test';

import {
  normalizeAttemptCounts,
  shouldShowBodyComparison,
} from '../../../../../features/coding/gateway/utils/gatewayFormatters.ts';

test('normalizeAttemptCounts falls back total attempts for legacy request logs', () => {
  assert.deepEqual(normalizeAttemptCounts({ attempt_count: 2, total_attempt_count: 0 }), {
    current: 2,
    total: 2,
  });
});

test('normalizeAttemptCounts keeps total attempts when present', () => {
  assert.deepEqual(normalizeAttemptCounts({ attempt_count: 1, total_attempt_count: 3 }), {
    current: 1,
    total: 3,
  });
});

test('shouldShowBodyComparison only shows distinct stored bodies', () => {
  assert.equal(shouldShowBodyComparison(null, '{"ok":true}'), false);
  assert.equal(shouldShowBodyComparison('{"ok":true}', '{"ok":true}'), false);
  assert.equal(shouldShowBodyComparison('{"upstream":true}', '{"client":true}'), true);
});
