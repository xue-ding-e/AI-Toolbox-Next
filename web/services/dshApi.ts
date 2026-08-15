import { invoke } from '@tauri-apps/api/core';
import type { AllApiHubProviderItem, AllApiHubProvidersResult } from '@/types/allApiHub';
import type {
  DshCredentialInput,
  DshModelSettingsInput,
  DshModelsProviderInput,
  DshPathInfo,
  DshRuntimeConfig,
  DshSettingsConfig,
  DshSettingsConfigInput,
} from '@/types/dsh';

export const getDshPathInfo = async (): Promise<DshPathInfo> => {
  return await invoke<DshPathInfo>('get_dsh_path_info');
};

export const getDshSettingsConfig = async (): Promise<DshSettingsConfig | null> => {
  return await invoke<DshSettingsConfig | null>('get_dsh_settings_config');
};

export const saveDshSettingsConfig = async (
  input: DshSettingsConfigInput,
): Promise<void> => {
  await invoke('save_dsh_settings_config', { input });
};

export const readDshRuntimeConfig = async (): Promise<DshRuntimeConfig> => {
  return await invoke<DshRuntimeConfig>('read_dsh_runtime_config');
};

export const saveDshModelSettings = async (
  input: DshModelSettingsInput,
): Promise<DshRuntimeConfig> => {
  return await invoke<DshRuntimeConfig>('save_dsh_model_settings', { input });
};

export const saveDshOtherSettings = async (
  otherSettings: Record<string, unknown>,
): Promise<DshRuntimeConfig> => {
  return await invoke<DshRuntimeConfig>('save_dsh_other_settings', { otherSettings });
};

export const saveDshModelsProvider = async (
  input: DshModelsProviderInput,
): Promise<DshRuntimeConfig> => {
  return await invoke<DshRuntimeConfig>('save_dsh_models_provider', { input });
};

export const saveDshCredential = async (
  input: DshCredentialInput,
): Promise<DshRuntimeConfig> => {
  return await invoke<DshRuntimeConfig>('save_dsh_credential', { input });
};

export const getDshCredentialValue = async (refName: string): Promise<string | null> => {
  return await invoke<string | null>('get_dsh_credential_value', { refName });
};

export const deleteDshCredential = async (
  refName: string,
): Promise<DshRuntimeConfig> => {
  return await invoke<DshRuntimeConfig>('delete_dsh_credential', { refName });
};

export const deleteDshRuntimeProvider = async (
  providerKey: string,
): Promise<DshRuntimeConfig> => {
  return await invoke<DshRuntimeConfig>('delete_dsh_runtime_provider', { providerKey });
};
export const listDshAllApiHubProviders = async (): Promise<AllApiHubProvidersResult> => {
  return await invoke<AllApiHubProvidersResult>('list_dsh_all_api_hub_providers');
};

export const resolveDshAllApiHubProviders = async (
  providerIds: string[]
): Promise<AllApiHubProviderItem[]> => {
  return await invoke<AllApiHubProviderItem[]>('resolve_dsh_all_api_hub_providers', {
    request: { providerIds },
  });
};
