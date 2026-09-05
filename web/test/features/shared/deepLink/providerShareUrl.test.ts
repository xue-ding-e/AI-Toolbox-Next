/// <reference types="node" />

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildProviderShareUrl,
  extractProviderConnectionFields,
  maskApiKey,
  sanitizeShareHomepage,
} from '../../../../features/shared/deepLink/providerShareUrl.ts';

test('buildProviderShareUrl emits the full param set for a complete provider', () => {
  const url = buildProviderShareUrl({
    app: 'claude',
    name: 'My Provider',
    category: 'third_party',
    apiKey: 'sk-test-123',
    baseUrl: 'https://api.example.com/v1',
    model: 'claude-sonnet-4',
    homepage: 'https://example.com',
    notes: 'hello world',
    icon: 'star',
    iconColor: '#0958d9',
  });

  assert.ok(url.startsWith('aitoolbox://v1/import?'), url);
  const params = new URLSearchParams(url.split('?')[1]);
  assert.equal(params.get('resource'), 'provider');
  assert.equal(params.get('app'), 'claude');
  assert.equal(params.get('name'), 'My Provider');
  assert.equal(params.get('category'), 'third_party');
  assert.equal(params.get('apiKey'), 'sk-test-123');
  assert.equal(params.get('baseUrl'), 'https://api.example.com/v1');
  assert.equal(params.get('model'), 'claude-sonnet-4');
  assert.equal(params.get('homepage'), 'https://example.com');
  assert.equal(params.get('notes'), 'hello world');
  assert.equal(params.get('icon'), 'star');
  assert.equal(params.get('iconColor'), '#0958d9');
});

test('buildProviderShareUrl omits empty and absent optional fields', () => {
  const url = buildProviderShareUrl({
    app: 'codex',
    name: 'Official',
    category: 'official',
  });

  const query = url.split('?')[1];
  const params = new URLSearchParams(query);
  assert.equal(params.get('name'), 'Official');
  assert.equal(params.get('category'), 'official');
  assert.equal(params.get('apiKey'), null);
  assert.equal(params.get('baseUrl'), null);
  assert.equal(params.get('model'), null);
  assert.equal(params.get('homepage'), null);
  assert.equal(params.get('notes'), null);
  assert.equal(params.get('icon'), null);
  assert.equal(params.get('iconColor'), null);
  assert.ok(!query.includes('config='), 'must never emit a config blob');
  assert.ok(!query.includes('extra='), 'must never emit an extra blob');
});

test('buildProviderShareUrl encodes unicode and reserved characters', () => {
  const url = buildProviderShareUrl({
    app: 'gemini',
    name: '供应商 & 渠道=测试',
    baseUrl: 'https://api.example.com/v1?a=1&b=2',
  });

  const params = new URLSearchParams(url.split('?')[1]);
  assert.equal(params.get('name'), '供应商 & 渠道=测试');
  assert.equal(params.get('baseUrl'), 'https://api.example.com/v1?a=1&b=2');
});

test('buildProviderShareUrl drops non-http homepage values', () => {
  const url = buildProviderShareUrl({
    app: 'claude',
    name: 'T',
    homepage: 'javascript:alert(1)',
  });
  const params = new URLSearchParams(url.split('?')[1]);
  assert.equal(params.get('homepage'), null);

  const httpUrl = buildProviderShareUrl({
    app: 'claude',
    name: 'T',
    homepage: 'http://insecure.example.com',
  });
  assert.equal(
    new URLSearchParams(httpUrl.split('?')[1]).get('homepage'),
    'http://insecure.example.com',
  );
});

test('sanitizeShareHomepage mirrors the URL param filter for local imports', () => {
  // The direct local-import request must use the same filter as the URL
  // builder — an unfiltered non-http value would hard-fail the backend parse.
  assert.equal(sanitizeShareHomepage('https://example.com'), 'https://example.com');
  assert.equal(sanitizeShareHomepage('  http://example.com  '), 'http://example.com');
  assert.equal(sanitizeShareHomepage('example.com'), undefined);
  assert.equal(sanitizeShareHomepage('ftp://example.com'), undefined);
  assert.equal(sanitizeShareHomepage('javascript:alert(1)'), undefined);
  assert.equal(sanitizeShareHomepage(''), undefined);
  assert.equal(sanitizeShareHomepage(undefined), undefined);
});

test('extractProviderConnectionFields reads claude env keys', () => {
  const fields = extractProviderConnectionFields(
    'claude',
    JSON.stringify({
      env: {
        ANTHROPIC_AUTH_TOKEN: 'sk-auth-token',
        ANTHROPIC_BASE_URL: 'https://api.anthropic.com',
        ANTHROPIC_MODEL: 'claude-sonnet-4-5',
      },
    }),
  );
  assert.deepEqual(fields, {
    apiKey: 'sk-auth-token',
    baseUrl: 'https://api.anthropic.com',
    model: 'claude-sonnet-4-5',
  });
});

test('extractProviderConnectionFields falls back to legacy ANTHROPIC_API_KEY', () => {
  const fields = extractProviderConnectionFields(
    'claude',
    JSON.stringify({ env: { ANTHROPIC_API_KEY: 'sk-legacy' } }),
  );
  assert.equal(fields.apiKey, 'sk-legacy');
});

test('extractProviderConnectionFields uses a role model when no default model is set', () => {
  const fields = extractProviderConnectionFields(
    'claude',
    JSON.stringify({
      env: {
        ANTHROPIC_AUTH_TOKEN: 'sk-x',
        ANTHROPIC_DEFAULT_SONNET_MODEL: 'claude-sonnet-4-5',
        ANTHROPIC_DEFAULT_OPUS_MODEL: 'claude-opus-4-1',
      },
    }),
  );
  assert.equal(fields.model, 'claude-sonnet-4-5');
});

test('extractProviderConnectionFields strips the claude-only [1M] suffix', () => {
  const fields = extractProviderConnectionFields(
    'claude',
    JSON.stringify({ env: { ANTHROPIC_MODEL: 'claude-sonnet-4-5[1M]' } }),
  );
  assert.equal(fields.model, 'claude-sonnet-4-5');
});

test('extractProviderConnectionFields prefers ANTHROPIC_MODEL over role models', () => {
  const fields = extractProviderConnectionFields(
    'claude',
    JSON.stringify({
      env: {
        ANTHROPIC_MODEL: 'claude-haiku-4-5',
        ANTHROPIC_DEFAULT_OPUS_MODEL: 'claude-opus-4-1',
      },
    }),
  );
  assert.equal(fields.model, 'claude-haiku-4-5');
});

test('extractProviderConnectionFields reads codex auth and TOML config', () => {
  const toml = [
    'model_provider = "my-provider"',
    'model = "gpt-5"',
    '',
    '[model_providers.my-provider]',
    'name = "My Provider"',
    'base_url = "https://api.example.com/v1"',
  ].join('\n');
  const fields = extractProviderConnectionFields(
    'codex',
    JSON.stringify({ auth: { OPENAI_API_KEY: 'sk-openai' }, config: toml }),
  );
  assert.deepEqual(fields, {
    apiKey: 'sk-openai',
    baseUrl: 'https://api.example.com/v1',
    model: 'gpt-5',
  });
});

test('extractProviderConnectionFields reads gemini env keys', () => {
  const fields = extractProviderConnectionFields(
    'gemini',
    JSON.stringify({
      env: {
        GEMINI_API_KEY: 'sk-gemini',
        GOOGLE_GEMINI_BASE_URL: 'https://generativelanguage.googleapis.com',
        GEMINI_MODEL: 'gemini-2.5-pro',
      },
    }),
  );
  assert.deepEqual(fields, {
    apiKey: 'sk-gemini',
    baseUrl: 'https://generativelanguage.googleapis.com',
    model: 'gemini-2.5-pro',
  });
});

test('extractProviderConnectionFields returns empty object for invalid json', () => {
  assert.deepEqual(extractProviderConnectionFields('claude', 'not-json'), {});
  assert.deepEqual(extractProviderConnectionFields('claude', undefined), {});
  assert.deepEqual(extractProviderConnectionFields('claude', ''), {});
  assert.deepEqual(extractProviderConnectionFields('codex', '[1,2,3]'), {});
});

test('extractProviderConnectionFields omits blank env values', () => {
  const fields = extractProviderConnectionFields(
    'gemini',
    JSON.stringify({ env: { GEMINI_API_KEY: '   ' } }),
  );
  assert.deepEqual(fields, {});
});

test('extract and build compose into a valid share URL for a codex provider', () => {
  const toml = [
    'model_provider = "openrouter"',
    '[model_providers.openrouter]',
    'base_url = "https://openrouter.ai/api/v1"',
  ].join('\n');
  const fields = extractProviderConnectionFields(
    'codex',
    JSON.stringify({ auth: { OPENAI_API_KEY: 'sk-or' }, config: toml }),
  );
  const url = buildProviderShareUrl({
    app: 'claude',
    name: 'OpenRouter',
    category: 'third_party',
    ...fields,
  });

  const params = new URLSearchParams(url.split('?')[1]);
  assert.equal(params.get('app'), 'claude');
  assert.equal(params.get('apiKey'), 'sk-or');
  assert.equal(params.get('baseUrl'), 'https://openrouter.ai/api/v1');
});

test('maskApiKey keeps a short prefix and masks the remainder', () => {
  assert.equal(maskApiKey('sk-abcdef123456'), 'sk-a' + '*'.repeat(20));
  assert.equal(maskApiKey('abcd'), '****');
  assert.equal(maskApiKey('abc'), '****');
  assert.equal(maskApiKey(undefined), '');
  assert.equal(maskApiKey(''), '');
});
