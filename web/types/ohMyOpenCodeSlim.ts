/**
 * Type definitions for oh-my-opencode-slim plugin configuration management.
 *
 * Oh My OpenCode Slim is a lightweight alternative to oh-my-opencode with fewer agents.
 * Configuration structure is similar but focuses on essential agents only:
 * - orchestrator
 * - oracle
 * - librarian
 * - explorer
 * - designer
 * - fixer
 */

/**
 * Agent configuration for oh-my-opencode-slim
 */
export interface OhMyOpenCodeSlimAgent {
  model?: string;
  skills?: string[];
  [key: string]: any; // Allow additional custom fields
}

/**
 * Centralized agent definitions for oh-my-opencode-slim
 */
export interface OhMyOpenCodeSlimAgents {
  orchestrator?: OhMyOpenCodeSlimAgent;
  oracle?: OhMyOpenCodeSlimAgent;
  librarian?: OhMyOpenCodeSlimAgent;
  explorer?: OhMyOpenCodeSlimAgent;
  designer?: OhMyOpenCodeSlimAgent;
  fixer?: OhMyOpenCodeSlimAgent;
  [key: string]: any; // Allow additional custom agents
}

/**
 * Oh My OpenCode Slim Agents Profile (stored in database)
 */
export interface OhMyOpenCodeSlimConfig {
  id: string;
  name: string;
  isApplied: boolean;
  agents?: OhMyOpenCodeSlimAgents;
  otherFields?: Record<string, any>; // For extra configuration fields
  createdAt?: string;
  updatedAt?: string;
}

/**
 * Input type for creating/updating Agents Profile
 */
export interface OhMyOpenCodeSlimConfigInput {
  id?: string;
  name: string;
  agents?: OhMyOpenCodeSlimAgents;
  otherFields?: Record<string, any>;
}

/**
 * Oh My OpenCode Slim Global Config
 */
export interface OhMyOpenCodeSlimGlobalConfig {
  id: string; // Fixed as "global"
  schema?: string;
  sisyphusAgent?: any;
  disabledAgents?: string[];
  disabledMcps?: string[];
  disabledHooks?: string[];
  lsp?: any;
  experimental?: any;
  otherFields?: Record<string, any>;
  updatedAt?: string;
}

/**
 * Input type for Global Config
 */
export interface OhMyOpenCodeSlimGlobalConfigInput {
  schema?: string;
  sisyphusAgent?: any;
  disabledAgents?: string[];
  disabledMcps?: string[];
  disabledHooks?: string[];
  lsp?: any;
  experimental?: any;
  otherFields?: Record<string, any>;
}

/**
 * Config path info
 */
export interface ConfigPathInfo {
  path: string;
  source: string;
}

/**
 * Agent types supported by oh-my-opencode-slim
 */
export const SLIM_AGENT_TYPES = [
  'orchestrator',
  'oracle',
  'librarian',
  'explorer',
  'designer',
  'fixer',
] as const;

export type SlimAgentType = typeof SLIM_AGENT_TYPES[number];

/**
 * Agent display names (Chinese + English)
 */
export const SLIM_AGENT_DISPLAY_NAMES: Record<SlimAgentType, string> = {
  orchestrator: '编排者 (Orchestrator)',
  oracle: '神谕者 (Oracle)',
  librarian: '图书管理员 (Librarian)',
  explorer: '探索者 (Explorer)',
  designer: '设计师 (Designer)',
  fixer: '修复者 (Fixer)',
};

/**
 * Agent descriptions (Chinese)
 */
export const SLIM_AGENT_DESCRIPTIONS: Record<SlimAgentType, string> = {
  orchestrator: '编写并执行代码，编排多代理工作流，从言语中解析未说出的意图，在战斗中召唤专家。直接塑造现实——当宇宙变得过于庞大时，把领域交给别人。',
  oracle: '根本原因分析、架构审查、调试指导、权衡分析。只读：神谕者提供建议，不直接介入。',
  librarian: '文档查询、GitHub 代码搜索、库研究、最佳实践检索。只读：他们获取智慧；实现交给别人。',
  explorer: '正则搜索、AST 模式匹配、文件发现、并行探索。只读：他们绘制疆域；其他人征服它。',
  designer: '现代响应式设计、CSS/Tailwind 精通、微动画与组件架构。优先视觉卓越而非代码完美——美感为先。',
  fixer: '代码实现、重构、测试、验证。执行计划——不研究、不委派、不策划。',
};
