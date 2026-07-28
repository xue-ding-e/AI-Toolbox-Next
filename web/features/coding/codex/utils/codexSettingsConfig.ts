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
  autoReviewModelOverride?: string;
  auth: Record<string, unknown>;
}

export function normalizeCodexAutoReviewModelOverride(value: unknown): string | undefined {
  if (typeof value !== 'string') {
    return undefined;
  }
  const normalized = value.trim();
  return normalized || undefined;
}

/**
 * Resolve provider-level auto-review override.
 * Prefer top-level settingsConfig.autoReviewModelOverride; fall back to the
 * first non-empty legacy per-model value for short-lived local drafts.
 */
export function resolveCodexAutoReviewModelOverride(
  settings: CodexSettingsConfig | undefined,
): string | undefined {
  if (!settings) {
    return undefined;
  }

  const topLevel = normalizeCodexAutoReviewModelOverride(settings.autoReviewModelOverride);
  if (topLevel) {
    return topLevel;
  }

  const models = settings.modelCatalog?.models || [];
  for (const item of models) {
    const legacyItem = item as CodexCatalogModel & {
      autoReviewModelOverride?: unknown;
      auto_review_model_override?: unknown;
    };
    const fromCamel = normalizeCodexAutoReviewModelOverride(legacyItem.autoReviewModelOverride);
    if (fromCamel) {
      return fromCamel;
    }
    const fromSnake = normalizeCodexAutoReviewModelOverride(legacyItem.auto_review_model_override);
    if (fromSnake) {
      return fromSnake;
    }
  }

  return undefined;
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
  autoReviewModelOverride,
  auth,
}: BuildCodexSettingsConfigInput): string {
  let finalConfig = config;
  const normalizedApiKey = apiKey.trim();
  const normalizedCatalogModels = normalizeCodexCatalogModels(catalogModels);
  const normalizedAutoReviewModelOverride = normalizeCodexAutoReviewModelOverride(
    autoReviewModelOverride,
  );

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
  if (category === 'custom' && normalizedAutoReviewModelOverride) {
    settingsConfig.autoReviewModelOverride = normalizedAutoReviewModelOverride;
  }

  return JSON.stringify(settingsConfig);
}
