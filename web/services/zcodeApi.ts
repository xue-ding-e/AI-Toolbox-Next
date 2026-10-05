import { invoke } from '@tauri-apps/api/core';
import type {
  ConfigPathInfo,
  ZcodeCommonConfig,
  ZcodeCommonConfigInput,
  ZcodeProvider,
  ZcodeProviderInput,
  ZcodeProviderTemplate,
} from '@/types/zcode';

export const listZcodeProviders = async (): Promise<ZcodeProvider[]> => {
  return await invoke<ZcodeProvider[]>('list_zcode_providers');
};

export const createZcodeProvider = async (
  provider: ZcodeProviderInput,
): Promise<ZcodeProvider> => {
  return await invoke<ZcodeProvider>('create_zcode_provider', { provider });
};

export const updateZcodeProvider = async (
  provider: ZcodeProvider,
): Promise<ZcodeProvider> => {
  return await invoke<ZcodeProvider>('update_zcode_provider', { provider });
};

export const deleteZcodeProvider = async (id: string): Promise<void> => {
  await invoke('delete_zcode_provider', { id });
};

export const reorderZcodeProviders = async (ids: string[]): Promise<void> => {
  await invoke('reorder_zcode_providers', { ids });
};

export const toggleZcodeProviderDisabled = async (
  providerId: string,
  isDisabled: boolean,
): Promise<void> => {
  await invoke('toggle_zcode_provider_disabled', { providerId, isDisabled });
};

export const getZcodeConfigFilePath = async (): Promise<string> => {
  return await invoke<string>('get_zcode_config_file_path');
};

export const getZcodeRootPathInfo = async (): Promise<ConfigPathInfo> => {
  return await invoke<ConfigPathInfo>('get_zcode_root_path_info');
};

export const revealZcodeConfigFolder = async (): Promise<void> => {
  await invoke('reveal_zcode_config_folder');
};

export const readZcodeSettings = async (): Promise<unknown> => {
  return await invoke('read_zcode_settings');
};

/**
 * Whether ZCode has migrated to the new provider registry. Until it has,
 * applying a provider would silently have no effect.
 */
export const getZcodeGenerationStatus = async (): Promise<boolean> => {
  return await invoke<boolean>('get_zcode_generation_status');
};

export const getZcodeCommonConfig = async (): Promise<ZcodeCommonConfig> => {
  return await invoke<ZcodeCommonConfig>('get_zcode_common_config');
};

export const saveZcodeCommonConfig = async (input: ZcodeCommonConfigInput): Promise<void> => {
  await invoke('save_zcode_common_config', { input });
};

export const listZcodeProviderTemplates = async (): Promise<ZcodeProviderTemplate[]> => {
  return await invoke<ZcodeProviderTemplate[]>('list_zcode_provider_templates');
};

export const saveZcodeProvider = async (provider: ZcodeProviderInput): Promise<string> => {
  return await invoke<string>('save_zcode_provider', { provider });
};

export const deleteZcodeProviderFromFile = async (providerId: string): Promise<void> => {
  await invoke('delete_zcode_provider_from_file', { providerId });
};

export const selectZcodeProvider = async (
  providerId: string,
  modelId: string,
): Promise<void> => {
  await invoke('select_zcode_provider', { providerId, modelId });
};

export type { ZcodeProvider };
