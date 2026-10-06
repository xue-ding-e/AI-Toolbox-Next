/**
 * OmO Native 的 `models.json` provider 与共享组件之间的适配层。
 *
 * `models.json` **没有对外发布的 schema**，字段形状反推自上游
 * `packages/omo-native/bin/lib/setup-opencode-providers.js` 的 `convertProvider`：
 * provider 级 `name` / `baseUrl` / `api` / `headers` / `models[]`，模型级 `id` /
 * `name` / `reasoning` / `input` / `contextWindow` / `maxTokens` / `thinking`。
 *
 * 共享的 ProviderCard / FetchModelsModal / 连通性测试都按 OpenCode 的 provider 形状
 * （`{ npm, name, options: { baseURL, apiKey, headers }, models }`）工作，所以这里做一次
 * 单向映射；写入仍走后端按 key 局部更新，未知字段原样保留。
 */

import type { ProviderDisplayData } from '@/components/common/ProviderCard/types';
// The modal owns the richer `ProviderConnectivityInfo` (it needs a full
// `OpenCodeProvider`); the batch-target module declares a narrower one. Build
// the modal's shape so the value satisfies both.
import type { ProviderConnectivityInfo } from '@/features/coding/shared/providerConnectivity/ProviderConnectivityTestModal';
import type { OpenCodeProvider } from '@/types/opencode';
import type { OmoNativeProvider } from '@/types/omoNative';

/** Native 支持的 `api` 值，来自上游 provider 适配器表。 */
export const OMO_NATIVE_API_OPTIONS = [
  { value: 'openai-completions', label: 'OpenAI Completions' },
  { value: 'openai-responses', label: 'OpenAI Responses' },
  { value: 'anthropic-messages', label: 'Anthropic Messages' },
  { value: 'google-generative-ai', label: 'Google Generative AI' },
];

/** Native 的 `api` → 共享组件认识的 npm 包名。 */
const NPM_BY_OMO_NATIVE_API: Record<string, string> = {
  'openai-completions': '@ai-sdk/openai-compatible',
  'openai-responses': '@ai-sdk/openai',
  'anthropic-messages': '@ai-sdk/anthropic',
  'google-generative-ai': '@ai-sdk/google',
};

export const omoNativeApiToNpm = (api: string): string =>
  NPM_BY_OMO_NATIVE_API[api] ?? '@ai-sdk/openai-compatible';

export const asRecord = (value: unknown): Record<string, unknown> =>
  value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};

/** headers 只保留字符串值：共享组件按 `Record<string, string>` 约定消费。 */
export const asStringRecord = (value: unknown): Record<string, string> => {
  const record = asRecord(value);
  return Object.fromEntries(
    Object.entries(record).filter((entry): entry is [string, string] => typeof entry[1] === 'string'),
  );
};

export const getStringField = (record: Record<string, unknown>, key: string): string => {
  const value = record[key];
  return typeof value === 'string' ? value : '';
};

export const getNumberField = (
  record: Record<string, unknown>,
  key: string,
): number | undefined => {
  const value = record[key];
  return typeof value === 'number' ? value : undefined;
};

export const isRecordEmpty = (record: Record<string, unknown>): boolean =>
  Object.keys(record).length === 0;

export interface OmoNativeModelEntry {
  id: string;
  model: Record<string, unknown>;
}

/**
 * 读 `models.json` 里某个 provider 的模型条目。
 *
 * 上游把模型存成**数组**（每条自带 `id`），不是对象 map——所以这里按数组读，
 * 并跳过没有 `id` 的条目（引擎也会忽略它们）。
 */
export const getOmoNativeModelEntries = (
  providerConfig: Record<string, unknown>,
): OmoNativeModelEntry[] => {
  const models = providerConfig.models;
  if (!Array.isArray(models)) return [];
  return models
    .map((entry) => {
      const record = asRecord(entry);
      const id = getStringField(record, 'id');
      return id ? { id, model: record } : null;
    })
    .filter((entry): entry is OmoNativeModelEntry => entry !== null);
};

export const omoNativeProviderToDisplayData = (
  provider: OmoNativeProvider,
  options: { fallbackBaseUrl: string },
): ProviderDisplayData => ({
  id: provider.key,
  name: getStringField(provider.config, 'name') || provider.key,
  sdkName: getStringField(provider.config, 'api') || 'models.json',
  baseUrl: getStringField(provider.config, 'baseUrl') || options.fallbackBaseUrl,
});

/** Native 的 provider 配置 → 共享连通性测试/卡片组件认识的 OpenCode provider 形状。 */
export const omoNativeProviderToOpenCodeProvider = (
  provider: OmoNativeProvider,
): OpenCodeProvider => {
  const config = provider.config;
  const models = Object.fromEntries(
    getOmoNativeModelEntries(config).map((entry) => [
      entry.id,
      { ...entry.model, id: undefined, name: getStringField(entry.model, 'name') || entry.id },
    ]),
  );
  const headers = asStringRecord(config.headers);
  return {
    npm: omoNativeApiToNpm(getStringField(config, 'api')),
    name: getStringField(config, 'name') || provider.key,
    options: {
      baseURL: getStringField(config, 'baseUrl'),
      ...(isRecordEmpty(headers) ? {} : { headers }),
    },
    models,
  };
};

export const omoNativeProviderToConnectivityInfo = (
  provider: OmoNativeProvider,
): ProviderConnectivityInfo => ({
  providerId: provider.key,
  providerName: getStringField(provider.config, 'name') || provider.key,
  providerConfig: omoNativeProviderToOpenCodeProvider(provider),
  configValueMode: 'omp',
  modelIds: getOmoNativeModelEntries(provider.config).map((entry) => entry.id),
});
