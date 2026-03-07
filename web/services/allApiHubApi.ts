import { invoke } from '@tauri-apps/api/core';

export interface AllApiHubProviderModelsResult {
  providerId: string;
  models: string[];
  status: 'loaded' | 'error' | 'unsupported';
  error?: string;
}

export const getAllApiHubProviderModels = async (
  providerIds: string[]
): Promise<AllApiHubProviderModelsResult[]> => {
  return await invoke<AllApiHubProviderModelsResult[]>('get_all_api_hub_provider_models', {
    request: { providerIds },
  });
};
