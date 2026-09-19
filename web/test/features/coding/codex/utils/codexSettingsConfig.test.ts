/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import { buildCodexSettingsConfig } from '../../../../../features/coding/codex/utils/codexSettingsConfig.ts';
import {
  extractCodexModel,
  extractCodexReasoningEffort,
} from '../../../../../utils/codexConfigUtils.ts';

const CUSTOM_CONFIG = [
  'model_provider = "custom"',
  'model_reasoning_effort = "high"',
  '',
  '[model_providers.custom]',
  'name = "OpenAI"',
  'wire_api = "responses"',
].join('\n');

function buildSettings(options: { reasoningEffort?: string }) {
  const raw = buildCodexSettingsConfig({
    category: 'custom',
    apiKey: 'sk-test',
    baseUrl: 'https://example.com/v1',
    model: 'glm-5.3',
    reasoningEffort: options.reasoningEffort,
    config: CUSTOM_CONFIG,
    catalogModels: [
      { model: 'glm-5.3', displayName: 'GLM 5.3', defaultReasoningLevel: 'max' },
    ],
    auth: {},
  });
  return JSON.parse(raw) as { config: string };
}

test('main model reasoning effort is projected into config.toml with the model', () => {
  const settings = buildSettings({ reasoningEffort: 'max' });

  assert.equal(extractCodexModel(settings.config), 'glm-5.3');
  assert.equal(extractCodexReasoningEffort(settings.config), 'max');
});

test('missing main-model reasoning effort keeps the stored config.toml value', () => {
  const settings = buildSettings({});

  assert.equal(extractCodexModel(settings.config), 'glm-5.3');
  assert.equal(extractCodexReasoningEffort(settings.config), 'high');
});
