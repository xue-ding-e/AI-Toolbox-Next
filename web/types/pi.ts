export interface PiPathInfo {
  path: string;
  source: 'custom' | 'env' | 'shell' | 'default';
}

export interface PiSettingsConfig {
  rootDir?: string | null;
  updatedAt?: string;
}

export interface PiSettingsConfigInput {
  rootDir?: string | null;
  clearRootDir?: boolean;
}

export type PiProviderSource = 'official_builtin' | 'auth_json' | 'models_json' | 'settings_json';
export type PiProviderCategory = 'subscription' | 'api_key' | 'custom';
export type PiCredentialKind = 'api_key' | 'oauth' | 'env_possible' | 'none';
export type PiProviderWarning = 'missing_provider' | 'missing_model';
export type PiDeleteScope = 'credential' | 'provider_config' | 'both';

export interface PiDefaultSelection {
  providerKey?: string | null;
  modelId?: string | null;
  thinkingLevel?: string | null;
}

export interface PiRuntimeProviderView {
  providerKey: string;
  displayName: string;
  sources: PiProviderSource[];
  categories: PiProviderCategory[];
  credentialKind: PiCredentialKind;
  credential?: unknown;
  modelsProvider?: Record<string, unknown>;
  runtimeFiles: string[];
  isBuiltin: boolean;
  isOverride: boolean;
  isDefault: boolean;
  modelIds?: string[];
  warnings?: PiProviderWarning[];
}

export interface PiBuiltinProvider {
  key: string;
  name: string;
}

export interface PiRuntimeConfig {
  rootPathInfo: PiPathInfo;
  settingsPath: string;
  authPath: string;
  modelsPath: string;
  promptPath: string;
  settings: Record<string, unknown>;
  auth: Record<string, unknown>;
  models: Record<string, unknown>;
  otherSettings: Record<string, unknown>;
  modelSettings: PiDefaultSelection;
  providers: PiRuntimeProviderView[];
  builtinProviders: PiBuiltinProvider[];
}

export interface PiModelSettingsInput {
  defaultProvider?: string | null;
  defaultModel?: string | null;
  defaultThinkingLevel?: string | null;
}

export interface PiAuthProviderInput {
  providerKey: string;
  credential: Record<string, unknown>;
}

export interface PiModelsProviderInput {
  providerKey: string;
  provider: Record<string, unknown>;
}
