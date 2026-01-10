import { invoke } from '@tauri-apps/api/core';
import type { OhMyOpenCodeConfig, OhMyOpenCodeGlobalConfig } from '@/types/ohMyOpenCode';

// ============================================================================
// Oh My OpenCode API
// ============================================================================

/**
 * List all oh-my-opencode configurations
 */
export const listOhMyOpenCodeConfigs = async (): Promise<OhMyOpenCodeConfig[]> => {
    return await invoke<OhMyOpenCodeConfig[]>('list_oh_my_opencode_configs');
};

/**
 * Create a new oh-my-opencode configuration
 */
export const createOhMyOpenCodeConfig = async (
    config: OhMyOpenCodeConfigInput
): Promise<OhMyOpenCodeConfig> => {
    return await invoke<OhMyOpenCodeConfig>('create_oh_my_opencode_config', { input: config });
};

/**
 * Update an existing oh-my-opencode configuration
 */
export const updateOhMyOpenCodeConfig = async (
    config: OhMyOpenCodeConfigInput
): Promise<OhMyOpenCodeConfig> => {
    return await invoke<OhMyOpenCodeConfig>('update_oh_my_opencode_config', { input: config });
};

/**
 * Delete an oh-my-opencode configuration
 */
export const deleteOhMyOpenCodeConfig = async (id: string): Promise<void> => {
    await invoke('delete_oh_my_opencode_config', { id });
};

/**
 * Apply a configuration to the oh-my-opencode.json file
 */
export const applyOhMyOpenCodeConfig = async (configId: string): Promise<void> => {
    await invoke('apply_oh_my_opencode_config', { configId });
};

/**
 * Reorder configurations
 */
export const reorderOhMyOpenCodeConfigs = async (ids: string[]): Promise<void> => {
    await invoke('reorder_oh_my_opencode_configs', { ids });
};

/**
 * Get config file path info
 */
export const getOhMyOpenCodeConfigPathInfo = async (): Promise<{ path: string; source: string }> => {
    return await invoke('get_oh_my_opencode_config_path_info');
};

// ============================================================================
// Oh My OpenCode Global Config API
// ============================================================================

/**
 * Get global config (从 oh_my_opencode_global_config 表读取)
 */
export const getOhMyOpenCodeGlobalConfig = async (): Promise<OhMyOpenCodeGlobalConfig> => {
    return await invoke<OhMyOpenCodeGlobalConfig>('get_oh_my_opencode_global_config');
};

/**
 * Save global config (保存到 oh_my_opencode_global_config 表)
 */
export const saveOhMyOpenCodeGlobalConfig = async (
    config: OhMyOpenCodeGlobalConfigInput
): Promise<OhMyOpenCodeGlobalConfig> => {
    console.log('saveOhMyOpenCodeGlobalConfig input:', JSON.stringify(config, null, 2));
    const result = await invoke<OhMyOpenCodeGlobalConfig>('save_oh_my_opencode_global_config', { input: config });
    console.log('saveOhMyOpenCodeGlobalConfig result:', JSON.stringify(result, null, 2));
    return result;
};

// ============================================================================
// Types for API
// ============================================================================

export interface OhMyOpenCodeConfigInput {
    id?: string; // Optional - will be generated if not provided
    name: string;
    agents: Record<string, Record<string, unknown>> | null;
    other_fields?: Record<string, unknown>;
}

/**
 * Global Config Input Type - all nested configs are generic JSON
 */
export interface OhMyOpenCodeGlobalConfigInput {
    schema?: string;
    sisyphusAgent?: Record<string, unknown> | null;
    disabledAgents?: string[];
    disabledMcps?: string[];
    disabledHooks?: string[];
    lsp?: Record<string, unknown> | null;
    experimental?: Record<string, unknown> | null;
    otherFields?: Record<string, unknown>;
}

// ============================================================================
// Helper Functions
// ============================================================================

/**
 * Generate a unique ID for a new config
 */
export const generateOhMyOpenCodeConfigId = (): string => {
    const timestamp = Date.now().toString(36);
    const random = Math.random().toString(36).substring(2, 8);
    return `omo_config_${timestamp}_${random}`;
};

/**
 * Create a default config input with preset values
 */
export const createDefaultOhMyOpenCodeConfig = (name: string): OhMyOpenCodeConfigInput => {
    return {
        id: generateOhMyOpenCodeConfigId(),
        name,
        agents: {
            'Sisyphus': { model: 'opencode/minimax-m2.1-free' },
            'oracle': { model: '' },
            'librarian': { model: '' },
            'explore': { model: '' },
            'frontend-ui-ux-engineer': { model: '' },
            'document-writer': { model: '' },
            'multimodal-looker': { model: '' },
        },
    };
};

/**
 * Get display name for an agent type
 */
export const getAgentDisplayName = (agentType: string): string => {
    const displayNames: Record<string, string> = {
        'Sisyphus': 'Sisyphus (主编排器)',
        'oracle': 'Oracle (架构顾问)',
        'librarian': 'Librarian (研究员)',
        'explore': 'Explore (搜索专家)',
        'frontend-ui-ux-engineer': 'Frontend UI/UX Engineer (UI/UX专家)',
        'document-writer': 'Document Writer (文档专家)',
        'multimodal-looker': 'Multimodal Looker (视觉分析师)',
    };
    return displayNames[agentType] || agentType;
};

/**
 * Get agent description
 */
export const getAgentDescription = (agentType: string): string => {
    const descriptions: Record<string, string> = {
        'Sisyphus': '复杂任务规划、多步骤开发、代理协调',
        'oracle': '架构决策、代码审查、技术选型',
        'librarian': '文档查找、开源研究、最佳实践',
        'explore': '代码定位、依赖追踪、结构理解',
        'frontend-ui-ux-engineer': '界面设计实现、组件开发、动画',
        'document-writer': 'README、API 文档、架构文档',
        'multimodal-looker': '图片/PDF/图表分析',
    };
    return descriptions[agentType] || '';
};
