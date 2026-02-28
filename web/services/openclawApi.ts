/**
 * OpenClaw API Service
 *
 * Handles all OpenClaw configuration related communication with the Tauri backend.
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  OpenClawConfig,
  OpenClawConfigPathInfo,
  OpenClawCommonConfig,
  OpenClawAgentsDefaults,
  OpenClawEnvConfig,
  OpenClawToolsConfig,
  ReadOpenClawConfigResult,
} from '@/types/openclaw';

/**
 * Get OpenClaw configuration file path
 */
export const getOpenClawConfigPath = async (): Promise<string> => {
  return await invoke<string>('get_openclaw_config_path');
};

/**
 * Get OpenClaw configuration path info including source
 */
export const getOpenClawConfigPathInfo = async (): Promise<OpenClawConfigPathInfo> => {
  return await invoke<OpenClawConfigPathInfo>('get_openclaw_config_path_info');
};

/**
 * Read OpenClaw configuration file with detailed result
 */
export const readOpenClawConfigWithResult = async (): Promise<ReadOpenClawConfigResult> => {
  return await invoke<ReadOpenClawConfigResult>('read_openclaw_config');
};

/**
 * Read OpenClaw configuration file (returns null on not found)
 */
export const readOpenClawConfig = async (): Promise<OpenClawConfig | null> => {
  const result = await readOpenClawConfigWithResult();
  if (result.status === 'success') {
    return result.config;
  }
  return null;
};

/**
 * Save OpenClaw configuration file (full replacement)
 */
export const saveOpenClawConfig = async (config: OpenClawConfig): Promise<void> => {
  await invoke('save_openclaw_config', { config });
};

/**
 * Backup OpenClaw configuration file
 * @returns The backup file path
 */
export const backupOpenClawConfig = async (): Promise<string> => {
  return await invoke<string>('backup_openclaw_config');
};

/**
 * Get OpenClaw common config from database
 */
export const getOpenClawCommonConfig = async (): Promise<OpenClawCommonConfig | null> => {
  return await invoke<OpenClawCommonConfig | null>('get_openclaw_common_config');
};

/**
 * Save OpenClaw common config to database
 */
export const saveOpenClawCommonConfig = async (config: OpenClawCommonConfig): Promise<void> => {
  await invoke('save_openclaw_common_config', { config });
};

/**
 * Get agents.defaults section from config
 */
export const getOpenClawAgentsDefaults = async (): Promise<OpenClawAgentsDefaults | null> => {
  return await invoke<OpenClawAgentsDefaults | null>('get_openclaw_agents_defaults');
};

/**
 * Set agents.defaults section in config (read-modify-write)
 */
export const setOpenClawAgentsDefaults = async (defaults: OpenClawAgentsDefaults): Promise<void> => {
  await invoke('set_openclaw_agents_defaults', { defaults });
};

/**
 * Get env section from config
 */
export const getOpenClawEnv = async (): Promise<OpenClawEnvConfig | null> => {
  return await invoke<OpenClawEnvConfig | null>('get_openclaw_env');
};

/**
 * Set env section in config (read-modify-write)
 */
export const setOpenClawEnv = async (env: OpenClawEnvConfig): Promise<void> => {
  await invoke('set_openclaw_env', { env });
};

/**
 * Get tools section from config
 */
export const getOpenClawTools = async (): Promise<OpenClawToolsConfig | null> => {
  return await invoke<OpenClawToolsConfig | null>('get_openclaw_tools');
};

/**
 * Set tools section in config (read-modify-write)
 */
export const setOpenClawTools = async (tools: OpenClawToolsConfig): Promise<void> => {
  await invoke('set_openclaw_tools', { tools });
};
