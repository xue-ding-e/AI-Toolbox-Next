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
 * Category definition for oh-my-opencode
 */
export interface OhMyOpenCodeCategoryDefinition {
  /** Category key used in configuration */
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
 * Order defines UI display and should be updated intentionally
 */
export const OH_MY_OPENCODE_AGENTS: OhMyOpenCodeAgentDefinition[] = [
  // ===== Core agents (Sisyphus, Prometheus, Atlas first) =====
  {
    key: 'Sisyphus',
    display: 'Sisyphus',
    descZh: '主协调者 - 默认主智能体，负责整体任务的规划、委派和执行协调',
    descEn: 'Primary orchestrator for planning, delegation, and execution coordination',
  },
  {
    key: 'Prometheus (Planner)',
    display: 'Prometheus',
    descZh: '规划师 - 任务规划，使用工作规划方法论进行任务分解和策略制定',
    descEn: 'Planner agent that decomposes tasks and builds strategy',
  },
  {
    key: 'Atlas',
    display: 'Atlas',
    descZh: '守门员 - 强制编排协议与风险控制，阻止编排者越权改项目文件，要求通过委派执行',
    descEn: 'Gatekeeper enforcing orchestration protocol and delegation',
  },
  // ===== Other agents =====
  {
    key: 'oracle',
    display: 'Oracle',
    descZh: '架构师 - 架构设计、代码审查、战略规划，利用GPT-5.2的逻辑推理能力',
    descEn: 'Architect for design, review, and strategic reasoning',
  },
  {
    key: 'librarian',
    display: 'Librarian',
    descZh: '资料管理员 - 多仓库分析、文档查找、实现示例搜索，深度代码库理解和GitHub研究',
    descEn: 'Documentation lookup and multi-repo analysis',
  },
  {
    key: 'explore',
    display: 'Explore',
    descZh: '探索者 - 快速代码库探索和模式匹配，专注于代码搜索和发现',
    descEn: 'Fast codebase exploration and pattern discovery',
  },
  {
    key: 'multimodal-looker',
    display: 'Multimodal Looker',
    descZh: '多模态观察者 - 视觉内容专家，分析PDF、图像、图表等多媒体内容',
    descEn: 'Visual content specialist for PDFs, images, and diagrams',
  },
  {
    key: 'frontend-ui-ux-engineer',
    display: 'Frontend UI/UX',
    descZh: '前端UI/UX工程师 - 前端开发，创建美观的用户界面，专注于创意和视觉设计',
    descEn: 'Frontend engineer focused on UI/UX design',
  },
  {
    key: 'document-writer',
    display: 'Document Writer',
    descZh: '文档写手 - 技术写作专家，擅长流畅的技术文档写作',
    descEn: 'Technical writing specialist',
  },
  {
    key: 'Sisyphus-Junior',
    display: 'Sisyphus-Junior',
    descZh: '专注执行者 - 执行单元，直接编写代码，不能再委派任务，模型由category动态决定(此为兜底)',
    descEn: 'Focused executor that writes code directly and cannot re-delegate',
  },
  {
    key: 'Metis (Plan Consultant)',
    display: 'Metis',
    descZh: '计划顾问 - 预规划分析，识别隐藏需求和潜在的AI失败点',
    descEn: 'Plan consultant for pre-analysis and risk detection',
  },
  {
    key: 'Momus (Plan Reviewer)',
    display: 'Momus',
    descZh: '计划审查员 - 计划审查，对生成的计划进行质量检查和风险评估',
    descEn: 'Plan reviewer for quality checks and risk assessment',
  },
  {
    key: 'OpenCode-Builder',
    display: 'OpenCode-Builder',
    descZh: '构建专家 - OpenCode原生build agent，默认禁用(被Sisyphus-Junior替代)，需手动启用',
    descEn: 'OpenCode native build agent (disabled by default)',
  },
];

/**
 * Centralized category definitions for oh-my-opencode
 * Order follows the default categories in 3.1
 */
export const OH_MY_OPENCODE_CATEGORIES: OhMyOpenCodeCategoryDefinition[] = [
  {
    key: 'visual-engineering',
    display: 'Visual Engineering',
    descZh: '前端工程师 - 前端开发、UI/UX设计、样式调整、动画效果，专注于视觉呈现',
    descEn: 'Frontend and UI/UX tasks with visual focus',
  },
  {
    key: 'ultrabrain',
    display: 'Ultrabrain',
    descZh: '超级大脑 - 深度逻辑推理、复杂架构决策、需要大量分析的高难度问题',
    descEn: 'Deep reasoning and complex architecture decisions',
  },
  {
    key: 'artistry',
    display: 'Artistry',
    descZh: '艺术家 - 高度创意任务、艺术性工作、新颖独特的想法生成',
    descEn: 'Highly creative and artistic tasks',
  },
  {
    key: 'quick',
    display: 'Quick',
    descZh: '快速执行者 - 简单任务、单文件修改、拼写修复、小改动，省钱省时',
    descEn: 'Fast execution for small or trivial tasks',
  },
  {
    key: 'unspecified-low',
    display: 'Unspecified (Low)',
    descZh: '通用助手(轻量) - 不适合其他类别的中等难度任务',
    descEn: 'General helper for medium complexity tasks',
  },
  {
    key: 'unspecified-high',
    display: 'Unspecified (High)',
    descZh: '通用助手(重量) - 不适合其他类别的高难度复杂任务',
    descEn: 'General helper for high complexity tasks',
  },
  {
    key: 'writing',
    display: 'Writing',
    descZh: '文档写手 - 通用文案、技术文档编写、README撰写、注释完善、技术写作',
    descEn: 'Documentation and writing tasks',
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
  isDisabled: boolean;
  agents: Record<string, OhMyOpenCodeAgentConfig> | null; // Generic JSON
  categories?: Record<string, OhMyOpenCodeAgentConfig> | null; // Generic JSON
  otherFields?: Record<string, unknown>;
  createdAt?: string;
  updatedAt?: string;
}

/**
 * Oh My OpenCode Global Config (全局通用配置)
 * 全局唯一配置，存储在数据库中，固定 ID 为 "global"
 * 当从本地文件加载时，ID 为 "__local__"
 */
export interface OhMyOpenCodeGlobalConfig {
  id: string; // "global" or "__local__"
  schema?: string;
  sisyphusAgent: Record<string, unknown> | null; // Generic JSON
  disabledAgents?: string[];
  disabledMcps?: string[];
  disabledHooks?: string[];
  disabledSkills?: string[];
  lsp: Record<string, unknown> | null; // Generic JSON
  experimental: Record<string, unknown> | null; // Generic JSON
  backgroundTask?: Record<string, unknown> | null;
  browserAutomationEngine?: Record<string, unknown> | null;
  claudeCode?: Record<string, unknown> | null;
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
  isDisabled?: boolean;
  agents: Record<string, OhMyOpenCodeAgentConfig> | null;
  categories?: Record<string, OhMyOpenCodeAgentConfig> | null;
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
  disabledSkills?: string[];
  lsp?: Record<string, unknown> | null;
  experimental?: Record<string, unknown> | null;
  backgroundTask?: Record<string, unknown> | null;
  browserAutomationEngine?: Record<string, unknown> | null;
  claudeCode?: Record<string, unknown> | null;
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
  categories?: Record<string, OhMyOpenCodeAgentConfig>;
  sisyphus_agent?: OhMyOpenCodeSisyphusConfig;
  disabled_agents?: string[];
  disabled_mcps?: string[];
  disabled_hooks?: string[];
  disabled_skills?: string[];
  lsp?: Record<string, OhMyOpenCodeLspServer>;
  experimental?: OhMyOpenCodeExperimental;
  background_task?: Record<string, unknown>;
  browser_automation_engine?: Record<string, unknown>;
  claude_code?: Record<string, unknown>;
}
