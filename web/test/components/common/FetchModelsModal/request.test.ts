/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';
import { buildModelsUrl, getDefaultModelsApiType } from '../../../../components/common/FetchModelsModal/request.ts';

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
