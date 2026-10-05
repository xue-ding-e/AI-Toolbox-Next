/**
 * OmO Native 配置类型。
 *
 * OmO Native 是上游 oh-my-openagent 5.0 起的**独立 edition**（`omo` 二进制 +
 * senpi 引擎，npm 包 `omo-ai`），与 OpenCode 插件版共用 `~/.omo/omo.jsonc`
 * 但读不同的 harness 块：插件版读 `[opencode]`，Native 读 `[native]`。
 *
 * 两套 edition 的 agent / category 名单**不相交**，不要与本项目
 * `@/types/ohMyOpenAgent`（插件版名单）互相套用。
 */

/** 运行时根目录信息。 */
export interface OmoNativePathInfo {
  path: string;
  source: string;
}

export interface OmoNativeSettingsConfig {
  rootDir?: string;
  updatedAt: string;
}

export interface OmoNativeSettingsConfigInput {
  rootDir?: string;
  clearRootDir?: boolean;
}

/** `omo` 二进制检测结果。 */
export interface OmoNativeCliInfo {
  found: boolean;
  path?: string;
  version?: string;
  versionNumber?: string;
  engineVersion?: string;
  error?: string;
}

/** 一个 agent/category 配置方案。 */
export interface OmoNativeAgentsConfig {
  id: string;
  name: string;
  isApplied: boolean;
  isDisabled: boolean;
  agents?: Record<string, unknown> | null;
  categories?: Record<string, unknown> | null;
  modelProfiles?: Record<string, unknown> | null;
  modelProfile?: string | null;
  task?: Record<string, unknown> | null;
  otherFields?: Record<string, unknown> | null;
  sortIndex?: number;
  createdAt?: string;
  updatedAt?: string;
}

export interface OmoNativeAgentsConfigInput {
  id?: string;
  name: string;
  isApplied?: boolean;
  isDisabled?: boolean;
  agents?: Record<string, unknown> | null;
  categories?: Record<string, unknown> | null;
  modelProfiles?: Record<string, unknown> | null;
  modelProfile?: string | null;
  task?: Record<string, unknown> | null;
  otherFields?: Record<string, unknown> | null;
}

/** `read_omo_native_runtime_config` 的返回。 */
export interface OmoNativeRuntimeConfig {
  /** 折叠后的 Native 生效视图（共享 base 键被 `[native]` 块覆盖）。 */
  effective: Record<string, unknown>;
  /** `[native]` 块原文（未折叠）。 */
  nativeBlock: Record<string, unknown>;
  /** 被 Native 视图采纳的共享 base 键。 */
  sharedBase: Record<string, unknown>;
  configPath: string;
  parsed: boolean;
  parseError?: string;
  settingsContent?: string;
  modelsContent?: string;
  mcpContent?: string;
}

export interface OmoNativeModelProfile {
  name: string;
  displayName?: string;
  family?: string;
  tier?: string;
  models?: unknown;
  builtin: boolean;
}

export interface OmoNativeProvider {
  key: string;
  builtin: boolean;
  oauth: boolean;
  config: Record<string, unknown>;
  hasKey: boolean;
  custom: boolean;
}

export interface OmoNativeProviderInput {
  key: string;
  config: Record<string, unknown>;
  apiKey?: string;
}

export interface OmoNativeMcpServer {
  name: string;
  config: Record<string, unknown>;
}

export interface OmoNativeSkill {
  name: string;
  path: string;
  description?: string;
}

/**
 * Native 内建 curated agents（核实于上游 v5.1.19
 * `packages/senpi-task/src/agents/builtin/*.ts`）。
 *
 * 前四个是 curated read-only agents；后三个是 ulw-loop reviewer。
 * `metis`/`momus` 已在上游 5.0.0-beta.51 后不再作为别名解析。
 */
export const OMO_NATIVE_AGENTS: string[] = [
  'explore',
  'librarian',
  'plan-consultant',
  'plan-reviewer',
  'omo-native-code-reviewer',
  'omo-native-gate-reviewer',
  'omo-native-qa-executor',
];

/**
 * Native 内建 categories（核实于上游 v5.1.19
 * `packages/senpi-task/src/category/*-categories.ts`）。
 *
 * 与插件版的关键差异：Native 独有 `architect`；`deep` 已拆成
 * `deep-low` / `deep-high`。
 */
export const OMO_NATIVE_CATEGORIES: string[] = [
  'architect',
  'visual-engineering',
  'artistry',
  'writing',
  'ultrabrain',
  'deep-low',
  'deep-high',
  'quick',
  'unspecified-low',
  'unspecified-high',
];

/**
 * `[native]` 块的合法键（上游 `OmoTypedHarnessConfigSchema`，`.strict()`）。
 * 写入时只能出现这些键，否则触发上游 unknown-key diagnostic。
 */
export const OMO_NATIVE_BLOCK_KEYS: string[] = [
  'formatOnMutation',
  'gateway',
  'categories',
  'agents',
  'git_master',
  'task',
  'teams',
  'models',
  'model_profiles',
  'model_profile',
  'memory',
  'telemetry',
  'computer',
  'disabled_skills',
];

/** 本模块接管的键（方案编辑器直接编辑的部分）。 */
export const OMO_NATIVE_MANAGED_KEYS: string[] = [
  'agents',
  'categories',
  'model_profiles',
  'model_profile',
  'task',
];
