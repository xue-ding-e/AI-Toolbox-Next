/**
 * Codex Configuration Types
 *
 * Type definitions for Codex configuration management.
 */

export type CodexProviderCategory = 'official' | 'third_party' | 'custom';

/**
 * Codex Provider settings configuration
 * Contains auth.json and config.toml content
 */
export interface CodexSettingsConfig {
  auth?: {
    OPENAI_API_KEY?: string;
  };
  config?: string; // TOML format string
}

/**
 * Codex Provider stored in database
 */
export interface CodexProvider {
  id: string;
  name: string;
  category: CodexProviderCategory;
  settingsConfig: string; // JSON string of CodexSettingsConfig
  sourceProviderId?: string;
  websiteUrl?: string;
  notes?: string;
  icon?: string;
  iconColor?: string;
  sortIndex?: number;
  isApplied?: boolean;
  createdAt: string;
  updatedAt: string;
}

/**
 * Common configuration for all providers
 */
export interface CodexCommonConfig {
  config: string; // TOML format string
  updatedAt?: string;
}

/**
 * Codex settings from files
 */
export interface CodexSettings {
  auth?: Record<string, string>;
  config?: string;
}

/**
 * Form values for creating/editing a provider
 */
export interface CodexProviderFormValues {
  name: string;
  category: CodexProviderCategory;
  apiKey?: string;
  baseUrl?: string;
  model?: string;
  configToml?: string;
  notes?: string;
  sourceProviderId?: string;
}

/**
 * Import conflict action
 */
export type ImportConflictAction = 'overwrite' | 'duplicate' | 'cancel';

/**
 * Import conflict info
 */
export interface ImportConflictInfo {
  existingProvider: CodexProvider;
  newProviderName: string;
  sourceProviderId: string;
}
