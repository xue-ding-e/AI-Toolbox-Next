/**
 * Oh My OpenCode Configuration Types
 *
 * Type definitions for oh-my-opencode plugin configuration management.
 * All nested config objects are generic JSON to allow flexibility.
 */

/**
 * Agent configuration - generic JSON structure
 */
export type OhMyOpenCodeAgentConfig = Record<string, unknown>;

/**
 * Sisyphus agent specific configuration - generic JSON structure
 */
export type OhMyOpenCodeSisyphusConfig = Record<string, unknown>;

/**
 * LSP Server configuration - generic JSON structure
 */
export type OhMyOpenCodeLspServer = Record<string, unknown>;

/**
 * Experimental features configuration - generic JSON structure
 */
export type OhMyOpenCodeExperimental = Record<string, unknown>;

/**
 * Agent types supported by oh-my-opencode
 */
export type OhMyOpenCodeAgentType =
  | 'Sisyphus'
  | 'oracle'
  | 'librarian'
  | 'explore'
  | 'frontend-ui-ux-engineer'
  | 'document-writer'
  | 'multimodal-looker';

/**
 * Oh My OpenCode Agents Profile (子 Agents 配置方案)
 * 只包含各 Agent 的模型配置，可以有多个方案供切换
 */
export interface OhMyOpenCodeAgentsProfile {
  id: string;
  name: string;
  isApplied: boolean;
  agents: Record<string, OhMyOpenCodeAgentConfig> | null; // Generic JSON
  otherFields?: Record<string, unknown>;
  createdAt?: string;
  updatedAt?: string;
}

/**
 * Oh My OpenCode Global Config (全局通用配置)
 * 全局唯一配置，存储在数据库中，固定 ID 为 "global"
 */
export interface OhMyOpenCodeGlobalConfig {
  id: 'global';
  schema?: string;
  sisyphusAgent: Record<string, unknown> | null; // Generic JSON
  disabledAgents?: string[];
  disabledMcps?: string[];
  disabledHooks?: string[];
  lsp: Record<string, unknown> | null; // Generic JSON
  experimental: Record<string, unknown> | null; // Generic JSON
  otherFields?: Record<string, unknown>;
  updatedAt?: string;
}

/**
 * @deprecated 使用 OhMyOpenCodeAgentsProfile 代替
 * 保留用于向后兼容
 */
export type OhMyOpenCodeConfig = OhMyOpenCodeAgentsProfile;

/**
 * Form values for Agents Profile modal (简化版)
 */
export interface OhMyOpenCodeAgentsProfileFormValues {
  id: string;
  name: string;
  agents: Record<string, OhMyOpenCodeAgentConfig> | null;
  otherFields?: Record<string, unknown>;
}

/**
 * Form values for Global Config modal
 */
export interface OhMyOpenCodeGlobalConfigFormValues {
  schema?: string;
  sisyphusAgent: Record<string, unknown> | null;
  disabledAgents?: string[];
  disabledMcps?: string[];
  disabledHooks?: string[];
  lsp?: Record<string, unknown> | null;
  experimental?: Record<string, unknown> | null;
  otherFields?: Record<string, unknown>;
}

/**
 * @deprecated 使用 OhMyOpenCodeAgentsProfileFormValues 代替
 */
export type OhMyOpenCodeConfigFormValues = OhMyOpenCodeAgentsProfileFormValues & OhMyOpenCodeGlobalConfigFormValues;

/**
 * Oh My OpenCode JSON file structure
 */
export interface OhMyOpenCodeJsonConfig {
  $schema?: string;
  agents?: Record<string, OhMyOpenCodeAgentConfig>;
  sisyphus_agent?: OhMyOpenCodeSisyphusConfig;
  disabled_agents?: string[];
  disabled_mcps?: string[];
  disabled_hooks?: string[];
  lsp?: Record<string, OhMyOpenCodeLspServer>;
  experimental?: OhMyOpenCodeExperimental;
}
