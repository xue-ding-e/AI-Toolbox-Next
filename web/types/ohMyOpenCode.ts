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
 * Agent definition for oh-my-opencode
 */
export interface OhMyOpenCodeAgentDefinition {
  /** Agent key used in configuration */
  key: string;
  /** Display name shown in UI */
  display: string;
  /** Chinese description */
  descZh: string;
  /** English description */
  descEn: string;
}

/**
 * Centralized agent definitions for oh-my-opencode
 * New agents should be appended at the end to maintain display order
 */
export const OH_MY_OPENCODE_AGENTS: OhMyOpenCodeAgentDefinition[] = [
  // ===== Existing agents (maintain original order) =====
  {
    key: 'Sisyphus',
    display: 'Sisyphus',
    descZh: '主编排器，复杂任务规划、多步骤开发、代理协调',
    descEn: 'Main orchestrator for complex task planning, multi-step development, and agent coordination',
  },
  {
    key: 'oracle',
    display: 'Oracle',
    descZh: '架构师、代码审查、战略分析、调试专家',
    descEn: 'Architect, code reviewer, strategic analyst, and debugging expert',
  },
  {
    key: 'librarian',
    display: 'Librarian',
    descZh: '多仓库分析、官方文档查询、开源实现搜索',
    descEn: 'Multi-repo analysis, official docs lookup, and open source implementation search',
  },
  {
    key: 'explore',
    display: 'Explore',
    descZh: '极速代码库扫描、模式匹配、上下文 Grep',
    descEn: 'Blazing fast codebase exploration, pattern matching, contextual grep',
  },
  {
    key: 'frontend-ui-ux-engineer',
    display: 'Frontend UI/UX',
    descZh: '界面设计实现、组件开发、动画效果',
    descEn: 'UI design implementation, component development, and animations',
  },
  {
    key: 'document-writer',
    display: 'Document Writer',
    descZh: '技术文档编写：README、API 文档、架构文档',
    descEn: 'Technical writing: README, API docs, architecture docs',
  },
  {
    key: 'multimodal-looker',
    display: 'Multimodal Looker',
    descZh: '视觉内容分析：PDF、图片、图表解读',
    descEn: 'Visual content analysis: PDFs, images, diagrams',
  },
  // ===== Sisyphus related agents (appended at the end) =====
  {
    key: 'build',
    display: 'Build',
    descZh: '默认构建智能体，负责代码实现',
    descEn: 'Default build agent for code implementation',
  },
  {
    key: 'plan',
    display: 'Plan',
    descZh: '默认计划智能体，负责任务规划',
    descEn: 'Default plan agent for task planning',
  },
  {
    key: 'OpenCode-Builder',
    display: 'OpenCode-Builder',
    descZh: 'OpenCode 的默认构建智能体，由于 SDK 限制而重命名（默认禁用）',
    descEn: 'OpenCode default build agent, renamed due to SDK limitations (disabled by default)',
  },
  {
    key: 'Prometheus (Planner)',
    display: 'Prometheus',
    descZh: 'OpenCode 的默认规划智能体，带有 work-planner 方法论（默认启用）',
    descEn: 'OpenCode default planner with work-planner methodology (enabled by default)',
  },
  {
    key: 'Metis (Plan Consultant)',
    display: 'Metis',
    descZh: '预规划分析智能体，识别隐藏需求和 AI 失败点',
    descEn: 'Pre-planning analysis agent, identifying hidden requirements and AI failure points',
  },
  {
    key: 'Momus (Plan Reviewer)',
    display: 'Momus',
    descZh: '计划审核者',
    descEn: 'Plan reviewer',
  },
  {
    key: 'orchestrator-sisyphus',
    display: 'Orchestrator Sisyphus',
    descZh: 'Sisyphus 编排器（Beta）',
    descEn: 'Sisyphus orchestrator (Beta)',
  },
];

/**
 * Agent types supported by oh-my-opencode
 * Auto-generated from OH_MY_OPENCODE_AGENTS
 */
export type OhMyOpenCodeAgentType = typeof OH_MY_OPENCODE_AGENTS[number]['key'];

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
