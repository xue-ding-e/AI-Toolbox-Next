/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buildModelsUrl,
  getDefaultModelsApiType,
  resolveModelsUrlApiKey,
} from '../../../../components/common/FetchModelsModal/request.ts';

test('model discovery selects native mode only for SDKs with native support', () => {
  assert.equal(getDefaultModelsApiType('@ai-sdk/anthropic'), 'native');
  assert.equal(getDefaultModelsApiType('@ai-sdk/google'), 'native');
  assert.equal(getDefaultModelsApiType('@ai-sdk/openai'), 'openai_compat');
  assert.equal(getDefaultModelsApiType('@ai-sdk/openai-compatible'), 'openai_compat');
  assert.equal(getDefaultModelsApiType(), 'openai_compat');
});

test('Gemini discovery supplies a version and key without rewriting stored base URLs', () => {
  assert.equal(buildModelsUrl(' https://gemini.example.test/// ', 'native', '@ai-sdk/google', 'a+b&c'),
    'https://gemini.example.test/v1beta/models?key=a%2Bb%26c');
  for (const version of ['v1', 'v1alpha', 'v1beta']) {
    assert.equal(buildModelsUrl(`https://gemini.example.test/${version}/`, 'native', '@ai-sdk/google', 'key'),
      `https://gemini.example.test/${version}/models?key=key`);
  }
});

test('OpenAI-compatible and Anthropic discovery preserve base paths and omit query credentials', () => {
  assert.equal(buildModelsUrl('https://api.example.test/v1/', 'native', '@ai-sdk/anthropic', 'key'),
    'https://api.example.test/v1/models');
  assert.equal(buildModelsUrl('https://api.example.test/custom', 'openai_compat', '@ai-sdk/google', 'key'),
    'https://api.example.test/custom/models');
  assert.equal(buildModelsUrl('', 'native', '@ai-sdk/google', 'key'), '');
});

test('config value modes keep raw provider config values out of the previewed URL', () => {
  // Pi `$ENV_VAR` / `!command` templates and OMP env names / `!command` values
  // are all resolved by the backend, so none of them belong in the URL preview.
  for (const configValueMode of ['pi', 'omp'] as const) {
    assert.equal(resolveModelsUrlApiKey('$PI_KEY', configValueMode), undefined);
    assert.equal(resolveModelsUrlApiKey('MY_PROVIDER_KEY', configValueMode), undefined);
    assert.equal(resolveModelsUrlApiKey('!bw get password provider-key', configValueMode), undefined);
  }
  assert.equal(resolveModelsUrlApiKey('sk-live', undefined), 'sk-live');

  // Google native auth travels in the query string, so the backend completes it
  // with the resolved key instead of the URL preview leaking the raw template.
  assert.equal(
    buildModelsUrl('https://gemini.example.test', 'native', '@ai-sdk/google', resolveModelsUrlApiKey('$PI_KEY', 'pi')),
    'https://gemini.example.test/v1beta/models',
  );
  assert.equal(
    buildModelsUrl('https://gemini.example.test', 'native', '@ai-sdk/google', resolveModelsUrlApiKey('sk-live', undefined)),
    'https://gemini.example.test/v1beta/models?key=sk-live',
  );
});
