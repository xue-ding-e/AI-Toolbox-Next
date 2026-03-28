/**
 * Oh My OpenAgent configuration types.
 *
 * All nested config objects are generic JSON to keep the editor flexible while
 * the upstream project is still evolving.
 */

/**
 * Agent configuration - generic JSON structure.
 */
export type OhMyOpenAgentAgentConfig = Record<string, unknown>;

/**
 * Sisyphus agent specific configuration - generic JSON structure.
 */
export type OhMyOpenAgentSisyphusConfig = Record<string, unknown>;

/**
 * LSP server configuration - generic JSON structure.
 */
export type OhMyOpenAgentLspServer = Record<string, unknown>;

/**
 * Experimental features configuration - generic JSON structure.
 */
export type OhMyOpenAgentExperimental = Record<string, unknown>;

/**
 * Agent definition for the Oh My OpenAgent compatibility layer.
 */
export interface OhMyOpenAgentAgentDefinition {
	/** Agent key used in configuration. */
	key: string;
}

/**
 * Category definition for the Oh My OpenAgent compatibility layer.
 */
export interface OhMyOpenAgentCategoryDefinition {
	/** Category key used in configuration. */
	key: string;
}

/**
 * Centralized agent definitions for Oh My OpenAgent.
 * Order defines UI display and should be updated intentionally.
 */
export const OH_MY_OPENAGENT_AGENTS: OhMyOpenAgentAgentDefinition[] = [
	// ===== 主 Agents：用户的直接入口，负责协调和决策（你主动找他们）=====
	{
		key: "__main_agents_separator__",
	},
	{
		key: "sisyphus",
	},
	{
		key: "hephaestus",
	},
	{
		key: "prometheus",
	},
	{
		key: "atlas",
	},
	// ===== 子 Agents：专业领域专家，被主 Agent 或系统调用（他们被动工作）=====
	{
		key: "__sub_agents_separator__",
	},
	{
		key: "oracle",
	},
	{
		key: "librarian",
	},
	{
		key: "explore",
	},
	{
		key: "multimodal-looker",
	},
	{
		key: "metis",
	},
	{
		key: "momus",
	},
	{
		key: "sisyphus-junior",
	},
];

/**
 * Centralized category definitions for Oh My OpenAgent.
 * Order follows the default categories in 3.1.
 */
export const OH_MY_OPENAGENT_CATEGORIES: OhMyOpenAgentCategoryDefinition[] = [
	{
		key: "visual-engineering",
	},
	{
		key: "ultrabrain",
	},
	{
		key: "deep",
	},
	{
		key: "artistry",
	},
	{
		key: "quick",
	},
	{
		key: "unspecified-low",
	},
	{
		key: "unspecified-high",
	},
	{
		key: "writing",
	},
];

/**
 * Agent types supported by Oh My OpenAgent.
 * Auto-generated from OH_MY_OPENAGENT_AGENTS.
 */
export type OhMyOpenAgentAgentType =
	(typeof OH_MY_OPENAGENT_AGENTS)[number]["key"];

/**
 * Oh My OpenAgent Agents Profile (子 Agents 配置方案).
 * 只包含各 Agent 的模型配置，可以有多个方案供切换。
 */
export interface OhMyOpenAgentAgentsProfile {
	id: string;
	name: string;
	isApplied: boolean;
	isDisabled: boolean;
	agents: Record<string, OhMyOpenAgentAgentConfig> | null;
	categories?: Record<string, OhMyOpenAgentAgentConfig> | null;
	otherFields?: Record<string, unknown>;
	sortIndex?: number;
	createdAt?: string;
	updatedAt?: string;
}

/**
 * Oh My OpenAgent Global Config (全局通用配置).
 * 全局唯一配置，存储在数据库中，固定 ID 为 "global"。
 * 当从本地文件加载时，ID 为 "__local__"。
 */
export interface OhMyOpenAgentGlobalConfig {
	id: string;
	schema?: string;
	sisyphusAgent: Record<string, unknown> | null;
	disabledAgents?: string[];
	disabledMcps?: string[];
	disabledHooks?: string[];
	disabledSkills?: string[];
	lsp: Record<string, unknown> | null;
	experimental: Record<string, unknown> | null;
	backgroundTask?: Record<string, unknown> | null;
	browserAutomationEngine?: Record<string, unknown> | null;
	claudeCode?: Record<string, unknown> | null;
	otherFields?: Record<string, unknown>;
	updatedAt?: string;
}

export interface OhMyOpenAgentLegacyUpgradeStatus {
	needsUpgrade: boolean;
	hasLegacyPlugin: boolean;
	hasLegacyLocalConfig: boolean;
	hasLegacyCustomConfigPath: boolean;
	hasLegacyWslMapping: boolean;
	hasLegacySshMapping: boolean;
	localConfigPath?: string;
}

export interface OhMyOpenAgentLegacyUpgradeResult {
	changed: boolean;
	pluginUpdated: boolean;
	localConfigRenamed: boolean;
	customConfigPathUpdated: boolean;
	wslMappingUpdated: boolean;
	wslFileRenamed: boolean;
	sshMappingUpdated: boolean;
}

/**
 * Main persisted Oh My OpenAgent profile type.
 */
export type OhMyOpenAgentConfig = OhMyOpenAgentAgentsProfile;

/**
 * Form values for Agents Profile modal.
 */
export interface OhMyOpenAgentAgentsProfileFormValues {
	id: string;
	name: string;
	isDisabled?: boolean;
	agents: Record<string, OhMyOpenAgentAgentConfig> | null;
	categories?: Record<string, OhMyOpenAgentAgentConfig> | null;
	otherFields?: Record<string, unknown>;
}

/**
 * Form values for Global Config modal.
 */
export interface OhMyOpenAgentGlobalConfigFormValues {
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
 * Combined form values used by the config editor.
 */
export type OhMyOpenAgentConfigFormValues =
	OhMyOpenAgentAgentsProfileFormValues &
	OhMyOpenAgentGlobalConfigFormValues;

/**
 * Oh My OpenAgent JSON file structure.
 */
export interface OhMyOpenAgentJsonConfig {
	$schema?: string;
	agents?: Record<string, OhMyOpenAgentAgentConfig>;
	categories?: Record<string, OhMyOpenAgentAgentConfig>;
	sisyphus_agent?: OhMyOpenAgentSisyphusConfig;
	disabled_agents?: string[];
	disabled_mcps?: string[];
	disabled_hooks?: string[];
	disabled_skills?: string[];
	lsp?: Record<string, OhMyOpenAgentLspServer>;
	experimental?: OhMyOpenAgentExperimental;
	background_task?: Record<string, unknown>;
	browser_automation_engine?: Record<string, unknown>;
	claude_code?: Record<string, unknown>;
}
