/**
 * Codex API Service
 *
 * Handles all Codex configuration related communication with the Tauri backend.
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  CodexProvider,
  CodexCommonConfig,
  CodexSettings,
} from '@/types/codex';

/**
 * Get Codex config directory path
 */
export const getCodexConfigPath = async (): Promise<string> => {
  return await invoke<string>('get_codex_config_dir_path');
};

/**
 * Get Codex config.toml file path
 */
export const getCodexConfigFilePath = async (): Promise<string> => {
  return await invoke<string>('get_codex_config_file_path');
};

/**
 * Reveal Codex config folder in file explorer
 */
export const revealCodexConfigFolder = async (): Promise<void> => {
  await invoke('reveal_codex_config_folder');
};

/**
 * List all Codex providers
 */
export const listCodexProviders = async (): Promise<CodexProvider[]> => {
  return await invoke<CodexProvider[]>('list_codex_providers');
};

/**
 * Create a new Codex provider
 */
export const createCodexProvider = async (
  provider: Omit<CodexProvider, 'createdAt' | 'updatedAt'>
): Promise<CodexProvider> => {
  return await invoke<CodexProvider>('create_codex_provider', { provider });
};

/**
 * Update an existing Codex provider
 */
export const updateCodexProvider = async (
  provider: CodexProvider
): Promise<CodexProvider> => {
  return await invoke<CodexProvider>('update_codex_provider', { provider });
};

/**
 * Delete a Codex provider
 */
export const deleteCodexProvider = async (id: string): Promise<void> => {
  await invoke('delete_codex_provider', { id });
};

/**
 * Select a Codex provider
 */
export const selectCodexProvider = async (id: string): Promise<void> => {
  await invoke('select_codex_provider', { id });
};

/**
 * Apply Codex configuration
 */
export const applyCodexConfig = async (providerId: string): Promise<void> => {
  await invoke('apply_codex_config', { providerId });
};

/**
 * Read Codex settings from files
 */
export const readCodexSettings = async (): Promise<CodexSettings> => {
  return await invoke<CodexSettings>('read_codex_settings');
};

/**
 * Get common configuration
 */
export const getCodexCommonConfig = async (): Promise<CodexCommonConfig | null> => {
  return await invoke<CodexCommonConfig | null>('get_codex_common_config');
};

/**
 * Save common configuration
 */
export const saveCodexCommonConfig = async (config: string): Promise<void> => {
  await invoke('save_codex_common_config', { config });
};
