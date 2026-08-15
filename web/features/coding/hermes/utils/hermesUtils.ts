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

/** 把模型的 `reasoningEfforts`(思考等级映射)转成 JsonEditor 可编辑的 JSON 字符串。 */
export const stringifyReasoningEfforts = (value: unknown): string | undefined => {
  const record = asRecord(value);
  if (Object.keys(record).length === 0) {
    return undefined;
  }
  return JSON.stringify(record, null, 2);
};

/**
 * 解析思考等级的 JsonEditor 值。仅保留"有非空字符串值"的条目(丢弃 null / 空值 /
 * 数字等),空映射或非法 JSON 返回 undefined(表示不写入字段)。
 */
export const parseThinkingLevelEfforts = (json: string): Record<string, string> | undefined => {
  const trimmed = (json || '').trim();
  if (!trimmed) {
    return undefined;
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch {
    return undefined;
  }
  const record = asRecord(parsed);
  const entries = Object.entries(record)
    .filter(([, value]) => typeof value === 'string' && value.trim() !== '')
    .map(([key, value]) => [key, (value as string).trim()] as const);
  if (entries.length === 0) {
    return undefined;
  }
  return Object.fromEntries(entries);
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
