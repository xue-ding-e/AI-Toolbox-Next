export interface ConfigPathInfo {
  path: string;
  source: 'custom' | 'env' | 'shell' | 'default';
}

/** Which `modelConfigRules` array a model row is projected into. */
export type ZcodeModelRuleKind = 'smart' | 'manual';

export interface ZcodeModelInputFormat {
  supportsText?: boolean;
  supportsImage?: boolean;
  supportsVideo?: boolean;
  supportsAudio?: boolean;
  supportsPdf?: boolean;
}

export interface ZcodeMaxOutputTokensSpec {
  max?: number;
  /** Mapping expression string, not a JSON object. Round-trips verbatim. */
  map?: string;
}

export interface ZcodeReasoningLevelSpec {
  /** Ordered low to high. */
  values?: string[];
  /** Mapping expression string, not a JSON object. Round-trips verbatim. */
  map?: string;
}

export interface ZcodeModelOptionSpecs {
  maxOutputTokens?: ZcodeMaxOutputTokensSpec;
  reasoningLevel?: ZcodeReasoningLevelSpec;
}

/**
 * Capability flags. `undefined` inherits from ZCode's catalog; an explicit
 * `false` pins the capability off.
 */
export interface ZcodeModelProperties {
  contextWindow?: number;
  inputFormat?: ZcodeModelInputFormat;
  supportsToolCall?: boolean;
  supportsJsonSchemaOutput?: boolean;
  supportsNativeWebSearch?: boolean;
  supportsMidConversationSystem?: boolean;
  requiresMfjsToolSchema?: boolean;
}

export interface ZcodeModelRow {
  modelId: string;
  displayName?: string;
  ruleKind: ZcodeModelRuleKind;
  enabled?: boolean;
  properties?: ZcodeModelProperties;
  optionSpecs?: ZcodeModelOptionSpecs;
  isDefault: boolean;
}

export interface ZcodeProviderAccess {
  type?: string;
  apiKey?: string;
  apiKeyRequired?: boolean;
  apiKeyManagementUrl?: string;
}

export interface ZcodeProviderApi {
  type?: string;
  baseUrl?: string;
  headers?: Record<string, string>;
}

export interface ZcodeProviderLogo {
  type?: string;
  key?: string;
}

export interface ZcodeProviderConfig {
  group?: string;
  logo?: ZcodeProviderLogo;
  access?: ZcodeProviderAccess;
  api?: ZcodeProviderApi;
  personalModelIds?: string[];
  modelOrder?: string[];
  visibility?: string;
}

export interface ZcodeSettingsConfig {
  providerId: string;
  providerName?: string;
  templateId?: string;
  config?: ZcodeProviderConfig;
  models: ZcodeModelRow[];
  defaultModelId?: string;
}

export interface ZcodeProvider {
  id: string;
  name: string;
  category: string;
  settingsConfig: string;
  sourceProviderId?: string | null;
  websiteUrl?: string | null;
  notes?: string | null;
  icon?: string | null;
  iconColor?: string | null;
  sortIndex: number;
  meta?: Record<string, unknown> | null;
  isApplied: boolean;
  isDisabled: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ZcodeProviderInput {
  id?: string | null;
  name: string;
  category: string;
  settingsConfig: string;
  sourceProviderId?: string | null;
  websiteUrl?: string | null;
  notes?: string | null;
  icon?: string | null;
  iconColor?: string | null;
  isDisabled?: boolean | null;
  meta?: Record<string, unknown> | null;
}

export interface ZcodeProviderTemplate {
  templateId: string;
  name: string;
  apiType?: string | null;
  baseUrl?: string | null;
  logoKey?: string | null;
}

export interface ZcodeCommonConfig {
  config: string;
  rootDir?: string | null;
  updatedAt: string;
}

export interface ZcodeCommonConfigInput {
  config: string;
  rootDir?: string | null;
  clearRootDir?: boolean | null;
}

export interface ZcodePromptConfigInput {
  id?: string | null;
  name: string;
  content: string;
}

export interface ZcodePromptConfig {
  id: string;
  name: string;
  content: string;
  isApplied: boolean;
  sortIndex: number;
  createdAt: string;
  updatedAt: string;
}

/** ZCode's own API protocol identifiers. */
export const ZCODE_API_TYPES = [
  { value: 'anthropic-messages', label: 'Anthropic Messages' },
  { value: 'openai-chat-completions', label: 'Chat Completions' },
  { value: 'openai-responses', label: 'Responses' },
] as const;
