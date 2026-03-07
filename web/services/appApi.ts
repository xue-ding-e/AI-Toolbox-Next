/**
 * App API Service
 *
 * Handles app-level operations like version info and updates.
 */

import { getVersion } from '@tauri-apps/api/app';
import { openUrl as openUrlExternal } from '@tauri-apps/plugin-opener';
import { invoke } from '@tauri-apps/api/core';
import { PRESET_MODELS_REMOTE_URL, updatePresetModels } from '@/constants/presetModels';
import type { PresetModel } from '@/constants/presetModels';

const GITHUB_REPO = 'coulsontl/ai-toolbox';
export { GITHUB_REPO };
const GITHUB_URL = `https://github.com/${GITHUB_REPO}`;

export interface UpdateInfo {
  hasUpdate: boolean;
  currentVersion: string;
  latestVersion: string;
  releaseUrl: string;
  releaseNotes: string;
  signature?: string;
  url?: string;
}

interface UpdateCheckResult {
  has_update: boolean;
  current_version: string;
  latest_version: string;
  release_url: string;
  release_notes: string;
  signature?: string;
  url?: string;
}

/**
 * Get current app version
 */
export const getAppVersion = async (): Promise<string> => {
  return await getVersion();
};

/**
 * Check for updates from GitHub releases (via Tauri backend)
 */
export const checkForUpdates = async (): Promise<UpdateInfo> => {
  const result = await invoke<UpdateCheckResult>('check_for_updates');

  return {
    hasUpdate: result.has_update,
    currentVersion: result.current_version,
    latestVersion: result.latest_version,
    releaseUrl: result.release_url,
    releaseNotes: result.release_notes,
    signature: result.signature,
    url: result.url,
  };
};

/**
 * Install the update if available
 */
export const installUpdate = async (): Promise<boolean> => {
  return await invoke('install_update');
};

/**
 * Open GitHub repository page
 */
export const openGitHubPage = async (): Promise<void> => {
  await openUrlExternal(GITHUB_URL);
};

/**
 * Open a URL in the default browser
 */
export const openExternalUrl = async (url: string): Promise<void> => {
  await openUrlExternal(url);
};

/**
 * Refresh the system tray menu
 */
export const refreshTrayMenu = async (): Promise<void> => {
  await invoke('refresh_tray_menu');
};

export const hasAllApiHubExtension = async (): Promise<boolean> => {
  return await invoke<boolean>('has_all_api_hub_extension');
};

/**
 * Set window background color (affects macOS titlebar color)
 */
export const setWindowBackgroundColor = async (r: number, g: number, b: number): Promise<void> => {
  await invoke('set_window_background_color', { r, g, b });
};

/**
 * Load preset models from local cache file (app data dir).
 * Returns true if the cache was found and applied, false otherwise.
 */
export const loadCachedPresetModels = async (): Promise<boolean> => {
  try {
    const json = await invoke<Record<string, PresetModel[]> | null>(
      'load_cached_preset_models',
    );
    if (json && typeof json === 'object') {
      updatePresetModels(json);
      console.log('[PresetModels] Loaded from local cache');
      return true;
    }
  } catch (err) {
    console.warn('[PresetModels] Failed to load local cache:', err);
  }
  return false;
};

/**
 * Fetch preset models from the remote repository, save to local
 * cache file, and update the in-memory PRESET_MODELS map.
 * Silently falls back to bundled defaults on network or parse errors.
 */
export const fetchRemotePresetModels = async (): Promise<void> => {
  try {
    const json = await invoke<Record<string, PresetModel[]>>(
      'fetch_remote_preset_models',
      { url: PRESET_MODELS_REMOTE_URL },
    );
    if (json && typeof json === 'object') {
      updatePresetModels(json);
      console.log('[PresetModels] Updated from remote');
    }
  } catch (err) {
    console.warn('[PresetModels] Failed to fetch remote, using bundled defaults:', err);
  }
};
