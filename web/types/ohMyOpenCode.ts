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
  /** Recommended model for this agent */
  recommendedModel?: string;
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
  // ===== Sisyphus's Curated Teammates (推荐配置的核心团队) =====
  {
    key: 'Sisyphus',
    display: 'Sisyphus',
    descZh: '主协调者 - 默认主智能体，负责任务规划、委派和执行协调',
    descEn: 'Primary orchestrator for planning, delegation, and execution coordination',
    recommendedModel: 'Claude Opus 4.5 High',
  },
  {
    key: 'hephaestus',
    display: 'Hephaestus',
    descZh: '深度工匠 - 自主深度工作者，目标导向执行，擅长复杂问题的深入研究和解决',
    descEn: 'Autonomous deep worker, goal-oriented execution — The Legitimate Craftsman',
    recommendedModel: 'GPT 5.2 Codex Medium',
  },
  {
    key: 'oracle',
    display: 'Oracle',
    descZh: '架构师 - 架构设计、调试、战略规划，利用GPT-5.2的逻辑推理能力',
    descEn: 'Architect for design, debugging, and strategic reasoning',
    recommendedModel: 'GPT 5.2 Medium',
  },
  {
    key: 'frontend-ui-ux-engineer',
    display: 'Frontend UI/UX',
    descZh: '前端工程师 - 前端开发，创建美观的用户界面，专注于创意和视觉设计',
    descEn: 'Frontend engineer focused on UI/UX design',
    recommendedModel: 'Gemini 3 Pro',
  },
  {
    key: 'explore',
    display: 'Explore',
    descZh: '探索者 - 通过上下文Grep快速探索代码库，闪电般的搜索速度',
    descEn: 'Blazing fast codebase exploration via Contextual Grep',
    recommendedModel: 'Claude Haiku 4.5',
  },
  {
    key: 'librarian',
    display: 'Librarian',
    descZh: '资料管理员 - 官方文档查找、开源实现搜索、代码库深度理解',
    descEn: 'Official docs, open source implementations, codebase exploration',
    recommendedModel: 'Claude Sonnet 4.5',
  },
  // ===== Separator =====
  {
    key: '__advanced_separator__',
    display: '─ Advanced ─',
    descZh: '以下 Agent 建议有经验的用户根据需要配置',
    descEn: 'The following agents are recommended for experienced users to configure as needed',
  },
  // ===== Advanced agents =====
  {
    key: 'multimodal-looker',
    display: 'Multimodal Looker',
    descZh: '多模态观察者 - 视觉内容专家，分析PDF、图像、图表等多媒体内容',
    descEn: 'Visual content specialist for PDFs, images, and diagrams',
  },
  {
    key: 'document-writer',
    display: 'Document Writer',
    descZh: '文档写手 - 技术写作专家，擅长流畅的技术文档写作',
    descEn: 'Technical writing specialist',
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
    descZh: '守门员 - 强制编排协议与风险控制，阻止编排者越权改项目文件',
    descEn: 'Gatekeeper enforcing orchestration protocol and delegation',
  },
  {
    key: 'Sisyphus-Junior',
    display: 'Sisyphus-Junior',
    descZh: '专注执行者 - 执行单元，直接编写代码，不能再委派任务，模型由category动态决定',
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
    descZh: '构建专家 - OpenCode原生build agent，默认禁用(被Sisyphus-Junior替代)',
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
    descZh: '前端工程师 - 前端开发、UI/UX设计、样式调整、动画效果，专注于视觉呈现 (Gemini 3 Pro)',
    descEn: 'Frontend and UI/UX tasks with visual focus (Gemini 3 Pro)',
  },
  {
    key: 'ultrabrain',
    display: 'Ultrabrain',
    descZh: '超级大脑 - 深度逻辑推理、复杂架构决策、需要大量分析的高难度问题 (GPT 5.2 Codex xhigh)',
    descEn: 'Deep reasoning and complex architecture decisions (GPT 5.2 Codex xhigh)',
  },
  {
    key: 'deep',
    display: 'Deep',
    descZh: '深度研究者 - 目标导向的自主问题解决，先深入研究再行动，适合棘手的深度理解问题 (GPT 5.2 Codex Medium)',
    descEn: 'Goal-oriented autonomous problem-solving. Thorough research before action. For hairy problems requiring deep understanding.',
  },
  {
    key: 'artistry',
    display: 'Artistry',
    descZh: '艺术家 - 高度创意任务、艺术性工作、新颖独特的想法生成 (Gemini 3 Pro max)',
    descEn: 'Highly creative and artistic tasks (Gemini 3 Pro max)',
  },
  {
    key: 'quick',
    display: 'Quick',
    descZh: '快速执行者 - 简单任务、单文件修改、拼写修复、小改动，省钱省时 (Claude Haiku 4.5)',
    descEn: 'Fast execution for small or trivial tasks (Claude Haiku 4.5)',
  },
  {
    key: 'unspecified-low',
    display: 'Unspecified (Low)',
    descZh: '通用助手(轻量) - 不适合其他类别的中等难度任务 (Claude Sonnet 4.5)',
    descEn: 'General helper for medium complexity tasks (Claude Sonnet 4.5)',
  },
  {
    key: 'unspecified-high',
    display: 'Unspecified (High)',
    descZh: '通用助手(重量) - 不适合其他类别的高难度复杂任务 (Claude Opus 4.5 max)',
    descEn: 'General helper for high complexity tasks (Claude Opus 4.5 max)',
  },
  {
    key: 'writing',
    display: 'Writing',
    descZh: '文档写手 - 通用文案、技术文档编写、README撰写、注释完善、技术写作 (Gemini 3 Flash)',
    descEn: 'Documentation and writing tasks (Gemini 3 Flash)',
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
