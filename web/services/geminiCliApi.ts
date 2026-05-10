import { invoke } from '@tauri-apps/api/core';
import type {
  ConfigPathInfo,
  GeminiCliCommonConfig,
  GeminiCliCommonConfigInput,
  GeminiCliLocalConfigInput,
  GeminiCliOfficialAccount,
  GeminiCliProvider,
  GeminiCliProviderInput,
  GeminiCliSettings,
} from '@/types/geminicli';

export const getGeminiCliConfigPath = async (): Promise<string> => {
  return await invoke<string>('get_gemini_cli_config_path');
};

export const getGeminiCliRootPathInfo = async (): Promise<ConfigPathInfo> => {
  return await invoke<ConfigPathInfo>('get_gemini_cli_root_path_info');
};

export const revealGeminiCliConfigFolder = async (): Promise<void> => {
  await invoke('reveal_gemini_cli_config_folder');
};

export const readGeminiCliSettings = async (): Promise<GeminiCliSettings> => {
  return await invoke<GeminiCliSettings>('read_gemini_cli_settings');
};

export const listGeminiCliProviders = async (): Promise<GeminiCliProvider[]> => {
  return await invoke<GeminiCliProvider[]>('list_gemini_cli_providers');
};

export const createGeminiCliProvider = async (
  provider: GeminiCliProviderInput,
): Promise<GeminiCliProvider> => {
  return await invoke<GeminiCliProvider>('create_gemini_cli_provider', { provider });
};

export const updateGeminiCliProvider = async (
  provider: GeminiCliProvider,
): Promise<GeminiCliProvider> => {
  return await invoke<GeminiCliProvider>('update_gemini_cli_provider', { provider });
};

export const deleteGeminiCliProvider = async (providerId: string): Promise<void> => {
  await invoke('delete_gemini_cli_provider', { id: providerId });
};

export const reorderGeminiCliProviders = async (providerIds: string[]): Promise<void> => {
  await invoke('reorder_gemini_cli_providers', { ids: providerIds });
};

export const selectGeminiCliProvider = async (
  providerId: string,
): Promise<void> => {
  await invoke('select_gemini_cli_provider', { id: providerId });
};

export const listGeminiCliOfficialAccounts = async (
  providerId: string,
): Promise<GeminiCliOfficialAccount[]> => {
  return await invoke<GeminiCliOfficialAccount[]>('list_gemini_cli_official_accounts', {
    providerId,
  });
};

export const startGeminiCliOfficialAccountOauth = async (
  providerId: string,
): Promise<GeminiCliOfficialAccount> => {
  return await invoke<GeminiCliOfficialAccount>('start_gemini_cli_official_account_oauth', {
    providerId,
  });
};

export const saveGeminiCliOfficialLocalAccount = async (
  providerId: string,
): Promise<GeminiCliOfficialAccount> => {
  return await invoke<GeminiCliOfficialAccount>('save_gemini_cli_official_local_account', {
    providerId,
  });
};

export const applyGeminiCliOfficialAccount = async (
  providerId: string,
  accountId: string,
): Promise<void> => {
  await invoke('apply_gemini_cli_official_account', { providerId, accountId });
};

export const deleteGeminiCliOfficialAccount = async (
  providerId: string,
  accountId: string,
): Promise<void> => {
  await invoke('delete_gemini_cli_official_account', { providerId, accountId });
};

export const refreshGeminiCliOfficialAccountLimits = async (
  providerId: string,
  accountId: string,
): Promise<GeminiCliOfficialAccount> => {
  return await invoke<GeminiCliOfficialAccount>('refresh_gemini_cli_official_account_limits', {
    providerId,
    accountId,
  });
};

export const copyGeminiCliOfficialAccountToken = async (
  providerId: string,
  accountId: string,
  tokenKind: 'access' | 'refresh',
): Promise<void> => {
  await invoke('copy_gemini_cli_official_account_token', {
    input: {
      providerId,
      accountId,
      tokenKind,
    },
  });
};

export const toggleGeminiCliProviderDisabled = async (
  providerId: string,
  isDisabled: boolean,
): Promise<void> => {
  await invoke('toggle_gemini_cli_provider_disabled', { providerId, isDisabled });
};

export const getGeminiCliCommonConfig = async (): Promise<GeminiCliCommonConfig | null> => {
  return await invoke<GeminiCliCommonConfig | null>('get_gemini_cli_common_config');
};

export const extractGeminiCliCommonConfigFromCurrentFile =
  async (): Promise<GeminiCliCommonConfig> => {
    return await invoke<GeminiCliCommonConfig>('extract_gemini_cli_common_config_from_current_file');
  };

export const saveGeminiCliCommonConfig = async (
  input: GeminiCliCommonConfigInput,
): Promise<void> => {
  await invoke('save_gemini_cli_common_config', { input });
};

export const saveGeminiCliLocalConfig = async (
  input: GeminiCliLocalConfigInput,
): Promise<GeminiCliProvider> => {
  return await invoke<GeminiCliProvider>('save_gemini_cli_local_config', { input });
};
