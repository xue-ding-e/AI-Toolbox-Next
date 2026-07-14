import type {
  GrokApiFormat,
  GrokCatalogModel,
  GrokProviderCategory,
  GrokSettingsConfig,
} from '../../../../types/grok';
import { normalizeGrokConfigForOfficialMode } from '../../../../utils/grokConfigUtils';
import { isJsonObject } from '../../../../utils/json';
import { normalizeGrokCatalogModels } from './grokCatalogModels';

export const DEFAULT_GROK_MODEL = 'grok-4.5';

export interface BuildGrokSettingsConfigInput {
  category: GrokProviderCategory;
  apiKey: string;
  baseUrl: string;
  model: string;
  apiFormat?: GrokApiFormat;
  /** When set, force every projected [model.*] to this backend-search flag. */
  supportsBackendSearch?: boolean;
  config: string;
  catalogModels: GrokCatalogModel[];
  auth: Record<string, unknown>;
}

export function parseGrokSettingsConfig(rawConfig: string | undefined): GrokSettingsConfig {
  if (!rawConfig?.trim()) return {};

  try {
    const parsedConfig = JSON.parse(rawConfig) as unknown;
    return isJsonObject(parsedConfig) ? parsedConfig as GrokSettingsConfig : {};
  } catch (error) {
    console.error('Failed to parse Grok settings config:', error);
    return {};
  }
}

export function buildGrokSettingsConfig({
  category,
  apiKey,
  baseUrl,
  model,
  apiFormat,
  supportsBackendSearch,
  config,
  catalogModels,
  auth,
}: BuildGrokSettingsConfigInput): string {
  const finalConfig = category === 'official'
    ? normalizeGrokConfigForOfficialMode(config)
    : config.trim();
  const normalizedApiKey = apiKey.trim();
  const normalizedBaseUrl = baseUrl.trim();
  // Official channels default to the current Grok default model when the form is left empty.
  // Custom channels also fall back so model mapping can be auto-created.
  const normalizedModel = model.trim() || DEFAULT_GROK_MODEL;
  // Form-level API format is the provider protocol source of truth. Model mapping
  // UI does not edit per-model apiBackend, so always project the selected format.
  // Keeping a previous "responses" value when the form is "chat" left live config
  // with api_backend = "responses" after apply.
  const apiBackend = apiFormat === 'openai_responses'
    ? 'responses'
    : apiFormat === 'anthropic_messages'
      ? 'messages'
      : 'chat_completions';
  const backendSearchFields = typeof supportsBackendSearch === 'boolean'
    ? { supportsBackendSearch }
    : {};
  let normalizedCatalogModels = normalizeGrokCatalogModels(catalogModels);

  if (category === 'custom') {
    normalizedCatalogModels = normalizedCatalogModels.map((catalogModel) => ({
      ...catalogModel,
      key: catalogModel.key?.trim() || catalogModel.model,
      ...(catalogModel.baseUrl?.trim() || !normalizedBaseUrl
        ? {}
        : { baseUrl: normalizedBaseUrl }),
      apiBackend,
      ...backendSearchFields,
    }));

    // If the user never filled model mapping, create a single default entry from
    // the selected/default model so save/apply can project [model.<key>].
    if (normalizedCatalogModels.length === 0) {
      normalizedCatalogModels = [{
        key: normalizedModel,
        model: normalizedModel,
        displayName: normalizedModel,
        ...(normalizedBaseUrl ? { baseUrl: normalizedBaseUrl } : {}),
        apiBackend,
        ...backendSearchFields,
      }];
    } else {
      const selectedModelExists = normalizedCatalogModels.some(
        (catalogModel) => catalogModel.key === normalizedModel || catalogModel.model === normalizedModel,
      );
      if (!selectedModelExists) {
        normalizedCatalogModels.push({
          key: normalizedModel,
          model: normalizedModel,
          displayName: normalizedModel,
          ...(normalizedBaseUrl ? { baseUrl: normalizedBaseUrl } : {}),
          apiBackend,
          ...backendSearchFields,
        });
      }
    }
  }

  const finalAuth = { ...auth };
  if (category === 'custom' && normalizedApiKey) {
    finalAuth.API_KEY = normalizedApiKey;
  } else {
    delete finalAuth.API_KEY;
  }

  const settingsConfig: GrokSettingsConfig = {
    auth: finalAuth,
    config: finalConfig.trim(),
    defaultModelKey: normalizedModel,
  };
  if (category === 'custom') {
    settingsConfig.modelCatalog = {
      models: normalizedCatalogModels,
    };
  }

  return JSON.stringify(settingsConfig);
}
