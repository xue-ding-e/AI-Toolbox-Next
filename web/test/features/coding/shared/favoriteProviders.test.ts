import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildFavoriteProviderOptions,
  buildFavoriteProviderStorageKey,
  extractFavoriteProviderRawId,
  getFavoriteProviderPayload,
  isFavoriteProviderForSource,
  type DshFavoriteProviderPayload,
  type HermesFavoriteProviderPayload,
  type ClaudeDesktopFavoriteProviderPayload,
} from '../../../../features/coding/shared/favoriteProviders.ts';

test('storage keys for the new dsh/hermes/claudedesktop sources use the prefix separator', () => {
  assert.equal(buildFavoriteProviderStorageKey('dsh', 'openai'), 'dsh:openai');
  assert.equal(buildFavoriteProviderStorageKey('hermes', 'north'), 'hermes:north');
  assert.equal(buildFavoriteProviderStorageKey('claudedesktop', 'abc-123'), 'claudedesktop:abc-123');
});

test('extractFavoriteProviderRawId round-trips the new source keys', () => {
  assert.equal(extractFavoriteProviderRawId('dsh', 'dsh:openai'), 'openai');
  assert.equal(extractFavoriteProviderRawId('hermes', 'hermes:north'), 'north');
  assert.equal(extractFavoriteProviderRawId('claudedesktop', 'claudedesktop:abc-123'), 'abc-123');
});

test('isFavoriteProviderForSource isolates each new source by prefix', () => {
  const dshFavorite = {
    id: '1', providerId: 'dsh:openai', npm: '', baseUrl: '',
    providerConfig: { npm: '', name: '', options: {}, models: {} },
    createdAt: '', updatedAt: '',
  };
  const hermesFavorite = {
    id: '2', providerId: 'hermes:north', npm: '', baseUrl: '',
    providerConfig: { npm: '', name: '', options: {}, models: {} },
    createdAt: '', updatedAt: '',
  };
  const desktopFavorite = {
    id: '3', providerId: 'claudedesktop:abc-123', npm: '', baseUrl: '',
    providerConfig: { npm: '', name: '', options: {}, models: {} },
    createdAt: '', updatedAt: '',
  };

  assert.equal(isFavoriteProviderForSource('dsh', dshFavorite), true);
  assert.equal(isFavoriteProviderForSource('dsh', hermesFavorite), false);
  assert.equal(isFavoriteProviderForSource('dsh', desktopFavorite), false);
  assert.equal(isFavoriteProviderForSource('hermes', hermesFavorite), true);
  assert.equal(isFavoriteProviderForSource('claudedesktop', desktopFavorite), true);
  assert.equal(
    isFavoriteProviderForSource('claudedesktop', {
      ...desktopFavorite, providerId: 'codex:abc-123',
    }),
    false,
  );
});

test('a dsh payload is preserved through buildFavoriteProviderOptions + getFavoriteProviderPayload', () => {
  const payload: DshFavoriteProviderPayload = {
    providerKey: 'openai',
    credential: { refName: 'OPENAI_API_KEY', value: 'sk-test' },
    modelsProvider: { api: 'openai-completions', baseURL: 'https://api.openai.com', apiKeyEnv: 'OPENAI_API_KEY' },
  };
  const envelope = buildFavoriteProviderOptions(
    { npm: '@ai-sdk/openai', name: 'openai', options: {}, models: {} },
    payload,
  );

  const favorite = {
    id: '1', providerId: 'dsh:openai', npm: '', baseUrl: '',
    providerConfig: envelope, createdAt: '', updatedAt: '',
  };
  const recovered = getFavoriteProviderPayload<DshFavoriteProviderPayload>(favorite);
  assert.equal(recovered?.providerKey, 'openai');
  assert.equal(recovered?.credential?.refName, 'OPENAI_API_KEY');
  assert.equal(recovered?.credential?.value, 'sk-test');
  assert.equal((recovered?.modelsProvider as Record<string, unknown>).api, 'openai-completions');
});

test('a hermes payload round-trips with its inline api_key kept intact', () => {
  const payload: HermesFavoriteProviderPayload = {
    providerKey: 'north',
    modelsProvider: { api_mode: 'anthropic', base_url: 'https://x.example', api_key: 'sk-h', models: [] },
  };
  const envelope = buildFavoriteProviderOptions(
    { npm: '@ai-sdk/anthropic', name: 'north', options: {}, models: {} },
    payload,
  );
  const favorite = {
    id: '1', providerId: 'hermes:north', npm: '', baseUrl: '',
    providerConfig: envelope, createdAt: '', updatedAt: '',
  };
  const recovered = getFavoriteProviderPayload<HermesFavoriteProviderPayload>(favorite);
  assert.equal(recovered?.providerKey, 'north');
  assert.equal((recovered?.modelsProvider as Record<string, unknown>).api_mode, 'anthropic');
  assert.equal((recovered?.modelsProvider as Record<string, unknown>).api_key, 'sk-h');
});

test('a claudedesktop payload round-trips through the OpenCode envelope', () => {
  const payload: ClaudeDesktopFavoriteProviderPayload = {
    name: 'My provider',
    category: 'custom',
    settingsConfig: '{"env":{"ANTHROPIC_BASE_URL":"https://x"}}',
    notes: 'imported earlier',
  };
  const envelope = buildFavoriteProviderOptions(
    { npm: '@ai-sdk/anthropic', name: payload.name, options: {}, models: {} },
    payload,
  );
  const favorite = {
    id: '1', providerId: 'claudedesktop:abc', npm: '', baseUrl: '',
    providerConfig: envelope, createdAt: '', updatedAt: '',
  };
  const recovered = getFavoriteProviderPayload<ClaudeDesktopFavoriteProviderPayload>(favorite);
  assert.equal(recovered?.name, 'My provider');
  assert.equal(recovered?.category, 'custom');
  assert.equal(recovered?.settingsConfig, '{"env":{"ANTHROPIC_BASE_URL":"https://x"}}');
  assert.equal(recovered?.notes, 'imported earlier');
});
