/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  OMP_API_DEFAULT_BASE_URL,
  OMP_API_DESCRIPTION_I18N_KEYS,
  OMP_API_OPTIONS,
  OMP_API_VALUES,
} from '../../../../../features/coding/oh_my_pi/utils/ompApiOptions.ts';

// The authoritative source is oh-my-pi's models.yml `ApiSchema`
// (packages/coding-agent/src/config/models-config-schema-bundle.ts). An unknown
// `api` value fails the whole models.yml validation and disables every custom
// provider, so this list must stay in lockstep with upstream.
const OMP_API_SCHEMA_VALUES = [
  'openai-completions',
  'openai-responses',
  'openai-codex-responses',
  'azure-openai-responses',
  'anthropic-messages',
  'bedrock-converse-stream',
  'google-generative-ai',
  'google-gemini-cli',
  'google-vertex',
] as const;

test('OMP_API_OPTIONS mirrors the omp ApiSchema vocabulary', () => {
  assert.deepEqual([...OMP_API_VALUES], [...OMP_API_SCHEMA_VALUES]);
  assert.deepEqual(
    OMP_API_OPTIONS.map((option) => option.value),
    [...OMP_API_SCHEMA_VALUES],
  );
  for (const option of OMP_API_OPTIONS) {
    assert.equal(option.label, option.value);
  }
});

test('OMP_API_DESCRIPTION_I18N_KEYS covers exactly the api vocabulary', () => {
  assert.deepEqual(Object.keys(OMP_API_DESCRIPTION_I18N_KEYS).sort(), [...OMP_API_SCHEMA_VALUES].sort());
  for (const api of OMP_API_SCHEMA_VALUES) {
    const key = OMP_API_DESCRIPTION_I18N_KEYS[api];
    assert.match(key, /^ohMyPi\.apiDescription\./);
  }
});

test('OMP_API_DEFAULT_BASE_URL only maps known vocabulary to https endpoints', () => {
  for (const [api, baseUrl] of Object.entries(OMP_API_DEFAULT_BASE_URL)) {
    assert.ok(
      (OMP_API_SCHEMA_VALUES as readonly string[]).includes(api),
      `unexpected default baseUrl key: ${api}`,
    );
    assert.match(baseUrl, /^https:\/\//);
  }
});
