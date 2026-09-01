import type {
  KimiCatalogModel,
  KimiProviderCategory,
  KimiProviderConfig,
  KimiSettingsConfig,
} from '@/types/kimi';

export const CUSTOM_KIMI_PROVIDER_KEY = 'custom';

/**
 * Official channel default model, matching what the real Kimi CLI projects:
 * catalog key `kimi-code/kimi-for-coding` -> model id `kimi-for-coding`.
 */
export const KIMI_OFFICIAL_DEFAULT_MODEL_KEY = 'kimi-code/kimi-for-coding';
export const KIMI_OFFICIAL_DEFAULT_MODEL_ID = 'kimi-for-coding';
export const KIMI_OFFICIAL_DEFAULT_MODEL_DISPLAY_NAME = 'K2.7 Coding';
export const KIMI_OFFICIAL_DEFAULT_MODEL_MAX_CONTEXT_SIZE = 262144;
/** Official API base URL, mirroring the backend `KIMI_OFFICIAL_API_BASE_URL`. */
export const KIMI_OFFICIAL_API_BASE_URL = 'https://api.kimi.com/coding/v1';

export interface ParsedKimiSettings {
  apiKey: string;
  baseUrl: string;
  providerKey: string;
  defaultModelKey: string;
  catalogModels: KimiCatalogModel[];
  customTomlConfig: string;
  rawJson: string;
  rawObject: Record<string, unknown>;
  parseError?: string;
}

export function parseKimiSettingsConfig(rawConfig?: string | null): ParsedKimiSettings {
  const trimmed = rawConfig?.trim() || '';
  if (!trimmed) {
    return {
      apiKey: '',
      baseUrl: '',
      providerKey: CUSTOM_KIMI_PROVIDER_KEY,
      defaultModelKey: '',
      catalogModels: [],
      customTomlConfig: '',
      rawJson: '',
      rawObject: {},
    };
  }

  let rawObject: Record<string, unknown> = {};
  try {
    const parsed = JSON.parse(trimmed);
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      rawObject = parsed as Record<string, unknown>;
    } else {
      return {
        apiKey: '',
        baseUrl: '',
        providerKey: CUSTOM_KIMI_PROVIDER_KEY,
        defaultModelKey: '',
        catalogModels: [],
        customTomlConfig: '',
        rawJson: trimmed,
        rawObject: {},
        parseError: 'Root JSON value must be an object',
      };
    }
  } catch (err) {
    return {
      apiKey: '',
      baseUrl: '',
      providerKey: CUSTOM_KIMI_PROVIDER_KEY,
      defaultModelKey: '',
      catalogModels: [],
      customTomlConfig: '',
      rawJson: trimmed,
      rawObject: {},
      parseError: err instanceof Error ? err.message : String(err),
    };
  }

  const auth = rawObject.auth && typeof rawObject.auth === 'object' && !Array.isArray(rawObject.auth)
    ? (rawObject.auth as Record<string, unknown>)
    : {};
  const apiKey = typeof auth.API_KEY === 'string' ? auth.API_KEY.trim() : '';

  const providerConfigs = rawObject.providerConfigs
    && typeof rawObject.providerConfigs === 'object'
    && !Array.isArray(rawObject.providerConfigs)
    ? (rawObject.providerConfigs as Record<string, Record<string, unknown>>)
    : {};

  let providerKey = CUSTOM_KIMI_PROVIDER_KEY;
  let baseUrl = '';

  const providerConfigEntries = Object.entries(providerConfigs);
  if (providerConfigEntries.length > 0) {
    const [firstKey, firstVal] = providerConfigEntries[0];
    if (firstKey) {
      providerKey = firstKey;
    }
    if (firstVal && typeof firstVal === 'object' && !Array.isArray(firstVal)) {
      if (typeof firstVal.base_url === 'string') {
        baseUrl = firstVal.base_url.trim();
      }
    }
  }

  const defaultModelKey = typeof rawObject.defaultModelKey === 'string'
    ? rawObject.defaultModelKey.trim()
    : '';

  const modelCatalog = rawObject.modelCatalog
    && typeof rawObject.modelCatalog === 'object'
    && !Array.isArray(rawObject.modelCatalog)
    ? (rawObject.modelCatalog as Record<string, unknown>)
    : {};

  const rawModels = Array.isArray(modelCatalog.models) ? modelCatalog.models : [];
  const catalogModels: KimiCatalogModel[] = rawModels
    .filter((item): item is Record<string, unknown> => Boolean(item && typeof item === 'object' && !Array.isArray(item)))
    .map((item) => {
      const key = typeof item.key === 'string' ? item.key.trim() : '';
      const model = typeof item.model === 'string' ? item.model.trim() : '';
      const provider = typeof item.provider === 'string' ? item.provider.trim() : providerKey;
      const displayName = typeof item.displayName === 'string' ? item.displayName.trim() : undefined;
      const maxContextSize = typeof item.maxContextSize === 'number' ? item.maxContextSize : undefined;
      const capabilities = Array.isArray(item.capabilities)
        ? (item.capabilities.filter((c): c is string => typeof c === 'string') as string[])
        : undefined;
      const supportEfforts = Array.isArray(item.supportEfforts)
        ? (item.supportEfforts.filter((e): e is string => typeof e === 'string') as string[])
        : undefined;
      const defaultEffort = typeof item.defaultEffort === 'string' ? item.defaultEffort : undefined;

      const catalogModel: KimiCatalogModel = {
        key: key || model,
        model,
        provider: provider || providerKey,
      };
      if (displayName) catalogModel.displayName = displayName;
      if (maxContextSize !== undefined) catalogModel.maxContextSize = maxContextSize;
      if (capabilities && capabilities.length > 0) catalogModel.capabilities = capabilities;
      if (supportEfforts && supportEfforts.length > 0) catalogModel.supportEfforts = supportEfforts;
      if (defaultEffort) catalogModel.defaultEffort = defaultEffort;

      return catalogModel;
    })
    .filter((item) => item.key || item.model);

  const customTomlConfig = typeof rawObject.config === 'string' ? rawObject.config : '';

  return {
    apiKey,
    baseUrl,
    providerKey,
    defaultModelKey,
    catalogModels,
    customTomlConfig,
    rawJson: JSON.stringify(rawObject, null, 2),
    rawObject,
  };
}

export function extractKimiBaseUrl(rawConfig?: string | null): string | undefined {
  const parsed = parseKimiSettingsConfig(rawConfig);
  return parsed.baseUrl || undefined;
}

export function extractKimiDefaultModel(rawConfig?: string | null): string | undefined {
  const parsed = parseKimiSettingsConfig(rawConfig);
  return parsed.defaultModelKey || undefined;
}

export function normalizeKimiCatalogModels(
  models: KimiCatalogModel[],
  fallbackProvider: string = CUSTOM_KIMI_PROVIDER_KEY,
): KimiCatalogModel[] {
  return models
    .map((m) => {
      const model = m.model?.trim() || '';
      const key = m.key?.trim() || model;
      const provider = m.provider?.trim() || fallbackProvider;
      const displayName = m.displayName?.trim() || undefined;

      if (!key && !model) return null;

      const normalized: KimiCatalogModel = {
        key: key || model,
        model: model || key,
        provider: provider || fallbackProvider,
      };
      if (displayName) normalized.displayName = displayName;
      if (m.maxContextSize !== undefined) normalized.maxContextSize = m.maxContextSize;
      if (m.capabilities && m.capabilities.length > 0) normalized.capabilities = m.capabilities;
      if (m.supportEfforts && m.supportEfforts.length > 0) normalized.supportEfforts = m.supportEfforts;
      if (m.defaultEffort) normalized.defaultEffort = m.defaultEffort;

      return normalized;
    })
    .filter((m): m is KimiCatalogModel => m !== null);
}

export interface BuildKimiSettingsConfigParams {
  category: KimiProviderCategory;
  apiKey?: string;
  baseUrl?: string;
  providerKey?: string;
  defaultModelKey?: string;
  catalogModels?: KimiCatalogModel[];
  customTomlConfig?: string;
  rawObject?: Record<string, unknown>;
}

export function buildKimiSettingsConfig(params: BuildKimiSettingsConfigParams): string {
  const {
    category,
    apiKey = '',
    baseUrl = '',
    providerKey = CUSTOM_KIMI_PROVIDER_KEY,
    defaultModelKey = '',
    catalogModels = [],
    customTomlConfig = '',
    rawObject = {},
  } = params;

  // Deep clone or copy rawObject to avoid mutating the input
  const result: Record<string, unknown> = { ...rawObject };

  if (category === 'official') {
    // Official provider credentials come from OAuth credentials.
    // ProviderConfigs is empty.
    const existingAuth = result.auth && typeof result.auth === 'object' && !Array.isArray(result.auth)
      ? { ...(result.auth as Record<string, unknown>) }
      : {};
    delete existingAuth.API_KEY;
    if (Object.keys(existingAuth).length > 0) {
      result.auth = existingAuth;
    } else {
      delete result.auth;
    }

    result.providerConfigs = {};

    const trimmedDefaultModel = defaultModelKey.trim();
    if (trimmedDefaultModel) {
      result.defaultModelKey = trimmedDefaultModel;
    } else {
      delete result.defaultModelKey;
    }

    delete result.modelCatalog;

    if (customTomlConfig.trim()) {
      result.config = customTomlConfig.trim();
    } else {
      delete result.config;
    }

    return JSON.stringify(result, null, 2);
  }

  // Custom provider
  // 1. auth
  const trimmedApiKey = apiKey.trim();
  const existingAuth = result.auth && typeof result.auth === 'object' && !Array.isArray(result.auth)
    ? { ...(result.auth as Record<string, unknown>) }
    : {};
  if (trimmedApiKey) {
    existingAuth.API_KEY = trimmedApiKey;
    result.auth = existingAuth;
  } else {
    delete existingAuth.API_KEY;
    if (Object.keys(existingAuth).length > 0) {
      result.auth = existingAuth;
    } else {
      delete result.auth;
    }
  }

  // 2. providerConfigs
  const activeProviderKey = providerKey.trim() || CUSTOM_KIMI_PROVIDER_KEY;
  const existingProviderConfigs = result.providerConfigs
    && typeof result.providerConfigs === 'object'
    && !Array.isArray(result.providerConfigs)
    ? { ...(result.providerConfigs as Record<string, KimiProviderConfig>) }
    : {};

  const existingCurrentConfig = existingProviderConfigs[activeProviderKey] || {};
  const updatedCurrentConfig: KimiProviderConfig = {
    type: 'openai',
    ...existingCurrentConfig,
    base_url: baseUrl.trim(),
  };

  // If there were other keys, decide whether to preserve or keep just the active one
  // In Kimi, custom provider typically has 1 providerConfig entry
  if (Object.keys(existingProviderConfigs).length <= 1) {
    result.providerConfigs = {
      [activeProviderKey]: updatedCurrentConfig,
    };
  } else {
    existingProviderConfigs[activeProviderKey] = updatedCurrentConfig;
    result.providerConfigs = existingProviderConfigs;
  }

  // 3. modelCatalog
  const normalizedModels = normalizeKimiCatalogModels(catalogModels, activeProviderKey);
  const existingModelCatalog = result.modelCatalog
    && typeof result.modelCatalog === 'object'
    && !Array.isArray(result.modelCatalog)
    ? { ...(result.modelCatalog as Record<string, unknown>) }
    : {};

  if (normalizedModels.length > 0) {
    result.modelCatalog = {
      ...existingModelCatalog,
      models: normalizedModels,
    };
  } else {
    delete result.modelCatalog;
  }

  // 4. defaultModelKey
  const trimmedDefaultModel = defaultModelKey.trim();
  if (trimmedDefaultModel) {
    result.defaultModelKey = trimmedDefaultModel;
  } else if (normalizedModels.length > 0) {
    result.defaultModelKey = normalizedModels[0].key;
  } else {
    delete result.defaultModelKey;
  }

  // 5. customTomlConfig
  if (customTomlConfig.trim()) {
    result.config = customTomlConfig.trim();
  } else {
    delete result.config;
  }

  // Ensure KimiSettingsConfig satisfies type (cast check)
  const _typeCheck: KimiSettingsConfig = result as unknown as KimiSettingsConfig;
  void _typeCheck;

  return JSON.stringify(result, null, 2);
}
