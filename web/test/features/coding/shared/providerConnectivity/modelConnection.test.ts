import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildTokenCapFields,
  resolveModelConnection,
} from '../../../../../features/coding/shared/providerConnectivity/modelConnection.ts';

const baseRequest = {
  npm: '@ai-sdk/openai',
  baseUrl: 'https://relay.example/v1',
  apiFormat: undefined as 'openai-codex-responses' | undefined,
  providerId: 'p',
  modelIds: ['m'],
};

test('a model without its own connection keeps the provider-level request untouched', () => {
  const request = resolveModelConnection(baseRequest, 'plain', {
    overridden: { npm: '@ai-sdk/anthropic', baseUrl: 'https://relay.example/anthropic/v1' },
  });
  assert.equal(request, baseRequest);
});

test('a model override replaces the connection but keeps the rest of the request', () => {
  const request = resolveModelConnection(baseRequest, 'overridden', {
    overridden: { npm: '@ai-sdk/anthropic', baseUrl: 'https://relay.example/anthropic/v1' },
  });
  assert.deepEqual(request, {
    npm: '@ai-sdk/anthropic',
    baseUrl: 'https://relay.example/anthropic/v1',
    apiFormat: undefined,
    providerId: 'p',
    modelIds: ['m'],
  });
});

test('leaving Codex for another protocol drops the Codex request shape', () => {
  const codexRequest = { ...baseRequest, npm: '@ai-sdk/openai', apiFormat: 'openai-codex-responses' as const };
  assert.equal(resolveModelConnection(codexRequest, 'plain', {
    plain: { npm: '@ai-sdk/anthropic', baseUrl: 'https://api.anthropic.com/v1' },
  }).apiFormat, undefined);
});

test('a model override keeps its own Codex request shape', () => {
  const request = resolveModelConnection(baseRequest, 'codex', {
    codex: { npm: '@ai-sdk/openai', baseUrl: 'https://chatgpt.com/backend-api', apiFormat: 'openai-codex-responses' },
  });
  assert.equal(request.apiFormat, 'openai-codex-responses');
  assert.equal(request.baseUrl, 'https://chatgpt.com/backend-api');
});

test('the token cap is named after the SDK that serves the model', () => {
  assert.deepEqual(buildTokenCapFields('@ai-sdk/google', 2048), { maxOutputTokens: 2048 });
  assert.deepEqual(buildTokenCapFields('@ai-sdk/anthropic', 2048), { maxTokens: 2048 });
  assert.deepEqual(buildTokenCapFields('@ai-sdk/openai', 2048), { maxTokens: 2048 });
  assert.deepEqual(buildTokenCapFields('@ai-sdk/openai-compatible', 2048), { maxTokens: 2048 });
  assert.deepEqual(buildTokenCapFields('@ai-sdk/google', undefined), {});
});

test('an unset token cap adds nothing, and the resolved npm picks the field', () => {
  const googleModel = resolveModelConnection(baseRequest, 'gemini', {
    gemini: { npm: '@ai-sdk/google', baseUrl: 'https://generativelanguage.googleapis.com/v1beta' },
  });
  // The provider is OpenAI-shaped, so a Google model must still get maxOutputTokens.
  assert.deepEqual(buildTokenCapFields(googleModel.npm, 1024), { maxOutputTokens: 1024 });
  assert.deepEqual(buildTokenCapFields(baseRequest.npm, 1024), { maxTokens: 1024 });
  assert.deepEqual(buildTokenCapFields(googleModel.npm, undefined), {});
});