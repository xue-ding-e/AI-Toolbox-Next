import { invoke } from '@tauri-apps/api/core';
import type {
  OmpProviderInput,
  OmpRuntimeConfig,
  OmpSettingsConfig,
  OmpSettingsConfigInput,
} from '@/types/ohMyPi';

export const getOmpSettingsConfig = async (): Promise<OmpSettingsConfig | null> =>
  invoke<OmpSettingsConfig | null>('get_omp_settings_config');

export const saveOmpSettingsConfig = async (input: OmpSettingsConfigInput): Promise<void> => {
  await invoke('save_omp_settings_config', { input });
};

export const readOmpRuntimeConfig = async (): Promise<OmpRuntimeConfig> =>
  invoke<OmpRuntimeConfig>('read_omp_runtime_config');

export const saveOmpProvider = async (input: OmpProviderInput): Promise<OmpRuntimeConfig> =>
  invoke<OmpRuntimeConfig>('save_omp_provider', { input });

export const deleteOmpProvider = async (providerKey: string): Promise<OmpRuntimeConfig> =>
  invoke<OmpRuntimeConfig>('delete_omp_provider', { providerKey });
