/**
 * OpenClaw Configuration Types
 *
 * Type definitions for OpenClaw configuration management.
 * Mirrors the Rust types in tauri/src/coding/open_claw/types.rs
 */

export interface OpenClawModelCost {
  input: number;
  output: number;
  cacheRead?: number;
  cacheWrite?: number;
  [key: string]: unknown;
}

export interface OpenClawModel {
  id: string;
  name?: string;
  alias?: string;
  contextWindow?: number;
  maxTokens?: number;
  reasoning?: boolean;
  input?: string[];
  cost?: OpenClawModelCost;
  [key: string]: unknown;
}

export interface OpenClawProviderConfig {
  baseUrl?: string;
  apiKey?: string;
  api?: string;
  models: OpenClawModel[];
  [key: string]: unknown;
}

export interface OpenClawModelsSection {
  mode?: string;
  providers?: Record<string, OpenClawProviderConfig>;
  [key: string]: unknown;
}

export interface OpenClawDefaultModel {
  primary: string;
  fallbacks: string[];
  [key: string]: unknown;
}

export interface OpenClawModelCatalogEntry {
  alias?: string;
  [key: string]: unknown;
}

export interface OpenClawAgentsDefaults {
  model?: OpenClawDefaultModel;
  models?: Record<string, OpenClawModelCatalogEntry>;
  [key: string]: unknown;
}

export interface OpenClawAgentsSection {
  defaults?: OpenClawAgentsDefaults;
  [key: string]: unknown;
}

export interface OpenClawEnvConfig {
  [key: string]: unknown;
}

export interface OpenClawToolsConfig {
  profile?: string;
  allow?: string[];
  deny?: string[];
  [key: string]: unknown;
}

export interface OpenClawConfig {
  models?: OpenClawModelsSection;
  agents?: OpenClawAgentsSection;
  env?: OpenClawEnvConfig;
  tools?: OpenClawToolsConfig;
  [key: string]: unknown;
}

export interface OpenClawConfigPathInfo {
  path: string;
  source: 'custom' | 'default';
}

export type ReadOpenClawConfigResult =
  | { status: 'success'; config: OpenClawConfig }
  | { status: 'notFound'; path: string }
  | { status: 'parseError'; path: string; error: string; contentPreview?: string }
  | { status: 'error'; error: string };

export interface OpenClawCommonConfig {
  configPath: string | null;
  updatedAt: string;
}
