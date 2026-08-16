import type { OpenCodeProvider } from '@/types/opencode';
import type { HermesRuntimeProviderView } from '@/types/hermes';

export const asRecord = (value: unknown): Record<string, unknown> => (
  value && typeof value === 'object' && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {}
);

export const getStringField = (value: Record<string, unknown>, key: string): string => {
  const fieldValue = value[key];
  return typeof fieldValue === 'string' ? fieldValue : '';
};

export const getNumberField = (value: Record<string, unknown>, key: string): number | undefined => {
  const fieldValue = value[key];
  return typeof fieldValue === 'number' && Number.isFinite(fieldValue) ? fieldValue : undefined;
};

export const isRecordEmpty = (value: Record<string, unknown>): boolean => Object.keys(value).length === 0;

export const setOptionalStringField = (
  target: Record<string, unknown>,
  key: string,
  value: unknown,
) => {
  if (typeof value === 'string' && value.trim()) {
    target[key] = value.trim();
  } else {
    delete target[key];
  }
};

/** Mask an api_key for display (mirrors pi's credential masking). */
export const maskCredential = (credential: unknown): string => {
  if (!credential || typeof credential !== 'string') {
    return '';
  }
  const key = credential.trim();
  if (key === '' || key.startsWith('$') || key.startsWith('!')) {
    return key;
  }
  if (key.length <= 10) {
    return '********';
  }
  return `${key.slice(0, 4)}...${key.slice(-4)}`;
};

/**
 * Extract a provider's `models` as an ordered array of `{ id, model }`.
 * The backend denormalizes the YAML dict into an array with `id` re-injected.
 */
export const getProviderModelRecords = (
  providerConfig: Record<string, unknown> | undefined,
): Array<{ id: string; model: Record<string, unknown> }> => {
  if (!providerConfig) {
    return [];
  }
  const models = providerConfig.models;
  if (!Array.isArray(models)) {
    return [];
  }
  return models
    .map((model) => {
      if (typeof model === 'string') {
        return { id: model, model: { id: model } };
      }
      if (model && typeof model === 'object' && typeof (model as Record<string, unknown>).id === 'string') {
        return {
          id: (model as Record<string, string>).id,
          model: model as Record<string, unknown>,
        };
      }
      return null;
    })
    .filter((entry): entry is { id: string; model: Record<string, unknown> } => !!entry);
};

/**
 * Map a Hermes provider `api_mode` to the preset SDK group used by connectivity
 * tests. Unknown modes default to the OpenAI-compatible group.
 */
export const hermesApiModeToSdkName = (apiMode?: string): string => {
  const mode = apiMode?.trim().toLowerCase() ?? '';
  if (mode.includes('anthropic')) {
    return '@ai-sdk/anthropic';
  }
  if (mode.includes('google') || mode.includes('gemini')) {
    return '@ai-sdk/google';
  }
  return '@ai-sdk/openai-compatible';
};

/** Hermes 官方思考等级取值(agent.reasoning_effort / reasoning_overrides)。 */
export const HERMES_REASONING_LEVELS = [
  'none',
  'minimal',
  'low',
  'medium',
  'high',
  'xhigh',
  'max',
  'ultra',
] as const;

export type HermesReasoningLevel = (typeof HERMES_REASONING_LEVELS)[number];

/** 规范化思考等级输入;非字符串 / 空白 / 不在枚举内返回 undefined。 */
export const parseReasoningEffort = (value: unknown): HermesReasoningLevel | undefined => {
  if (typeof value !== 'string') {
    return undefined;
  }
  const trimmed = value.trim();
  return (HERMES_REASONING_LEVELS as readonly string[]).includes(trimmed)
    ? (trimmed as HermesReasoningLevel)
    : undefined;
};

/** Build an OpenCodeProvider-ish view used by the shared connectivity test. */
export const buildHermesConnectivityProvider = (
  provider: HermesRuntimeProviderView,
): OpenCodeProvider => {
  const providerConfig = asRecord(provider.provider);
  const apiKey = getStringField(providerConfig, 'api_key') || getStringField(providerConfig, 'apiKey');
  const baseUrl = getStringField(providerConfig, 'base_url') || getStringField(providerConfig, 'baseUrl');
  const models = Object.fromEntries((provider.modelIds ?? []).map((id) => [id, {}]));

  return {
    npm: hermesApiModeToSdkName(provider.apiMode || getStringField(providerConfig, 'api_mode')),
    name: provider.displayName,
    options: {
      ...(baseUrl ? { baseURL: baseUrl } : {}),
      ...(apiKey ? { apiKey } : {}),
    },
    models,
  };
};
