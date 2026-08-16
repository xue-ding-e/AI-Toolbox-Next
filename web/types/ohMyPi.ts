export interface OmpPathInfo {
  path: string;
  source: 'custom' | 'env' | 'shell' | 'default';
}

export interface OmpSettingsConfig {
  rootDir?: string | null;
  updatedAt?: string;
}

export interface OmpSettingsConfigInput {
  rootDir?: string | null;
  clearRootDir?: boolean;
}

export type OmpProviderSource = 'official_builtin' | 'models_yml' | 'settings_yml';
export type OmpProviderCategory = 'subscription' | 'api_key' | 'custom';
export type OmpCredentialKind = 'api_key' | 'oauth' | 'env_possible' | 'none';
export type OmpProviderWarning = 'missing_provider' | 'missing_model';

export interface OmpDefaultSelection {
  providerKey?: string | null;
  modelId?: string | null;
  thinkingLevel?: string | null;
}

export interface OmpRuntimeProviderView {
  providerKey: string;
  displayName: string;
  sources: OmpProviderSource[];
  categories: OmpProviderCategory[];
  credentialKind: OmpCredentialKind;
  credential?: unknown;
  modelsProvider?: Record<string, unknown>;
  runtimeFiles: string[];
  isBuiltin: boolean;
  isOverride: boolean;
  isDefault: boolean;
  modelIds?: string[];
  warnings?: OmpProviderWarning[];
}

export interface OmpBuiltinProvider {
  key: string;
  name: string;
}

export interface OmpRuntimeConfig {
  rootPathInfo: OmpPathInfo;
  configPath: string;
  modelsPath: string;
  mcpPath: string;
  promptPath: string;
  settings: Record<string, unknown>;
  models: Record<string, unknown>;
  otherSettings: Record<string, unknown>;
  modelSettings: OmpDefaultSelection;
  providers: OmpRuntimeProviderView[];
  builtinProviders: OmpBuiltinProvider[];
  /** Raw `config.yml` / `config.yaml` file content for file-based preview. */
  configContent?: string | null;
  /** Raw `models.yml` file content for file-based preview. */
  modelsContent?: string | null;
  /** Raw `mcp.json` file content for file-based preview. */
  mcpContent?: string | null;
  /** Raw prompt file content for file-based preview. */
  promptContent?: string | null;
}

export interface OmpModelSettingsInput {
  defaultProvider?: string | null;
  defaultModel?: string | null;
  defaultThinkingLevel?: string | null;
  /** Explicitly remove defaultThinkingLevel; distinguishes "cleared" from "not touched". */
  clearThinkingLevel?: boolean;
}

export interface OmpModelsProviderInput {
  providerKey: string;
  provider: Record<string, unknown>;
}

export type OmpExtensionScope = 'user' | 'project' | 'unknown';
export type OmpExtensionKind = 'package' | 'local_file' | 'local_directory';

export interface OmpExtensionSummary {
  id: string;
  source: string;
  scope: OmpExtensionScope;
  kind: OmpExtensionKind;
  path?: string;
  builtIn?: boolean;
  currentVersion?: string;
  latestVersion?: string;
  updateAvailable?: boolean;
}

export interface OmpExtensionListResult {
  extensionsPath: string;
  packagesPath: string;
  extensions: OmpExtensionSummary[];
  raw: string;
  /** Resolved host-side `omp` path (or WSL label) used for plugin CLI ops. */
  cliPath?: string;
  /** Best-effort `omp --version` for the resolved CLI. */
  cliVersion?: string;
}

export interface OmpExtensionInstallInput {
  source: string;
}

export interface OmpExtensionUpdateInput {
  /** When set, upgrades one plugin (`omp plugin upgrade <name>`). When omitted, upgrades all. */
  source?: string;
}

export interface OmpExtensionActionInput {
  source: string;
  scope?: OmpExtensionScope;
  kind?: OmpExtensionKind;
  path?: string;
}

export interface OmpExtensionCommandResult {
  command: string;
  output: string;
}
