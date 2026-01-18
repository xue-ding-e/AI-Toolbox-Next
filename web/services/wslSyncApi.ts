/**
 * WSL Sync API Service
 *
 * Handles all WSL sync configuration related communication with the Tauri backend.
 */

import { invoke } from '@tauri-apps/api/core';
import type {
  WSLDetectResult,
  WSLErrorResult,
  WSLSyncConfig,
  WSLStatusResult,
  FileMapping,
  SyncResult,
} from '@/types/wslsync';

/**
 * Detect WSL availability and get distro list
 */
export const wslDetect = async (): Promise<WSLDetectResult> => {
  return await invoke<WSLDetectResult>('wsl_detect');
};

/**
 * Check if a specific WSL distro is available
 */
export const wslCheckDistro = async (distro: string): Promise<WSLErrorResult> => {
  return await invoke<WSLErrorResult>('wsl_check_distro', { distro });
};

/**
 * Get WSL sync configuration
 */
export const wslGetConfig = async (): Promise<WSLSyncConfig> => {
  return await invoke<WSLSyncConfig>('wsl_get_config');
};

/**
 * Save WSL sync configuration
 */
export const wslSaveConfig = async (config: WSLSyncConfig): Promise<void> => {
  await invoke('wsl_save_config', { config });
};

/**
 * Add a new file mapping
 */
export const wslAddFileMapping = async (mapping: FileMapping): Promise<void> => {
  await invoke('wsl_add_file_mapping', { mapping });
};

/**
 * Update an existing file mapping
 */
export const wslUpdateFileMapping = async (mapping: FileMapping): Promise<void> => {
  await invoke('wsl_update_file_mapping', { mapping });
};

/**
 * Delete a file mapping
 */
export const wslDeleteFileMapping = async (id: string): Promise<void> => {
  await invoke('wsl_delete_file_mapping', { id });
};

/**
 * Reset all file mappings (delete all)
 */
export const wslResetFileMappings = async (): Promise<void> => {
  await invoke('wsl_reset_file_mappings');
};

/**
 * Sync all files or specific module to WSL
 */
export const wslSync = async (module?: string): Promise<SyncResult> => {
  return await invoke<SyncResult>('wsl_sync', { module });
};

/**
 * Get current WSL sync status
 */
export const wslGetStatus = async (): Promise<WSLStatusResult> => {
  return await invoke<WSLStatusResult>('wsl_get_status');
};

/**
 * Test if a Windows path exists and can be accessed
 */
export const wslTestPath = async (windowsPath: string): Promise<boolean> => {
  return await invoke<boolean>('wsl_test_path', { windowsPath });
};

/**
 * Get default file mappings
 */
export const wslGetDefaultMappings = async (): Promise<FileMapping[]> => {
  return await invoke<FileMapping[]>('wsl_get_default_mappings');
};
