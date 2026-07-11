/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  canToggleCodexRemoteCompaction,
  extractCodexBaseUrl,
  getCodexIgnoredCommonConfigKeys,
  isCodexGoalModeEnabled,
  isCodexRemoteCompactionEnabled,
  setCodexGoalMode,
  setCodexRemoteCompaction,
} from '../../../../../utils/codexConfigUtils.ts';

test('extractCodexBaseUrl prefers the selected provider table over other locations', () => {
  assert.equal(extractCodexBaseUrl(`
model_provider = "chat"
base_url = "https://legacy.example.com/v1"

[model_providers.responses]
base_url = "https://responses.example.com/v1"

[model_providers.chat]
base_url = "https://chat.example.com/v1/chat/completions"
`), 'https://chat.example.com/v1/chat/completions');
});

test('extractCodexBaseUrl supports root-level legacy configs', () => {
  assert.equal(
    extractCodexBaseUrl('base_url = "https://legacy.example.com/v1"'),
    'https://legacy.example.com/v1',
  );
});

test('setCodexGoalMode adds and detects features.goals', () => {
  const nextConfig = setCodexGoalMode('model = "gpt-5"\n', true);

  assert.match(nextConfig, /\[features\]\ngoals = true/);
  assert.equal(isCodexGoalModeEnabled(nextConfig), true);
});

test('setCodexGoalMode removes only goals from features', () => {
  const config = [
    'model = "gpt-5"',
    '',
    '[features]',
    'plugins = true',
    'goals = true',
    '',
  ].join('\n');

  const nextConfig = setCodexGoalMode(config, false);

  assert.equal(isCodexGoalModeEnabled(nextConfig), false);
  assert.match(nextConfig, /\[features\]\nplugins = true/);
});

test('setCodexGoalMode removes empty features section', () => {
  const nextConfig = setCodexGoalMode('[features]\ngoals = true\n', false);

  assert.equal(nextConfig.includes('[features]'), false);
  assert.equal(isCodexGoalModeEnabled(nextConfig), false);
});

test('setCodexGoalMode updates dotted features key through TOML parser', () => {
  const enabledConfig = setCodexGoalMode('model = "gpt-5"\nfeatures.goals = false # keep comment\n', true);

  assert.equal(isCodexGoalModeEnabled(enabledConfig), true);
  assert.match(enabledConfig, /\[features\]\ngoals = true/);
  assert.doesNotMatch(enabledConfig, /features\.goals/);
});

test('setCodexGoalMode rewrites through TOML parser', () => {
  const nextConfig = setCodexGoalMode('[features]\n# keep section note\ngoals = true\n', false);

  assert.equal(isCodexGoalModeEnabled(nextConfig), false);
  assert.doesNotMatch(nextConfig, /keep section note/);
  assert.doesNotMatch(nextConfig, /goals\s*=/);
});

test('getCodexIgnoredCommonConfigKeys only reports protected common config entries', () => {
  const config = [
    '[features]',
    'plugins = true',
    'test_generation = false',
    '',
    '[mcp_servers.local]',
    'command = "uvx"',
    '',
    '[plugins."demo@local"]',
    'enabled = true',
    '',
  ].join('\n');

  assert.deepEqual(getCodexIgnoredCommonConfigKeys(config), [
    '[mcp_servers]',
    '[plugins]',
    '[features].plugins',
  ]);
});

test('getCodexIgnoredCommonConfigKeys allows regular feature flags', () => {
  const config = [
    '[features]',
    'test_generation = false',
    'image_generation = false',
    '',
  ].join('\n');

  assert.deepEqual(getCodexIgnoredCommonConfigKeys(config), []);
});

test('setCodexRemoteCompaction toggles custom provider name', () => {
  const config = [
    'model_provider = "custom"',
    '',
    '[model_providers.custom]',
    'name = "RightCode"',
    'wire_api = "responses"',
    '',
  ].join('\n');

  const enabledConfig = setCodexRemoteCompaction(config, true);
  assert.equal(canToggleCodexRemoteCompaction(enabledConfig), true);
  assert.equal(isCodexRemoteCompactionEnabled(enabledConfig), true);
  assert.match(enabledConfig, /name = "OpenAI"/);

  const disabledConfig = setCodexRemoteCompaction(enabledConfig, false, 'RightCode');
  assert.equal(isCodexRemoteCompactionEnabled(disabledConfig), false);
  assert.match(disabledConfig, /name = "RightCode"/);

  const defaultDisabledConfig = setCodexRemoteCompaction(enabledConfig, false);
  assert.equal(isCodexRemoteCompactionEnabled(defaultDisabledConfig), false);
  assert.match(defaultDisabledConfig, /name = "custom"/);
});

test('setCodexRemoteCompaction ignores reserved built-in provider ids', () => {
  const config = [
    'model_provider = "openai"',
    '',
    '[model_providers.openai]',
    'name = "OpenAI"',
    '',
  ].join('\n');

  assert.equal(canToggleCodexRemoteCompaction(config), false);
  assert.equal(setCodexRemoteCompaction(config, true), config);
});
