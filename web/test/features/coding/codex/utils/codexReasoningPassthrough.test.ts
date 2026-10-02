/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  codexReasoningPassthroughApplies,
  readCodexPreserveReasoningEffort,
  withCodexPreserveReasoningEffort,
} from '../../../../../features/coding/codex/utils/codexReasoningPassthrough.ts';

test('codexReasoningPassthroughApplies only matches a custom openai_chat upstream', () => {
  assert.equal(
    codexReasoningPassthroughApplies({
      isOfficial: false,
      isCustomProvider: true,
      apiFormat: 'openai_chat',
    }),
    true,
  );
  // Same-protocol passthrough and other conversion targets have no use for it.
  assert.equal(
    codexReasoningPassthroughApplies({
      isOfficial: false,
      isCustomProvider: true,
      apiFormat: 'openai_responses',
    }),
    false,
  );
  assert.equal(
    codexReasoningPassthroughApplies({
      isOfficial: false,
      isCustomProvider: true,
      apiFormat: 'anthropic_messages',
    }),
    false,
  );
  // Profile endpoints own their `codexChatReasoning` dialect.
  assert.equal(
    codexReasoningPassthroughApplies({
      isOfficial: false,
      isCustomProvider: false,
      apiFormat: 'openai_chat',
    }),
    false,
  );
  assert.equal(
    codexReasoningPassthroughApplies({
      isOfficial: true,
      isCustomProvider: true,
      apiFormat: 'openai_chat',
    }),
    false,
  );
});

test('readCodexPreserveReasoningEffort defaults to on and honors an explicit opt-out', () => {
  assert.equal(readCodexPreserveReasoningEffort(undefined), true);
  assert.equal(readCodexPreserveReasoningEffort({}), true);
  assert.equal(readCodexPreserveReasoningEffort({ preserveReasoningEffort: true }), true);
  assert.equal(readCodexPreserveReasoningEffort({ preserveReasoningEffort: false }), false);
  assert.equal(readCodexPreserveReasoningEffort({ preserve_reasoning_effort: false }), false);
});

test('withCodexPreserveReasoningEffort writes when applicable and clears otherwise', () => {
  assert.deepEqual(
    withCodexPreserveReasoningEffort({ providerType: 'custom' }, true, true),
    { providerType: 'custom', preserveReasoningEffort: true },
  );
  assert.deepEqual(
    withCodexPreserveReasoningEffort({ providerType: 'custom' }, false, true),
    { providerType: 'custom', preserveReasoningEffort: false },
  );
  // Switching to a profile endpoint / non-Chat target drops any stale flag.
  assert.deepEqual(
    withCodexPreserveReasoningEffort(
      { providerType: 'custom', preserveReasoningEffort: true },
      true,
      false,
    ),
    { providerType: 'custom' },
  );
  assert.deepEqual(
    withCodexPreserveReasoningEffort(
      { preserve_reasoning_effort: true },
      true,
      false,
    ),
    undefined,
  );
});