import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  parseKimiSettingsConfig,
  buildKimiSettingsConfig,
  extractKimiBaseUrl,
  extractKimiDefaultModel,
  normalizeKimiCatalogModels,
} from '../../../../../features/coding/kimi/utils/settingsConfig.ts';
import type { KimiCatalogModel } from '../../../../../types/kimi.ts';

describe('kimi settingsConfig utils', () => {
  describe('parseKimiSettingsConfig', () => {
    it('handles empty or null string gracefully', () => {
      const parsed1 = parseKimiSettingsConfig('');
      assert.equal(parsed1.apiKey, '');
      assert.equal(parsed1.baseUrl, '');
      assert.equal(parsed1.providerKey, 'custom');
      assert.equal(parsed1.defaultModelKey, '');
      assert.deepEqual(parsed1.catalogModels, []);
      assert.equal(parsed1.parseError, undefined);

      const parsed2 = parseKimiSettingsConfig(null);
      assert.equal(parsed2.apiKey, '');
      assert.equal(parsed2.baseUrl, '');
      assert.equal(parsed2.providerKey, 'custom');
      assert.deepEqual(parsed2.catalogModels, []);
      assert.equal(parsed2.parseError, undefined);
    });

    it('handles invalid JSON string gracefully with parseError', () => {
      const parsed = parseKimiSettingsConfig('{ invalid json');
      assert.equal(parsed.apiKey, '');
      assert.equal(parsed.baseUrl, '');
      assert.equal(parsed.providerKey, 'custom');
      assert.equal(parsed.rawJson, '{ invalid json');
      assert.ok(parsed.parseError);
    });

    it('correctly parses custom provider JSON with models and extra fields', () => {
      const json = JSON.stringify({
        auth: {
          API_KEY: 'sk-test-123',
          extra_auth_field: 'keep-me',
        },
        defaultModelKey: 'kimi-code/k3',
        providerConfigs: {
          my_provider: {
            type: 'openai',
            base_url: 'https://api.example.com/v1',
            custom_header: 'preserved',
          },
        },
        modelCatalog: {
          models: [
            {
              key: 'kimi-code/k3',
              model: 'k3',
              provider: 'my_provider',
              displayName: 'K3 Moonshot',
              maxContextSize: 200000,
            },
          ],
          custom_catalog_meta: 'preserved',
        },
        config: '# custom toml\n[kimi]\ntimeout = 30',
        unknownRootField: { foo: 'bar' },
      });

      const parsed = parseKimiSettingsConfig(json);
      assert.equal(parsed.apiKey, 'sk-test-123');
      assert.equal(parsed.baseUrl, 'https://api.example.com/v1');
      assert.equal(parsed.providerKey, 'my_provider');
      assert.equal(parsed.defaultModelKey, 'kimi-code/k3');
      assert.equal(parsed.customTomlConfig, '# custom toml\n[kimi]\ntimeout = 30');
      assert.equal(parsed.catalogModels.length, 1);
      assert.equal(parsed.catalogModels[0].displayName, 'K3 Moonshot');
      assert.equal(parsed.catalogModels[0].maxContextSize, 200000);
      assert.equal(parsed.parseError, undefined);
      assert.deepEqual(parsed.rawObject.unknownRootField, { foo: 'bar' });
    });
  });

  describe('extractKimiBaseUrl and extractKimiDefaultModel', () => {
    it('extracts baseUrl and defaultModelKey from valid JSON', () => {
      const json = JSON.stringify({
        defaultModelKey: 'kimi-code/k3',
        providerConfigs: {
          custom: {
            type: 'openai',
            base_url: 'https://api.moonshot.cn/v1',
          },
        },
      });

      assert.equal(extractKimiBaseUrl(json), 'https://api.moonshot.cn/v1');
      assert.equal(extractKimiDefaultModel(json), 'kimi-code/k3');
    });

    it('returns undefined when missing or invalid', () => {
      assert.equal(extractKimiBaseUrl(''), undefined);
      assert.equal(extractKimiDefaultModel(''), undefined);
      assert.equal(extractKimiBaseUrl('{ invalid'), undefined);
      assert.equal(extractKimiDefaultModel('{ invalid'), undefined);
    });
  });

  describe('normalizeKimiCatalogModels', () => {
    it('normalizes catalog models and fills defaults', () => {
      const input: KimiCatalogModel[] = [
        {
          key: 'kimi-code/k3',
          model: 'k3',
          provider: '',
          displayName: 'K3',
        },
        {
          key: 'only-key',
          model: '',
          provider: 'custom',
        },
        {
          key: '',
          model: 'only-model',
          provider: '',
        },
      ];

      const normalized = normalizeKimiCatalogModels(input, 'fallback-provider');
      assert.equal(normalized.length, 3);
      assert.equal(normalized[0].provider, 'fallback-provider');
      assert.equal(normalized[1].model, 'only-key');
      assert.equal(normalized[2].key, 'only-model');
      assert.equal(normalized[2].provider, 'fallback-provider');
    });
  });

  describe('buildKimiSettingsConfig and round-trip unknown fields preservation', () => {
    it('preserves unknown top-level and nested fields in custom provider', () => {
      const initialJson = JSON.stringify({
        auth: {
          API_KEY: 'sk-old',
          another_key: 'preserve-this',
        },
        providerConfigs: {
          my_provider: {
            type: 'openai',
            base_url: 'https://old.url',
            extra_param: 123,
          },
        },
        modelCatalog: {
          models: [],
          custom_meta: { key: 'val' },
        },
        custom_top_level_field: 'do_not_lose_me',
      });

      const parsed = parseKimiSettingsConfig(initialJson);

      const updatedModels: KimiCatalogModel[] = [
        {
          key: 'kimi-code/k3',
          model: 'k3',
          provider: 'my_provider',
          displayName: 'K3 Updated',
        },
      ];

      const builtJson = buildKimiSettingsConfig({
        category: 'custom',
        apiKey: 'sk-new',
        baseUrl: 'https://new.url',
        providerKey: parsed.providerKey,
        defaultModelKey: 'kimi-code/k3',
        catalogModels: updatedModels,
        customTomlConfig: '',
        rawObject: parsed.rawObject,
      });

      const reparsed = JSON.parse(builtJson);
      assert.equal(reparsed.auth.API_KEY, 'sk-new');
      assert.equal(reparsed.auth.another_key, 'preserve-this');
      assert.equal(reparsed.providerConfigs.my_provider.base_url, 'https://new.url');
      assert.equal(reparsed.providerConfigs.my_provider.extra_param, 123);
      assert.equal(reparsed.custom_top_level_field, 'do_not_lose_me');
      assert.equal(reparsed.defaultModelKey, 'kimi-code/k3');
      assert.equal(reparsed.modelCatalog.models.length, 1);
      assert.equal(reparsed.modelCatalog.custom_meta.key, 'val');
    });

    it('builds official provider structure correctly (empty providerConfigs, no API_KEY, no modelCatalog)', () => {
      const builtJson = buildKimiSettingsConfig({
        category: 'official',
        apiKey: 'sk-should-be-ignored',
        baseUrl: 'https://should-be-ignored',
        defaultModelKey: 'kimi-code/k3',
        customTomlConfig: '# toml',
      });

      const parsed = JSON.parse(builtJson);
      assert.equal(parsed.auth, undefined);
      assert.deepEqual(parsed.providerConfigs, {});
      assert.equal(parsed.modelCatalog, undefined);
      assert.equal(parsed.defaultModelKey, 'kimi-code/k3');
      assert.equal(parsed.config, '# toml');
    });
  });
});
