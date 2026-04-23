import assert from 'node:assert/strict';
import test from 'node:test';

import {
  advanceVisibleContextId,
  shouldShowVisibleFeedback,
} from '../../../../../features/coding/shared/sessionManager/utils.ts';

test('advanceVisibleContextId only increments when page becomes hidden', () => {
  assert.equal(advanceVisibleContextId(0, true, true), 0);
  assert.equal(advanceVisibleContextId(0, false, false), 0);
  assert.equal(advanceVisibleContextId(2, false, true), 2);
  assert.equal(advanceVisibleContextId(3, true, false), 4);
});

test('shouldShowVisibleFeedback requires active page and current visible context', () => {
  assert.equal(shouldShowVisibleFeedback(true, undefined, 5), true);
  assert.equal(shouldShowVisibleFeedback(true, 5, 5), true);
  assert.equal(shouldShowVisibleFeedback(true, 4, 5), false);
  assert.equal(shouldShowVisibleFeedback(false, undefined, 5), false);
  assert.equal(shouldShowVisibleFeedback(false, 5, 5), false);
});

test('previous visible context stays stale after page is hidden and shown again', () => {
  const requestVisibleContextId = 2;
  const hiddenVisibleContextId = advanceVisibleContextId(requestVisibleContextId, true, false);
  const shownVisibleContextId = advanceVisibleContextId(hiddenVisibleContextId, false, true);

  assert.equal(hiddenVisibleContextId, 3);
  assert.equal(shownVisibleContextId, 3);
  assert.equal(shouldShowVisibleFeedback(true, requestVisibleContextId, shownVisibleContextId), false);
});
