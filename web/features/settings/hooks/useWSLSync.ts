/**
 * WSL Sync Hook
 *
 * Manages WSL sync configuration and operations
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import type {
  WSLSyncConfig,
  WSLStatusResult,
  SyncResult,
  FileMapping,
  WSLDetectResult,
  SyncProgress,
  WslDirectModuleStatus,
} from '@/types/wslsync';
import {
  wslGetConfig,
  wslSaveConfig,
  wslSync,
  wslGetStatus,
  wslDetect,
  wslCheckDistro,
  wslGetDefaultMappings,
} from '@/services/wslSyncApi';
import { useSettingsStore } from '@/stores';

// Map visibleTabs keys to sync module keys
const TAB_TO_MODULE: Record<string, string> = {
  opencode: 'opencode',
  claudecode: 'claude',
  codex: 'codex',
  openclaw: 'openclaw',
  geminicli: 'geminicli',
};
const ALL_CODING_MODULES = ['opencode', 'claude', 'codex', 'geminicli', 'openclaw'];

export function useWSLSync() {
  const [config, setConfig] = useState<WSLSyncConfig | null>(null);
  const [status, setStatus] = useState<WSLStatusResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [syncWarning, setSyncWarning] = useState<string | null>(null);
  const [syncProgress, setSyncProgress] = useState<SyncProgress | null>(null);

  // Flag to prevent reload after we just saved defaults
  const skipNextReload = useRef(false);

  /**
   * Load WSL sync configuration
   */
  const loadConfig = useCallback(async () => {
    // Skip if we just saved defaults to prevent loop
    if (skipNextReload.current) {
      skipNextReload.current = false;
      return;
    }

    try {
      setLoading(true);
      setLoadError(null);
      const data = await wslGetConfig();

      // If no file mappings exist, initialize with defaults
      if (!data.fileMappings || data.fileMappings.length === 0) {
        const defaultMappings = await wslGetDefaultMappings();
        const defaultConfig: WSLSyncConfig = {
          ...data,
          fileMappings: defaultMappings,
        };
        // Set flag to skip the next reload triggered by save
        skipNextReload.current = true;
        await wslSaveConfig(defaultConfig);
        setConfig(defaultConfig);
      } else {
        setConfig(data);
      }
    } catch (error) {
      console.error('Failed to load WSL config:', error);
      setLoadError(error instanceof Error ? error.message : String(error));
    } finally {
      setLoading(false);
    }
  }, []);

  /**
   * Load WSL sync status
   */
  const loadStatus = useCallback(async () => {
    try {
      setLoadError(null);
      const data = await wslGetStatus();
      setStatus(data);
    } catch (error) {
      console.error('Failed to load WSL status:', error);
      setLoadError(error instanceof Error ? error.message : String(error));
    }
  }, []);

  /**
   * Save WSL sync configuration
   */
  const saveConfig = useCallback(async (newConfig: WSLSyncConfig) => {
    try {
      setLoading(true);
      await wslSaveConfig(newConfig);
      setConfig(newConfig);
      await loadStatus();
    } catch (error) {
      console.error('Failed to save WSL config:', error);
      throw error;
    } finally {
      setLoading(false);
    }
  }, [loadStatus]);

  /**
   * Execute sync operation
   */
  const sync = useCallback(async (module?: string) => {
    try {
      setSyncing(true);
      setSyncProgress(null); // Clear previous progress
      // Compute skip modules from visibleTabs
      const { visibleTabs } = useSettingsStore.getState();
      const visibleModules = visibleTabs
        .map((k) => TAB_TO_MODULE[k])
        .filter(Boolean);
      const wslDirectModules = (config?.moduleStatuses || [])
        .filter((item) => item.isWslDirect)
        .map((item) => item.module);
      const skipModules = ALL_CODING_MODULES.filter(
        (m) => !visibleModules.includes(m) || wslDirectModules.includes(m)
      );
      const result = await wslSync(module, skipModules.length > 0 ? skipModules : undefined);
      await loadStatus();
      return result;
    } catch (error) {
      console.error('Failed to sync:', error);
      throw error;
    } finally {
      setSyncing(false);
      setSyncProgress(null); // Clear progress when done
    }
  }, [config?.moduleStatuses, loadStatus]);

  /**
   * Detect WSL availability
   */
  const detect = useCallback(async (): Promise<WSLDetectResult> => {
    try {
      return await wslDetect();
    } catch (error) {
      console.error('Failed to detect WSL:', error);
      throw error;
    }
  }, []);

  /**
   * Check if a specific distro is available
   */
  const checkDistro = useCallback(async (distro: string) => {
    try {
      return await wslCheckDistro(distro);
    } catch (error) {
      console.error('Failed to check distro:', error);
      throw error;
    }
  }, []);

  /**
   * Get default file mappings
   */
  const getDefaultMappings = useCallback(async (): Promise<FileMapping[]> => {
    try {
      return await wslGetDefaultMappings();
    } catch (error) {
      console.error('Failed to get default mappings:', error);
      throw error;
    }
  }, []);

  /**
   * Initialize default configuration if not exists
   */
  const initializeDefaultConfig = useCallback(async () => {
    try {
      setLoading(true);
      const defaultMappings = await getDefaultMappings();
        const defaultConfig: WSLSyncConfig = {
          enabled: false,
          distro: 'Ubuntu',
          syncMcp: true,
          syncSkills: true,
          fileMappings: defaultMappings,
          moduleStatuses: [],
          lastSyncStatus: 'never',
        };
      await wslSaveConfig(defaultConfig);
      setConfig(defaultConfig);
    } catch (error) {
      console.error('Failed to initialize default config:', error);
      throw error;
    } finally {
      setLoading(false);
    }
  }, [getDefaultMappings]);

  // Load config and status on mount
  useEffect(() => {
    loadConfig();
    loadStatus();
  }, [loadConfig, loadStatus]);

  // Listen to WSL config changes
  useEffect(() => {
    const unlistenConfig = listen('wsl-config-changed', () => {
      loadConfig();
      loadStatus();
    });

    const unlistenSync = listen<SyncResult>('wsl-sync-completed', () => {
      loadStatus();
      setSyncProgress(null); // Clear progress when sync completes
    });

    const unlistenWarning = listen<string>('wsl-sync-warning', (event) => {
      setSyncWarning(event.payload);
    });

    const unlistenProgress = listen<SyncProgress>('wsl-sync-progress', (event) => {
      setSyncProgress(event.payload);
    });

    return () => {
      unlistenConfig.then(fn => fn());
      unlistenSync.then(fn => fn());
      unlistenWarning.then(fn => fn());
      unlistenProgress.then(fn => fn());
    };
  }, [loadConfig, loadStatus]);

  /**
   * Dismiss sync warning
   */
  const dismissSyncWarning = useCallback(() => {
    setSyncWarning(null);
  }, []);

  return {
    config,
    status,
    loading,
    syncing,
    loadError,
    syncWarning,
    syncProgress,
    moduleStatuses: config?.moduleStatuses || status?.moduleStatuses || ([] as WslDirectModuleStatus[]),
    loadConfig,
    loadStatus,
    saveConfig,
    sync,
    detect,
    checkDistro,
    getDefaultMappings,
    initializeDefaultConfig,
    dismissSyncWarning,
  };
}
