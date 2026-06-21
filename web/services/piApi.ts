import { invoke } from '@tauri-apps/api/core';
import type {
  PiAuthProviderInput,
  PiDeleteScope,
  PiModelSettingsInput,
  PiModelsProviderInput,
  PiPathInfo,
  PiRuntimeConfig,
  PiSettingsConfig,
  PiSettingsConfigInput,
} from '@/types/pi';

export const getPiRootPathInfo = async (): Promise<PiPathInfo> => {
  return await invoke<PiPathInfo>('get_pi_root_path_info');
};

export const getPiSettingsConfig = async (): Promise<PiSettingsConfig | null> => {
  return await invoke<PiSettingsConfig | null>('get_pi_settings_config');
};

export const savePiSettingsConfig = async (
  input: PiSettingsConfigInput,
): Promise<void> => {
  await invoke('save_pi_settings_config', { input });
};

export const readPiRuntimeConfig = async (): Promise<PiRuntimeConfig> => {
  return await invoke<PiRuntimeConfig>('read_pi_runtime_config');
};

export const savePiModelSettings = async (
  input: PiModelSettingsInput,
): Promise<PiRuntimeConfig> => {
  return await invoke<PiRuntimeConfig>('save_pi_model_settings', { input });
};

export const savePiOtherSettings = async (
  otherSettings: Record<string, unknown>,
): Promise<PiRuntimeConfig> => {
  return await invoke<PiRuntimeConfig>('save_pi_other_settings', { otherSettings });
};

export const savePiAuthProvider = async (
  input: PiAuthProviderInput,
): Promise<PiRuntimeConfig> => {
  return await invoke<PiRuntimeConfig>('save_pi_auth_provider', { input });
};

export const savePiModelsProvider = async (
  input: PiModelsProviderInput,
): Promise<PiRuntimeConfig> => {
  return await invoke<PiRuntimeConfig>('save_pi_models_provider', { input });
};

export const deletePiRuntimeProvider = async (
  providerKey: string,
  scope: PiDeleteScope,
): Promise<PiRuntimeConfig> => {
  return await invoke<PiRuntimeConfig>('delete_pi_runtime_provider', { providerKey, scope });
};
