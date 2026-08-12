import { invoke } from '@tauri-apps/api/core';
import type {
  OmpExtensionActionInput,
  OmpExtensionCommandResult,
  OmpExtensionInstallInput,
  OmpExtensionListResult,
  OmpExtensionUpdateInput,
  OmpModelSettingsInput,
  OmpModelsProviderInput,
  OmpPathInfo,
  OmpRuntimeConfig,
  OmpSettingsConfig,
  OmpSettingsConfigInput,
} from '@/types/ohMyPi';

export const getOmpRootPathInfo = async (): Promise<OmpPathInfo> => {
  return await invoke<OmpPathInfo>('get_omp_root_path_info');
};

export const getOmpSettingsConfig = async (): Promise<OmpSettingsConfig | null> => {
  return await invoke<OmpSettingsConfig | null>('get_omp_settings_config');
};

export const saveOmpSettingsConfig = async (
  input: OmpSettingsConfigInput,
): Promise<void> => {
  await invoke('save_omp_settings_config', { input });
};

export const readOmpRuntimeConfig = async (): Promise<OmpRuntimeConfig> => {
  return await invoke<OmpRuntimeConfig>('read_omp_runtime_config');
};

export const saveOmpModelSettings = async (
  input: OmpModelSettingsInput,
): Promise<OmpRuntimeConfig> => {
  return await invoke<OmpRuntimeConfig>('save_omp_model_settings', { input });
};

export const saveOmpOtherSettings = async (
  otherSettings: Record<string, unknown>,
): Promise<OmpRuntimeConfig> => {
  return await invoke<OmpRuntimeConfig>('save_omp_other_settings', { otherSettings });
};

export const saveOmpModelsProvider = async (
  input: OmpModelsProviderInput,
): Promise<OmpRuntimeConfig> => {
  return await invoke<OmpRuntimeConfig>('save_omp_models_provider', { input });
};

export const deleteOmpRuntimeProvider = async (
  providerKey: string,
): Promise<OmpRuntimeConfig> => {
  return await invoke<OmpRuntimeConfig>('delete_omp_runtime_provider', { providerKey });
};

export const listOmpExtensions = async (): Promise<OmpExtensionListResult> => {
  return await invoke<OmpExtensionListResult>('list_omp_extensions');
};

export const installOmpExtension = async (
  input: OmpExtensionInstallInput,
): Promise<OmpExtensionCommandResult> => {
  return await invoke<OmpExtensionCommandResult>('install_omp_extension', { input });
};

export const uninstallOmpExtension = async (
  input: OmpExtensionActionInput,
): Promise<OmpExtensionCommandResult> => {
  return await invoke<OmpExtensionCommandResult>('uninstall_omp_extension', { input });
};

export const updateOmpExtensions = async (
  input?: OmpExtensionUpdateInput,
): Promise<OmpExtensionCommandResult> => {
  return await invoke<OmpExtensionCommandResult>('update_omp_extensions', { input });
};