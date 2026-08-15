/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  OPENCLAW_PROFILE_OPTIONS,
  OPENCLAW_TOOLS_PROFILES,
} from '../../../../features/coding/openclaw/constants.ts';

const profiles = OPENCLAW_TOOLS_PROFILES as readonly string[];

test('OPENCLAW_TOOLS_PROFILES contains exactly the four upstream values', () => {
  assert.deepEqual([...profiles], ['minimal', 'coding', 'messaging', 'full']);
});

test('OPENCLAW_TOOLS_PROFILES has no legacy values', () => {
  for (const legacy of ['default', 'strict', 'permissive', 'custom']) {
    assert.equal(profiles.includes(legacy), false);
  }
});

test('OPENCLAW_PROFILE_OPTIONS label keys match the new enum', () => {
  assert.deepEqual(
    OPENCLAW_PROFILE_OPTIONS.map((option) => option.value),
    ['minimal', 'coding', 'messaging', 'full']
  );
  assert.deepEqual(
    OPENCLAW_PROFILE_OPTIONS.map((option) => option.labelKey),
    [
      'openclaw.tools.profileMinimal',
      'openclaw.tools.profileCoding',
      'openclaw.tools.profileMessaging',
      'openclaw.tools.profileFull',
    ]
  );
});