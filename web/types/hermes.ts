/**
 * Hermes Agent configuration types.
 *
 * Hermes keeps its whole config in a single YAML file (`config.yaml`); the
 * provider fact source is that runtime file, not the DB. These types mirror
 * the backend serde contract (all field names are camelCase on the wire).
 */

export interface HermesPathInfo {
  path: string;
  source: 'custom' | 'env' | 'shell' | 'default';
}

export interface HermesSettingsConfig {
  configDir?: string | null;
  updatedAt: string;
}

export interface HermesSettingsConfigInput {
  configDir?: string | null;
  clearConfigDir?: boolean;
}

export type HermesProviderWarning = 'missing_provider' | 'missing_model';

/**
 * A single Hermes provider merged from `custom_providers` (writable list),
 * the `providers:` dict (read-only overlay) or the default provider record.
 */
export interface HermesRuntimeProviderView {
  providerKey: string;
  displayName: string;
  /** Raw `api_key` of the provider (masked in the UI). */
  credential?: unknown;
  apiMode?: string;
  /**
   * Raw provider JSON (custom_providers entry or providers dict entry),
   * with its `models` dict denormalized to a UI-friendly array.
   */
  provider?: Record<string, unknown>;
  modelIds?: string[];
  isBuiltin: boolean;
  /** True when the provider only exists in the read-only `providers:` dict. */
  isReadOnly: boolean;
  isDefault: boolean;
  warnings?: HermesProviderWarning[];
}

export interface HermesBuiltinProvider {
  key: string;
  name: string;
}

/** Top-level `model:` section parsed from config.yaml. */
export interface HermesModelSettings {
  defaultModel?: string | null;
  defaultProvider?: string | null;
  baseUrl?: string | null;
  contextLength?: number | null;
  maxTokens?: number | null;
}

/** Save input for the top-level `model:` section (string/number clear semantics follow pi). */
export interface HermesModelSettingsInput {
  defaultModel?: string | null;
  defaultProvider?: string | null;
  baseUrl?: string | null;
  contextLength?: number | null;
  maxTokens?: number | null;
  clearContextLength?: boolean;
  clearMaxTokens?: boolean;
}

/** Upsert a single `custom_providers` entry, keyed by `name`. */
export interface HermesModelsProviderInput {
  providerKey: string;
  provider: Record<string, unknown>;
}

export interface HermesRuntimeConfig {
  rootPathInfo: HermesPathInfo;
  configPath: string;
  promptPath: string;
  /** Raw `config.yaml` content as a JSON object (unknown top-level keys pass through). */
  config: Record<string, unknown>;
  modelSettings: HermesModelSettings;
  /** Everything except the managed keys (`model`, `custom_providers`, `providers`, `mcp_servers`, `_config_version`). */
  otherSettings: Record<string, unknown>;
  providers: HermesRuntimeProviderView[];
  builtinProviders: HermesBuiltinProvider[];
  /** Raw `config.yaml` file content for file-based preview. */
  configContent?: string | null;
  /** Raw prompt file content for file-based preview. */
  promptContent?: string | null;
}

/** Which Hermes memory blob to edit: agent `MEMORY.md` or user `USER.md`. */
export type HermesMemoryKind = 'memory' | 'user';

/** Character budgets + enable flags for Hermes' two memory blobs. */
export interface HermesMemoryLimits {
  memory: number;
  user: number;
  memoryEnabled: boolean;
  userEnabled: boolean;
}
