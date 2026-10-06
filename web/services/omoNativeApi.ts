import { invoke } from '@tauri-apps/api/core';
import type {
  OmoNativeAgentsConfig,
  OmoNativeAgentsConfigInput,
  OmoNativeCliInfo,
  OmoNativeMcpServer,
  OmoNativeModelProfile,
  OmoNativePathInfo,
  OmoNativeProvider,
  OmoNativeProviderInput,
  OmoNativeRuntimeConfig,
  OmoNativeSettingsConfig,
  OmoNativeSettingsConfigInput,
} from '@/types/omoNative';

// ============================================================================
// 路径与设置
// ============================================================================

export const getOmoNativeRootPathInfo = async (): Promise<OmoNativePathInfo> =>
  await invoke<OmoNativePathInfo>('get_omo_native_root_path_info');

export const getOmoNativeSettingsConfig = async (): Promise<OmoNativeSettingsConfig | null> =>
  await invoke<OmoNativeSettingsConfig | null>('get_omo_native_settings_config');

export const saveOmoNativeSettingsConfig = async (
  config: OmoNativeSettingsConfigInput,
): Promise<void> => {
  await invoke('save_omo_native_settings_config', { config });
};

export const readOmoNativeRuntimeConfig = async (): Promise<OmoNativeRuntimeConfig> =>
  await invoke<OmoNativeRuntimeConfig>('read_omo_native_runtime_config');

export const getOmoNativeCliInfo = async (): Promise<OmoNativeCliInfo> =>
  await invoke<OmoNativeCliInfo>('get_omo_native_cli_info');

// ============================================================================
// Agent/Category 方案
// ============================================================================

export const listOmoNativeAgentsConfigs = async (): Promise<OmoNativeAgentsConfig[]> =>
  await invoke<OmoNativeAgentsConfig[]>('list_omo_native_agents_configs');

export const createOmoNativeAgentsConfig = async (
  config: OmoNativeAgentsConfigInput,
): Promise<OmoNativeAgentsConfig> =>
  await invoke<OmoNativeAgentsConfig>('create_omo_native_agents_config', { config });

export const updateOmoNativeAgentsConfig = async (
  config: OmoNativeAgentsConfigInput,
): Promise<OmoNativeAgentsConfig> =>
  await invoke<OmoNativeAgentsConfig>('update_omo_native_agents_config', { config });

export const deleteOmoNativeAgentsConfig = async (configId: string): Promise<void> => {
  await invoke('delete_omo_native_agents_config', { configId });
};

export const reorderOmoNativeAgentsConfigs = async (ids: string[]): Promise<void> => {
  await invoke('reorder_omo_native_agents_configs', { ids });
};

export const toggleOmoNativeAgentsConfigDisabled = async (
  configId: string,
  isDisabled: boolean,
): Promise<void> => {
  await invoke('toggle_omo_native_agents_config_disabled', { configId, isDisabled });
};

export const applyOmoNativeAgentsConfig = async (configId: string): Promise<void> => {
  await invoke('apply_omo_native_agents_config', { configId });
};

export const clearOmoNativeAppliedConfig = async (configId: string): Promise<void> => {
  await invoke('clear_omo_native_applied_config', { configId });
};

export const saveOmoNativeLocalConfig = async (
  config: OmoNativeAgentsConfigInput,
): Promise<OmoNativeAgentsConfig> =>
  await invoke<OmoNativeAgentsConfig>('save_omo_native_local_config', { config });

// ============================================================================
// 名单与模型档
// ============================================================================

export const listOmoNativeBuiltinAgents = async (): Promise<string[]> =>
  await invoke<string[]>('list_omo_native_builtin_agents');

export const listOmoNativeBuiltinCategories = async (): Promise<string[]> =>
  await invoke<string[]>('list_omo_native_builtin_categories');

export const listOmoNativeModelProfiles = async (): Promise<OmoNativeModelProfile[]> =>
  await invoke<OmoNativeModelProfile[]>('list_omo_native_model_profiles');

// ============================================================================
// Providers
// ============================================================================

export const listOmoNativeProviders = async (): Promise<OmoNativeProvider[]> =>
  await invoke<OmoNativeProvider[]>('list_omo_native_providers');

export const saveOmoNativeProvider = async (input: OmoNativeProviderInput): Promise<void> => {
  await invoke('save_omo_native_provider', { input });
};

export const deleteOmoNativeProvider = async (
  providerKey: string,
  removeKey: boolean,
): Promise<void> => {
  await invoke('delete_omo_native_provider', { providerKey, removeKey });
};

// ============================================================================
// MCP 与 Skills
// ============================================================================

export const listOmoNativeMcpServers = async (): Promise<OmoNativeMcpServer[]> =>
  await invoke<OmoNativeMcpServer[]>('list_omo_native_mcp_servers');

export const saveOmoNativeMcpServer = async (
  name: string,
  config: Record<string, unknown>,
): Promise<void> => {
  await invoke('save_omo_native_mcp_server', { name, config });
};

export const deleteOmoNativeMcpServer = async (name: string): Promise<void> => {
  await invoke('delete_omo_native_mcp_server', { name });
};
