import assert from 'node:assert/strict';
import test from 'node:test';

import type { CodexProvider } from '../../../../../types/codex.ts';
import { saveCodexProviderCatalogWithGatewayReengage } from '../../../../../features/coding/codex/utils/codexProviderCatalogSave.ts';

const createProvider = (isApplied: boolean): CodexProvider => ({
  id: 'codex-provider',
  name: 'Codex Provider',
  category: 'custom',
  settingsConfig: JSON.stringify({
    auth: { OPENAI_API_KEY: 'sk-test' },
    config: 'model = "old-model"',
    modelCatalog: { models: [{ model: 'old-model' }] },
  }),
  isApplied,
  createdAt: '2026-07-22T00:00:00.000Z',
  updatedAt: '2026-07-22T00:00:00.000Z',
});

const nextSettingsConfig = JSON.stringify({
  auth: { OPENAI_API_KEY: 'sk-test' },
  config: 'model = "new-model"',
  modelCatalog: { models: [{ model: 'new-model', displayName: 'New Model' }] },
});

test('applied Codex catalog save restores direct before updating and reengages failover', async () => {
  const calls: string[] = [];
  let savedProvider: CodexProvider | undefined;

  const result = await saveCodexProviderCatalogWithGatewayReengage({
    provider: createProvider(true),
    settingsConfig: nextSettingsConfig,
    gatewayMode: 'failover',
    updateProvider: async (provider) => {
      calls.push('save');
      savedProvider = provider;
      return provider;
    },
    restoreDirect: async () => {
      calls.push('restore');
      return 'direct';
    },
    engageSingle: async () => {
      calls.push('single');
      return 'single';
    },
    engageFailover: async () => {
      calls.push('failover');
      return 'failover';
    },
    onGatewayStatusChange: (status) => {
      calls.push(`status:${status}`);
    },
  });

  assert.equal(result, savedProvider);
  assert.deepEqual(calls, ['restore', 'status:direct', 'save', 'single', 'failover', 'status:failover']);
  assert.equal(savedProvider?.settingsConfig, nextSettingsConfig);
});

test('applied Codex catalog save replays an aggregate takeover with its config', async () => {
  const calls: string[] = [];
  const aggregateConfig = { providerIds: ['a', 'b'], separator: '-' };

  await saveCodexProviderCatalogWithGatewayReengage({
    provider: createProvider(true),
    settingsConfig: nextSettingsConfig,
    gatewayMode: 'aggregate',
    aggregateConfig,
    updateProvider: async (provider) => {
      calls.push('save');
      return provider;
    },
    restoreDirect: async () => {
      calls.push('restore');
      return 'direct';
    },
    engageSingle: async () => {
      calls.push('single');
      return 'single';
    },
    engageFailover: async () => {
      calls.push('failover');
      return 'failover';
    },
    engageAggregate: async (config) => {
      calls.push(`aggregate:${config.providerIds.join(',')}`);
      return 'aggregate';
    },
    onGatewayStatusChange: (status) => {
      calls.push(`status:${status}`);
    },
  });

  assert.deepEqual(calls, ['restore', 'status:direct', 'save', 'aggregate:a,b', 'status:aggregate']);
});

test('unapplied Codex catalog save does not interrupt an active gateway takeover', async () => {
  const calls: string[] = [];

  await saveCodexProviderCatalogWithGatewayReengage({
    provider: createProvider(false),
    settingsConfig: nextSettingsConfig,
    gatewayMode: 'aggregate',
    aggregateConfig: { providerIds: ['a'], separator: '-' },
    updateProvider: async (provider) => {
      calls.push('save');
      return provider;
    },
    restoreDirect: async () => {
      calls.push('restore');
      return 'direct';
    },
    engageSingle: async () => {
      calls.push('single');
      return 'single';
    },
    engageFailover: async () => {
      calls.push('failover');
      return 'failover';
    },
    engageAggregate: async () => {
      calls.push('aggregate');
      return 'aggregate';
    },
  });

  assert.deepEqual(calls, ['save']);
});

test('applied Codex catalog save writes directly when gateway mode is inactive', async () => {
  const calls: string[] = [];

  await saveCodexProviderCatalogWithGatewayReengage({
    provider: createProvider(true),
    settingsConfig: nextSettingsConfig,
    gatewayMode: null,
    updateProvider: async (provider) => {
      calls.push('save');
      return provider;
    },
    restoreDirect: async () => {
      calls.push('restore');
      return 'direct';
    },
    engageSingle: async () => {
      calls.push('single');
      return 'single';
    },
    engageFailover: async () => {
      calls.push('failover');
      return 'failover';
    },
  });

  assert.deepEqual(calls, ['save']);
});
