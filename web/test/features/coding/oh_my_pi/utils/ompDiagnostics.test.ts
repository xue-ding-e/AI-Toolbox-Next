import assert from 'node:assert/strict';
import test from 'node:test';

import { getOmpDiagnostics } from '../../../../../features/coding/oh_my_pi/utils/ompDiagnostics.ts';
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

test('OMP diagnostics use homogeneous model overrides and refuse mixed connections', () => {
  const provider = { api: 'openai-completions', baseUrl: 'https://relay.example/v1', models: [
    { id: 'one', api: 'openai-codex-responses' }, { id: 'two', api: 'openai-codex-responses' },
  ] };
  assert.equal(getOmpDiagnostics(provider).apiFormat, 'openai-codex-responses');
  assert.equal(getOmpDiagnostics({ ...provider, models: [...provider.models, { id: 'chat' }] }).supportsConnectivity, false);
  assert.equal(getOmpDiagnostics({ ...provider, models: [{ id: 'one' }, { id: 'two', baseUrl: 'https://other.example/v1' }] }).supportsConnectivity, false);
  assert.equal(getOmpDiagnostics({ api: 'google-generative-ai', baseUrl: 'https://generativelanguage.googleapis.com' }).baseUrl, 'https://generativelanguage.googleapis.com/v1beta');
});
