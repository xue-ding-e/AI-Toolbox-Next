/**
 * OmO Native 的 `models.json` provider 读取层。
 *
 * `models.json` **没有对外发布的 schema**，字段形状反推自上游
 * `packages/omo-native/bin/lib/setup-opencode-providers.js` 的 `convertProvider`：
 * provider 级 `name` / `baseUrl` / `api` / `headers` / `models[]`，模型级 `id` /
 * `name` / `reasoning` / `input` / `contextWindow` / `maxTokens` / `thinking`。
 *
 * 这里只做**取值与归一化**（按数组读模型、headers 只留字符串、api ↔ npm 换算、
 * 弹窗 JSON 字符串 ↔ 文件数组/对象的互转），不做展示形状映射——卡片走
 * `OmoNativeProviderCard`，连通性请求在页面 handler 里组装。
 * 写入仍走后端按 provider key 局部更新，未知字段原样保留。
 *
 * ⚠️ **OmO Native 与 Pi 同用 senpi 引擎、同读 `models.json`**（OMP 读的是
 * `models.yml`，字段形状不同）。模型字段的权威定义见上游
 * `<runtime>/docs/models.md`，其中：
 * - `input` 是**数组**（`["text", "image"]`），不是字符串；
 * - 思考级别用 `thinkingLevelMap`（三态：省略/字符串/`null`），**没有** OMP 的
 *   `thinking: { efforts, defaultLevel }` 结构。
 */

// 相对路径（不用 `@/` 别名）：本模块有 node:test 单测，而测试用的 loader
// 只解析相对 specifier。同族的 `pi/utils/piFetchedModels.ts` 出于同一原因也这么写。
import type { FetchedModel } from '../../../../components/common/FetchModelsModal/types.ts';
import type { PresetModel } from '../../../../constants/presetModels.ts';
import {
  PI_INPUT_TYPES,
  buildPiThinkingLevelMapFromPreset,
} from '../../../../utils/piModelMetadata.ts';

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

/**
 * `ModelFormModal` 把「输入类型」交回来时是 **JSON 字符串**（`ModelFormModal:761`
 * 的 `JSON.stringify(inputModalities)`），但 `models.json` 要的是**数组**
 * （上游 `docs/models.md` 的 `input`：`["text"]` 或 `["text", "image"]`）。
 * 直接赋值会写出 `"input": "[\"text\"]"` 这种引擎读不到的字符串。
 */
export const parseInputTypes = (value: string | undefined): string[] => {
  if (!value) return [];
  try {
    const parsed: unknown = JSON.parse(value);
    return Array.isArray(parsed)
      ? parsed.filter((entry): entry is string => typeof entry === 'string')
      : [];
  } catch {
    return [];
  }
};

/** 反向：把 `models.json` 里的 `input` 数组转成弹窗要的 JSON 字符串。 */
export const stringifyInputTypes = (value: unknown): string | undefined => {
  if (!Array.isArray(value)) return undefined;
  const strings = value.filter((entry): entry is string => typeof entry === 'string');
  return strings.length > 0 ? JSON.stringify(strings) : undefined;
};

/** 同上，用于 `thinkingLevelMap` 这类 JSON 对象字段。 */
export const stringifyRecordField = (value: unknown): string | undefined => {
  const record = asRecord(value);
  return isRecordEmpty(record) ? undefined : JSON.stringify(record, null, 2);
};

/** 反向：把弹窗交回的 JSON 字符串转成对象；非法或空则返回 `{}`。 */
export const parseJsonRecord = (value: string | undefined): Record<string, unknown> => {
  if (!value) return {};
  try {
    return asRecord(JSON.parse(value));
  } catch {
    return {};
  }
};

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

/**
 * 由预设模型构造一条 `models.json` 模型条目（「获取模型 → 应用」用）。
 *
 * 与 Pi 的 `buildPiModelFromPreset` 同构——**两者同一个引擎、同一份 schema**。
 * 预设只做「能力补全」，**绝不改写上游返回的 model id 大小写**。
 *
 * **不写模型级 `api`**：它是「覆盖 provider 的 `api`」用的可选项，provider 已经
 * 有值时再写一遍是冗余——引擎自己生成的行里也没有这个键。
 */
export const buildOmoNativeModelFromPreset = (
  preset: PresetModel,
  modelId: string,
  fallbackName: string,
): Record<string, unknown> => {
  const inputTypes = (preset.modalities?.input ?? []).filter((entry) => PI_INPUT_TYPES.has(entry));
  const cost = asRecord(preset.cost);
  const nextCost: Record<string, number> = {};
  const inputCost = getNumberField(cost, 'input');
  const outputCost = getNumberField(cost, 'output');
  const cacheReadCost = getNumberField(cost, 'cacheRead') ?? getNumberField(cost, 'cache_read');
  const cacheWriteCost = getNumberField(cost, 'cacheWrite') ?? getNumberField(cost, 'cache_write');
  if (inputCost !== undefined) nextCost.input = inputCost;
  if (outputCost !== undefined) nextCost.output = outputCost;
  if (cacheReadCost !== undefined) nextCost.cacheRead = cacheReadCost;
  if (cacheWriteCost !== undefined) nextCost.cacheWrite = cacheWriteCost;
  // 上游的 `null` 表示「该级别不支持」，是有效语义，但预设里那些只是「没这个变体」
  // ——滤掉 null 与空串，避免把「未声明」写成「显式不支持」。
  const thinkingLevelMap = Object.fromEntries(
    Object.entries(buildPiThinkingLevelMapFromPreset(preset.variants)).filter(
      ([, value]) => value !== null && value !== undefined && value !== '',
    ),
  );

  return {
    id: modelId,
    name: preset.name || fallbackName,
    ...(preset.reasoning !== undefined ? { reasoning: preset.reasoning } : {}),
    ...(inputTypes.length > 0 ? { input: inputTypes } : {}),
    ...(preset.contextLimit ? { contextWindow: preset.contextLimit } : {}),
    ...(preset.outputLimit ? { maxTokens: preset.outputLimit } : {}),
    ...(!isRecordEmpty(nextCost) ? { cost: nextCost } : {}),
    ...(!isRecordEmpty(thinkingLevelMap) ? { thinkingLevelMap } : {}),
  };
};

/** 「获取模型」新增一条：命中预设则补全能力，未命中只写 `id` + `name`。 */
export const buildFetchedOmoNativeModel = (
  fetchedModel: FetchedModel,
  matchedPresetModel?: PresetModel | null,
): Record<string, unknown> => {
  if (matchedPresetModel) {
    return buildOmoNativeModelFromPreset(
      matchedPresetModel,
      fetchedModel.id,
      fetchedModel.name || fetchedModel.id,
    );
  }
  return {
    id: fetchedModel.id,
    ...(fetchedModel.name ? { name: fetchedModel.name } : {}),
  };
};

// 曾经还有两个映射函数在这里：`omoNativeProviderToDisplayData`（卡片展示）与
// `omoNativeProviderToConnectivityInfo`（单项连通性弹窗入参）。2026-10-07 页面收敛后，
// 卡片展示改走 `OmoNativeProviderCard` 的 `ProviderCardVariantProps`，单项连通性测试
// 改走模型列表工具栏的 `onTestModels`（与 ZCode 一致，不再用弹窗）——两者都成了死代码。
// 连通性请求的形状组装现在内联在 `OmoNativeProvidersSection` 的两个 handler 里
// （批量 + 单项），它们共用 `buildProviderConnectivityBatchTarget`。
