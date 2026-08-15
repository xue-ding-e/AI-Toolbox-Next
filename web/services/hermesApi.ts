import { invoke } from '@tauri-apps/api/core';
import type {
  HermesMemoryKind,
  HermesMemoryLimits,
  HermesModelSettingsInput,
  HermesModelsProviderInput,
  HermesPathInfo,
  HermesRuntimeConfig,
  HermesSettingsConfig,
  HermesSettingsConfigInput,
} from '@/types/hermes';

export const getHermesRootPathInfo = async (): Promise<HermesPathInfo> => {
  return await invoke<HermesPathInfo>('get_hermes_root_path_info');
};

export const getHermesSettingsConfig = async (): Promise<HermesSettingsConfig | null> => {
  return await invoke<HermesSettingsConfig | null>('get_hermes_settings_config');
};

export const saveHermesSettingsConfig = async (
  input: HermesSettingsConfigInput,
): Promise<void> => {
  await invoke('save_hermes_settings_config', { input });
};

export const readHermesRuntimeConfig = async (): Promise<HermesRuntimeConfig> => {
  return await invoke<HermesRuntimeConfig>('read_hermes_runtime_config');
};

export const saveHermesModelsProvider = async (
  input: HermesModelsProviderInput,
): Promise<HermesRuntimeConfig> => {
  return await invoke<HermesRuntimeConfig>('save_hermes_models_provider', { input });
};

export const deleteHermesRuntimeProvider = async (
  providerKey: string,
): Promise<HermesRuntimeConfig> => {
  return await invoke<HermesRuntimeConfig>('delete_hermes_runtime_provider', { providerKey });
};

export const saveHermesModelSettings = async (
  input: HermesModelSettingsInput,
): Promise<HermesRuntimeConfig> => {
  return await invoke<HermesRuntimeConfig>('save_hermes_model_settings', { input });
};

export const saveHermesOtherSettings = async (
  otherSettings: Record<string, unknown>,
): Promise<HermesRuntimeConfig> => {
  return await invoke<HermesRuntimeConfig>('save_hermes_other_settings', { otherSettings });
};

export const getHermesMemory = async (kind: HermesMemoryKind): Promise<string> => {
  return await invoke<string>('get_hermes_memory', { kind });
};

export const setHermesMemory = async (
  kind: HermesMemoryKind,
  content: string,
): Promise<void> => {
  await invoke('set_hermes_memory', { kind, content });
};

export const getHermesMemoryLimits = async (): Promise<HermesMemoryLimits> => {
  return await invoke<HermesMemoryLimits>('get_hermes_memory_limits');
};

export const setHermesMemoryEnabled = async (
  kind: HermesMemoryKind,
  enabled: boolean,
): Promise<HermesMemoryLimits> => {
  return await invoke<HermesMemoryLimits>('set_hermes_memory_enabled', { kind, enabled });
};
