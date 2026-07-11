import type {
  CodexCatalogModel,
  CodexProviderCategory,
  CodexSettingsConfig,
} from '../../../../types/codex';
import {
  normalizeCodexConfigForOfficialMode,
  removeCodexBaseUrl,
  removeCodexModel,
  setCodexBaseUrl,
  setCodexModel,
} from '../../../../utils/codexConfigUtils';
import { isJsonObject } from '../../../../utils/json';
import { normalizeCodexCatalogModels } from './codexCatalogModels';

export interface BuildCodexSettingsConfigInput {
  category: CodexProviderCategory;
  apiKey: string;
  baseUrl: string;
  model: string;
  config: string;
  catalogModels: CodexCatalogModel[];
  auth: Record<string, unknown>;
}

export function parseCodexSettingsConfig(rawConfig: string | undefined): CodexSettingsConfig {
  if (!rawConfig?.trim()) return {};

  try {
    const parsedConfig = JSON.parse(rawConfig) as unknown;
    return isJsonObject(parsedConfig) ? parsedConfig as CodexSettingsConfig : {};
  } catch (error) {
    console.error('Failed to parse Codex settings config:', error);
    return {};
  }
}

export function buildCodexSettingsConfig({
  category,
  apiKey,
  baseUrl,
  model,
  config,
  catalogModels,
  auth,
}: BuildCodexSettingsConfigInput): string {
  let finalConfig = config;
  const normalizedApiKey = apiKey.trim();
  const normalizedCatalogModels = normalizeCodexCatalogModels(catalogModels);

  if (category === 'custom') {
    finalConfig = baseUrl
      ? setCodexBaseUrl(finalConfig, baseUrl)
      : removeCodexBaseUrl(finalConfig);
  } else {
    finalConfig = normalizeCodexConfigForOfficialMode(finalConfig);
  }
  finalConfig = model
    ? setCodexModel(finalConfig, model)
    : removeCodexModel(finalConfig);

  const finalAuth = { ...auth };
  if (category === 'custom' && normalizedApiKey) {
    finalAuth.OPENAI_API_KEY = normalizedApiKey;
  } else {
    delete finalAuth.OPENAI_API_KEY;
  }

  const settingsConfig: CodexSettingsConfig = {
    auth: finalAuth,
    config: finalConfig.trim(),
  };
  if (category === 'custom' && normalizedCatalogModels.length > 0) {
    settingsConfig.modelCatalog = {
      models: normalizedCatalogModels,
    };
  }

  return JSON.stringify(settingsConfig);
}
