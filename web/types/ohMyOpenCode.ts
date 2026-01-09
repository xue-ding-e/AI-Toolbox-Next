/**
 * Oh My OpenCode Configuration Types
 * 
 * Type definitions for oh-my-opencode plugin configuration management.
 */

/**
 * Agent configuration
 */
export interface OhMyOpenCodeAgentConfig {
  model?: string;
  temperature?: number;
  top_p?: number;
  prompt?: string;
  prompt_append?: string;
  disable?: boolean;
  description?: string;
  mode?: 'subagent' | 'primary' | 'all';
  color?: string;
  [key: string]: unknown;
}

/**
 * Sisyphus agent specific configuration
 */
export interface OhMyOpenCodeSisyphusConfig {
  disabled?: boolean;
  default_builder_enabled?: boolean;
  planner_enabled?: boolean;
  replace_plan?: boolean;
}

/**
 * LSP Server configuration
 */
export interface OhMyOpenCodeLspServer {
  command?: string[];
  extensions?: string[];
  priority?: number;
  env?: Record<string, string>;
  initialization?: Record<string, unknown>;
  disabled?: boolean;
}

/**
 * Experimental features configuration
 */
export interface OhMyOpenCodeExperimental {
  preemptive_compaction_threshold?: number;
  truncate_all_tool_outputs?: boolean;
  aggressive_truncation?: boolean;
  auto_resume?: boolean;
  dcp_for_compaction?: boolean;
}

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
  agents: {
    Sisyphus?: OhMyOpenCodeAgentConfig;
    oracle?: OhMyOpenCodeAgentConfig;
    librarian?: OhMyOpenCodeAgentConfig;
    explore?: OhMyOpenCodeAgentConfig;
    'frontend-ui-ux-engineer'?: OhMyOpenCodeAgentConfig;
    'document-writer'?: OhMyOpenCodeAgentConfig;
    'multimodal-looker'?: OhMyOpenCodeAgentConfig;
  };
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
  sisyphusAgent?: OhMyOpenCodeSisyphusConfig;
  disabledAgents?: string[];
  disabledMcps?: string[];
  disabledHooks?: string[];
  lsp?: Record<string, OhMyOpenCodeLspServer>;
  experimental?: OhMyOpenCodeExperimental;
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
  agents: Record<string, OhMyOpenCodeAgentConfig | undefined>;
  otherFields?: Record<string, unknown>;
}

/**
 * Form values for Global Config modal
 */
export interface OhMyOpenCodeGlobalConfigFormValues {
  sisyphusAgent?: OhMyOpenCodeSisyphusConfig;
  disabledAgents?: string[];
  disabledMcps?: string[];
  disabledHooks?: string[];
  lsp?: Record<string, OhMyOpenCodeLspServer>;
  experimental?: OhMyOpenCodeExperimental;
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
  agents?: {
    [key: string]: OhMyOpenCodeAgentConfig;
  };
  sisyphus_agent?: OhMyOpenCodeSisyphusConfig;
  disabled_agents?: string[];
  disabled_mcps?: string[];
  disabled_hooks?: string[];
  disabled_skills?: string[];
  disabled_commands?: string[];
  lsp?: Record<string, OhMyOpenCodeLspServer>;
  experimental?: OhMyOpenCodeExperimental;
}
