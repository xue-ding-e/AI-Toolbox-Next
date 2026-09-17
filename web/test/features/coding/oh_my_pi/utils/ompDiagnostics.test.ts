import assert from 'node:assert/strict';
import test from 'node:test';

import {
  getOmpDiagnostics,
  getTestableOmpModelIds,
  toOmpModelConnectionMap,
} from '../../../../../features/coding/oh_my_pi/utils/ompDiagnostics.ts';
import { OMP_API_DEFAULT_BASE_URL } from '../../../../../features/coding/oh_my_pi/utils/ompApiOptions.ts';
import { buildModelsUrl, getDefaultModelsApiType } from '../../../../../components/common/FetchModelsModal/request.ts';
import { buildProviderConnectivityBatchTarget } from '../../../../../features/coding/shared/providerConnectivity/batchTestTarget.ts';

test('OMP Anthropic diagnostics add the API version without rewriting runtime configuration', () => {
  for (const baseUrl of [OMP_API_DEFAULT_BASE_URL['anthropic-messages']!, 'https://relay.example/anthropic/v1/']) {
    const provider = { api: 'anthropic-messages', baseUrl };
    const snapshot = JSON.stringify(provider);
    const connection = getOmpDiagnostics(provider);
    const expected = baseUrl.includes('relay') ? 'https://relay.example/anthropic/v1' : 'https://api.anthropic.com/v1';
    assert.equal(connection.baseUrl, expected);
    assert.equal(buildModelsUrl(connection.baseUrl, getDefaultModelsApiType(connection.npm), connection.npm), `${expected}/models`);
    assert.equal(JSON.stringify(provider), snapshot);
  }
});

test('OMP Codex diagnostics preserve the native protocol and custom authentication in batch requests', () => {
  const connection = getOmpDiagnostics({ api: 'openai-codex-responses', baseUrl: OMP_API_DEFAULT_BASE_URL['openai-codex-responses'] });
  const headers = { 'ChatGPT-Account-Id': 'test-account' };
  const target = buildProviderConnectivityBatchTarget({
    providerId: 'test', providerName: 'Test', providerConfig: { npm: connection.npm, options: { baseURL: connection.baseUrl, apiKey: 'test-key', headers } },
    apiFormat: connection.apiFormat, modelIds: ['test-model'],
  }, { requireBaseUrl: true, errorMessages: { missingBaseUrl: 'url', missingApiKey: 'key', missingModel: 'model' } });
  assert.equal(target.request?.npm, '@ai-sdk/openai');
  assert.equal(target.request?.apiFormat, 'openai-codex-responses');
  assert.equal(target.request?.baseUrl, 'https://chatgpt.com/backend-api');
  assert.deepEqual(target.request?.headers, headers);
  assert.equal(connection.supportsConnectivity, true);
  assert.equal(connection.supportsModelDiscovery, false);
});

test('unsupported native protocols never fall through to OpenAI-compatible diagnostics', () => {
  for (const api of ['azure-openai-responses', 'bedrock-converse-stream', 'google-gemini-cli', 'google-vertex', 'custom-api', '']) {
    const connection = getOmpDiagnostics({ api, baseUrl: 'https://example.com' });
    assert.equal(connection.supportsConnectivity, false, api);
    assert.equal(connection.supportsModelDiscovery, false, api);
  }
});

test('OMP diagnostics use homogeneous model overrides and fall back to the provider endpoint when mixed', () => {
  const provider = { api: 'openai-completions', baseUrl: 'https://relay.example/v1', models: [
    { id: 'one', api: 'openai-codex-responses' }, { id: 'two', api: 'openai-codex-responses' },
  ] };
  assert.equal(getOmpDiagnostics(provider).apiFormat, 'openai-codex-responses');

  // A model on another protocol no longer greys the provider buttons out:
  // discovery keeps the provider endpoint, connectivity follows each model.
  const mixedProtocol = getOmpDiagnostics({ ...provider, models: [...provider.models, { id: 'chat' }] });
  assert.equal(mixedProtocol.mixedConnections, true);
  assert.equal(mixedProtocol.supportsConnectivity, true);
  assert.equal(mixedProtocol.supportsModelDiscovery, true);
  assert.equal(mixedProtocol.api, 'openai-completions');
  assert.equal(mixedProtocol.baseUrl, 'https://relay.example/v1');

  const mixedBaseUrl = getOmpDiagnostics({
    ...provider, models: [{ id: 'one' }, { id: 'two', baseUrl: 'https://other.example/v1' }],
  });
  assert.equal(mixedBaseUrl.mixedConnections, true);
  assert.equal(mixedBaseUrl.supportsConnectivity, true);
  assert.equal(mixedBaseUrl.supportsModelDiscovery, true);
  assert.equal(mixedBaseUrl.baseUrl, 'https://relay.example/v1');

  assert.equal(getOmpDiagnostics({ api: 'google-generative-ai', baseUrl: 'https://generativelanguage.googleapis.com' }).baseUrl, 'https://generativelanguage.googleapis.com/v1beta');
});

test('a per-model api override keeps discovery enabled and tests each model on its own connection', () => {
  const diagnostics = getOmpDiagnostics({
    api: 'openai-responses',
    baseUrl: 'https://relay.example/v1',
    models: [
      { id: 'gpt-5' },
      { id: 'claude-sonnet', api: 'anthropic-messages', baseUrl: 'https://relay.example/anthropic' },
    ],
  });

  // Issue #360: this exact shape used to disable "fetch models" entirely.
  assert.equal(diagnostics.supportsModelDiscovery, true);
  assert.equal(diagnostics.supportsConnectivity, true);
  assert.equal(diagnostics.mixedConnections, true);
  assert.equal(diagnostics.api, 'openai-responses');
  assert.equal(diagnostics.npm, '@ai-sdk/openai');
  assert.equal(buildModelsUrl(diagnostics.baseUrl, getDefaultModelsApiType(diagnostics.npm), diagnostics.npm), 'https://relay.example/v1/models');

  const modelIds = getTestableOmpModelIds(diagnostics.modelConnections);
  const modelConnections = toOmpModelConnectionMap(diagnostics.modelConnections);
  assert.deepEqual(modelIds, ['gpt-5', 'claude-sonnet']);
  assert.deepEqual(modelConnections, {
    'gpt-5': { npm: '@ai-sdk/openai', baseUrl: 'https://relay.example/v1', apiFormat: undefined },
    'claude-sonnet': { npm: '@ai-sdk/anthropic', baseUrl: 'https://relay.example/anthropic/v1', apiFormat: undefined },
  });

  // The batch probe follows the model it picked, not the provider connection.
  const target = buildProviderConnectivityBatchTarget({
    providerId: 'p',
    providerName: 'P',
    providerConfig: { npm: diagnostics.npm, options: { baseURL: diagnostics.baseUrl, apiKey: 'k' } },
    apiFormat: diagnostics.apiFormat,
    modelIds,
    modelConnections,
  }, {
    requireBaseUrl: true,
    preferredModelId: 'claude-sonnet',
    errorMessages: { missingBaseUrl: 'url', missingApiKey: 'key', missingModel: 'model' },
  });
  assert.equal(target.request?.npm, '@ai-sdk/anthropic');
  assert.equal(target.request?.baseUrl, 'https://relay.example/anthropic/v1');
  assert.equal(target.request?.apiFormat, undefined);
  assert.deepEqual(target.request?.modelIds, ['claude-sonnet']);
});

test('a mixed provider without a provider-level api keeps per-model testing but cannot discover models', () => {
  const diagnostics = getOmpDiagnostics({ baseUrl: 'https://relay.example/v1', models: [
    { id: 'one', api: 'openai-completions' }, { id: 'two', api: 'anthropic-messages' },
  ] });
  assert.equal(diagnostics.mixedConnections, true);
  // Every model still knows its own protocol, so connectivity testing works…
  assert.equal(diagnostics.supportsConnectivity, true);
  assert.deepEqual(getTestableOmpModelIds(diagnostics.modelConnections), ['one', 'two']);
  // …but there is no provider-level protocol to list the catalog with.
  assert.equal(diagnostics.supportsModelDiscovery, false);

});

test('connectivity is disabled when no model has a testable connection', () => {
  const overrides = [
    { id: 'one', api: 'google-vertex' }, { id: 'two', api: 'bedrock-converse-stream' },
  ];

  // Nothing testable anywhere (no provider api either): stay off rather than
  // probing an unknown endpoint.
  const untestable = getOmpDiagnostics({ baseUrl: 'https://relay.example/v1', models: overrides });
  assert.equal(untestable.supportsConnectivity, false);
  assert.equal(untestable.supportsModelDiscovery, false);
  assert.deepEqual(getTestableOmpModelIds(untestable.modelConnections), []);

  // A testable provider api is not enough when every model overrides to an
  // untestable protocol: the button must not open an empty test list.
  const overriddenEverywhere = getOmpDiagnostics({ api: 'openai-responses', baseUrl: 'https://relay.example/v1', models: overrides });
  assert.equal(overriddenEverywhere.supportsModelDiscovery, true);
  assert.equal(overriddenEverywhere.supportsConnectivity, false);
});
