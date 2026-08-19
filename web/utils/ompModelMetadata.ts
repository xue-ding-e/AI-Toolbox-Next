import type { OpenCodeModelVariant } from '@/types/opencode';

export const PI_INPUT_TYPES = new Set(['text', 'image']);
export const PI_STANDARD_THINKING_LEVEL_KEYS = ['off', 'minimal', 'low', 'medium', 'high'] as const;
export const PI_EXTENDED_THINKING_LEVEL_KEYS = ['xhigh', 'max'] as const;
export const PI_THINKING_LEVEL_KEYS = [
  ...PI_STANDARD_THINKING_LEVEL_KEYS,
  ...PI_EXTENDED_THINKING_LEVEL_KEYS,
] as const;
export const PI_THINKING_LEVELS = new Set<string>(PI_THINKING_LEVEL_KEYS);
export const PI_THINKING_LEVEL_OPTIONS = PI_STANDARD_THINKING_LEVEL_KEYS.map((value) => ({
  value,
  label: value,
}));
const PI_EXTENDED_THINKING_LEVELS = new Set<string>(PI_EXTENDED_THINKING_LEVEL_KEYS);

// OMP 模型 `thinking` 结构支持的思考级别词表(不含 off/auto,它们与列表正交)。
const OMP_THINKING_EFFORT_KEYS = ['minimal', 'low', 'medium', 'high', 'xhigh', 'max'] as const;

const asRecord = (value: unknown): Record<string, unknown> => (
  value && typeof value === 'object' && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {}
);

const asStringArray = (value: unknown): string[] => (
  Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : []
);

/** OMP `thinking.mode` 上游枚举(与 OMP ThinkingControlModeSchema 一致)。 */
export const OMP_THINKING_MODE_KEYS = ['effort', 'budget', 'google-level', 'anthropic-adaptive', 'anthropic-budget-effort'] as const;
export type OmpThinkingMode = (typeof OMP_THINKING_MODE_KEYS)[number];

/**
 * 按 OMP provider `api` 推断 `thinking.mode` 的合理默认值,镜像上游
 * `inferThinkingControlMode`:google 系→google-level、anthropic-messages/
 * bedrock-converse-stream→anthropic-adaptive、其余→effort。api 缺失或未知
 * 时回退 effort。OMP 对 mode 校验严格(必填),生成的 thinking 必须带 mode,
 * 否则整个 models.yml 校验失败、所有自定义 provider 被禁用。
 */
export const inferOmpThinkingMode = (api?: string): OmpThinkingMode => {
  switch (api) {
    case 'google-generative-ai':
    case 'google-gemini-cli':
    case 'google-vertex':
      return 'google-level';
    case 'anthropic-messages':
    case 'bedrock-converse-stream':
      return 'anthropic-adaptive';
    default:
      return 'effort';
  }
};

/** 从 OMP 模型 `thinking` 结构推导可选思考级别列表(不含 off;off 表示关闭
 *  思考,由调用方作为独立选项处理,OMP 的 EffortSchema 词表为 minimal..max)。 */
export const getOmpModelThinkingLevels = (
  model: Record<string, unknown> | undefined,
): string[] => {
  if (!model || model.reasoning === false) {
    return [];
  }

  const thinking = asRecord(model.thinking);
  const efforts = asStringArray(thinking.efforts).filter((effort) =>
    OMP_THINKING_EFFORT_KEYS.some((key) => key === effort),
  );

  if (efforts.length > 0) {
    // 严格按模型声明列表返回(对应上游 getSupportedEfforts):
    // OMP 的 thinking.efforts 是完整支持集,不是标准级别的超集。之前取
    // 并集会让 UI 出现模型实际不支持的级别,选中保存后会被后端判为
    // unsupported → 全局 defaultThinkingLevel 被误删。efforts 须保序去重。
    const ordered: string[] = [];
    for (const effort of OMP_THINKING_EFFORT_KEYS) {
      if (efforts.includes(effort)) {
        ordered.push(effort);
      }
    }
    return ordered;
  }

  // legacy range vocabulary (pre-efforts configs)
  const minLevel = typeof thinking.minLevel === 'string' ? thinking.minLevel : undefined;
  const maxLevel = typeof thinking.maxLevel === 'string' ? thinking.maxLevel : undefined;
  if (minLevel || maxLevel) {
    const minIndex = minLevel
      ? PI_THINKING_LEVEL_KEYS.indexOf(minLevel as (typeof PI_THINKING_LEVEL_KEYS)[number])
      : 1; // 不含 off
    const maxIndex = maxLevel
      ? PI_THINKING_LEVEL_KEYS.indexOf(maxLevel as (typeof PI_THINKING_LEVEL_KEYS)[number])
      : PI_THINKING_LEVEL_KEYS.length - 1;
    if (minIndex >= 1 && maxIndex >= 1 && minIndex <= maxIndex) {
      return PI_THINKING_LEVEL_KEYS.slice(0, maxIndex + 1).filter(
        (_level, index) => index >= minIndex,
      );
    }
  }

  // 模型无 thinking 块:上游 getSupportedEfforts 对它返回空数组(即 OMP 认为该
  // 模型不受控,不声明任何 effort)。这里回退到全局词表 minimal..high 是给用户
  // 一个可选的降级 UI——因为 defaultThinkingLevel 是全局键、后端也不再据此
  // gating,这个兜底无害;语义是"全局词表兜底",而非"该模型声称支持这些级别"。
  return model.reasoning === true ? [...PI_STANDARD_THINKING_LEVEL_KEYS.slice(1)] : [];
};

/** 从 OMP 模型 `thinking` 结构读取默认思考级别(取 defaultLevel,无则 undefined)。 */
export const getOmpModelDefaultThinkingLevel = (
  model: Record<string, unknown> | undefined,
): string | undefined => {
  if (!model || model.reasoning === false) {
    return undefined;
  }
  const thinking = asRecord(model.thinking);
  return typeof thinking.defaultLevel === 'string' ? thinking.defaultLevel : undefined;
};

export const normalizeOmpThinkingLevelKey = (key: string): string | undefined => {
  if (key === 'none') {
    return 'off';
  }
  return PI_THINKING_LEVELS.has(key) ? key : undefined;
};

export const isOmpThinkingLevelMapEntrySupported = (
  levelKey: string,
  thinkingLevelMap: Record<string, unknown>,
): boolean => {
  const mappedValue = thinkingLevelMap[levelKey];
  if (mappedValue === null) {
    return false;
  }
  return !PI_EXTENDED_THINKING_LEVELS.has(levelKey) || mappedValue !== undefined;
};

export const getPresetThinkingLevelValue = (
  variant: OpenCodeModelVariant,
): string | null | undefined => {
  if (variant.disabled === true) {
    return null;
  }
  if (typeof variant.reasoningEffort === 'string') {
    return variant.reasoningEffort === 'none' ? 'none' : variant.reasoningEffort;
  }
  // Claude / Anthropic OpenCode presets use top-level `effort` (not reasoningEffort).
  if (typeof variant.effort === 'string') {
    return variant.effort === 'none' ? 'none' : variant.effort;
  }
  const thinkingConfig = asRecord(variant.thinkingConfig);
  if (typeof thinkingConfig.thinkingLevel === 'string') {
    return thinkingConfig.thinkingLevel;
  }
  if (typeof variant.thinkingLevel === 'string') {
    return variant.thinkingLevel;
  }
  return undefined;
};

/** 从 OpenCode preset variants 推导 OMP 的 `thinking` 结构(按 effort 聚合)。
 *  `mode` 由 OMP provider 的 `api` 推断(见 {@link inferOmpThinkingMode});api
 *  未知时回退 effort。OMP 的 thinking schema 将 mode 视为必填,缺失会导致整个
 *  models.yml 校验失败。 */
export const buildOmpThinkingFromPreset = (
  variants: Record<string, OpenCodeModelVariant> | undefined,
  api?: string,
): Record<string, unknown> | undefined => {
  if (!variants || Object.keys(variants).length === 0) {
    return undefined;
  }
  const efforts: string[] = [];
  let defaultLevel: string | undefined;
  Object.entries(variants).forEach(([variantKey, variant]) => {
    const levelKey = normalizeOmpThinkingLevelKey(variantKey);
    if (!levelKey) {
      return;
    }
    const levelValue = getPresetThinkingLevelValue(variant);
    if (
      typeof levelValue === 'string'
      && levelValue !== 'none'
      && OMP_THINKING_EFFORT_KEYS.some((key) => key === levelValue)
      && !efforts.includes(levelValue)
    ) {
      efforts.push(levelValue);
      if (variant.disabled !== true && variantKey === 'high') {
        defaultLevel = levelValue;
      }
    }
  });

  if (efforts.length === 0) {
    return undefined;
  }
  // canonical effort ordering
  efforts.sort(
    (left, right) =>
      OMP_THINKING_EFFORT_KEYS.indexOf(left as never) - OMP_THINKING_EFFORT_KEYS.indexOf(right as never),
  );
  const thinking: Record<string, unknown> = { mode: inferOmpThinkingMode(api), efforts };
  if (defaultLevel) {
    thinking.defaultLevel = defaultLevel;
  }
  return thinking;
};
