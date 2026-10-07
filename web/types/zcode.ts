export interface ConfigPathInfo {
  path: string;
  source: 'custom' | 'env' | 'shell' | 'default';
}

/** Which `modelConfigRules` array a model row is projected into. */
export type ZcodeModelRuleKind = 'smart' | 'manual';

/**
 * Input modalities.
 *
 * ZCode validates model rules with a `.strict()` schema, and the two rule kinds
 * accept different subsets: a manual rule only carries
 * `supportsImage`/`supportsVideo`/`supportsPdf`, while `supportsText` and
 * `supportsAudio` are smart-only. The backend filters the rest out.
 */
export interface ZcodeModelInputFormat {
  supportsText?: boolean;
  supportsImage?: boolean;
  supportsVideo?: boolean;
  supportsAudio?: boolean;
  supportsPdf?: boolean;
}

/** Output modalities. Only text is expressible today; smart rules only. */
export interface ZcodeModelOutputFormat {
  supportsText?: boolean;
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
 *
 * `outputFormat`, `supportsToolCall`, and `requiresMfjsToolSchema` are
 * smart-only — a manual rule must not carry them, or ZCode rejects the file.
 * The backend filters by rule kind before writing.
 */
export interface ZcodeModelProperties {
  contextWindow?: number;
  inputFormat?: ZcodeModelInputFormat;
  outputFormat?: ZcodeModelOutputFormat;
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

/**
 * Provider credentials.
 *
 * ZCode validates this with a `.strict()` schema accepting exactly these three
 * keys, so no other field may be added here.
 */
export interface ZcodeProviderAccess {
  /** `api-key` or `zhipu-coding-plan-api-key`. */
  type?: string;
  apiKey?: string;
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
  /**
   * Where the official-account card sits in the provider list, counted as the
   * number of provider cards above it. UI state; absent means "at the top".
   */
  officialAccountIndex?: number | null;
  updatedAt: string;
}

/** One file as the config preview shows it; `content` is absent when it does not exist. */
export interface ZcodePreviewFile {
  path: string;
  content?: string;
}

export interface ZcodeConfigPreview {
  providerConfig: ZcodePreviewFile;
  /** Only present on installations that have not migrated past `config.json`. */
  legacyConfig?: ZcodePreviewFile;
  cliConfig: ZcodePreviewFile;
  setting: ZcodePreviewFile;
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

/** An official-account snapshot, or the virtual entry mirroring the live login. */
export interface ZcodeOfficialAccount {
  id: string;
  /** OAuth namespace the snapshot logs into: `zai` or `bigmodel`. */
  providerId: string;
  name: string;
  /** `oauth` for a saved snapshot, `local` for the live-login mirror. */
  kind: 'oauth' | 'local';
  email?: string;
  accountId?: string;
  isApplied: boolean;
  isVirtual: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ZcodeOfficialAccountApplyResult {
  /** Name of the login saved aside before it was overwritten, when there was one. */
  preservedAs?: string;
}

/** The providers `zcode login` accepts, in the order its help lists them. */
export const ZCODE_LOGIN_PROVIDERS = [
  { value: 'zai', label: 'z.ai' },
  { value: 'bigmodel', label: 'BigModel' },
] as const;
