/**
 * OpenCode API Service
 *
 * Handles all OpenCode configuration related communication with the Tauri backend.
 */

import { invoke } from '@tauri-apps/api/core';
import type { OpenCodeConfig } from '@/types/opencode';

/**
 * Configuration path information
 */
export interface ConfigPathInfo {
  path: string;
  source: 'custom' | 'env' | 'shell' | 'default';
}

/**
 * OpenCode common configuration
 */
export interface OpenCodeCommonConfig {
  configPath: string | null;
  updatedAt: string;
}

/**
 * Get OpenCode configuration file path
 */
export const getOpenCodeConfigPath = async (): Promise<string> => {
  return await invoke<string>('get_opencode_config_path');
};

/**
 * Get OpenCode configuration path info including source
 */
export const getOpenCodeConfigPathInfo = async (): Promise<ConfigPathInfo> => {
  return await invoke<ConfigPathInfo>('get_opencode_config_path_info');
};

/**
 * Read OpenCode configuration file
 */
export const readOpenCodeConfig = async (): Promise<OpenCodeConfig | null> => {
  return await invoke<OpenCodeConfig | null>('read_opencode_config');
};

/**
 * Save OpenCode configuration file
 */
export const saveOpenCodeConfig = async (config: OpenCodeConfig): Promise<void> => {
  await invoke('save_opencode_config', { config });
};

/**
 * Get OpenCode common config
 */
export const getOpenCodeCommonConfig = async (): Promise<OpenCodeCommonConfig | null> => {
  return await invoke<OpenCodeCommonConfig | null>('get_opencode_common_config');
};

/**
 * Save OpenCode common config
 */
export const saveOpenCodeCommonConfig = async (config: OpenCodeCommonConfig): Promise<void> => {
  await invoke('save_opencode_common_config', { config });
};
