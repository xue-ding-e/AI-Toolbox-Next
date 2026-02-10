/**
 * SSH Sync Hook
 *
 * Manages SSH sync configuration and operations
 */

import { useState, useEffect, useCallback, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { SSHSyncConfig, SSHStatusResult, SyncResult, SSHFileMapping, SyncProgress } from '@/types/sshsync';
import {
  sshGetConfig,
  sshSaveConfig,
  sshSync,
  sshGetStatus,
  sshGetDefaultMappings,
} from '@/services/sshSyncApi';

export function useSSHSync() {
  const [config, setConfig] = useState<SSHSyncConfig | null>(null);
  const [status, setStatus] = useState<SSHStatusResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [syncWarning, setSyncWarning] = useState<string | null>(null);
  const [syncProgress, setSyncProgress] = useState<SyncProgress | null>(null);

  const skipNextReload = useRef(false);

  /**
   * Load SSH sync configuration
   */
  const loadConfig = useCallback(async () => {
    if (skipNextReload.current) {
      skipNextReload.current = false;
      return;
    }

    try {
      setLoading(true);
      const data = await sshGetConfig();

      // If no file mappings exist, initialize with defaults
      if (!data.fileMappings || data.fileMappings.length === 0) {
        const defaultMappings = await sshGetDefaultMappings();
        const defaultConfig: SSHSyncConfig = {
          ...data,
          fileMappings: defaultMappings,
        };
        skipNextReload.current = true;
        await sshSaveConfig(defaultConfig);
        setConfig(defaultConfig);
      } else {
        setConfig(data);
      }
    } catch (error) {
      console.error('Failed to load SSH config:', error);
    } finally {
      setLoading(false);
    }
  }, []);

  /**
   * Load SSH sync status
   */
  const loadStatus = useCallback(async () => {
    try {
      const data = await sshGetStatus();
      setStatus(data);
    } catch (error) {
      console.error('Failed to load SSH status:', error);
    }
  }, []);

  /**
   * Save SSH sync configuration
   */
  const saveConfig = useCallback(async (newConfig: SSHSyncConfig) => {
    try {
      setLoading(true);
      await sshSaveConfig(newConfig);
      setConfig(newConfig);
      await loadStatus();
    } catch (error) {
      console.error('Failed to save SSH config:', error);
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
      setSyncProgress(null);
      const result = await sshSync(module);
      await loadStatus();
      return result;
    } catch (error) {
      console.error('Failed to sync:', error);
      throw error;
    } finally {
      setSyncing(false);
      setSyncProgress(null);
    }
  }, [loadStatus]);

  /**
   * Get default file mappings
   */
  const getDefaultMappings = useCallback(async (): Promise<SSHFileMapping[]> => {
    try {
      return await sshGetDefaultMappings();
    } catch (error) {
      console.error('Failed to get default mappings:', error);
      throw error;
    }
  }, []);

  // Load config and status on mount
  useEffect(() => {
    loadConfig();
    loadStatus();
  }, [loadConfig, loadStatus]);

  // Listen to SSH events
  useEffect(() => {
    const unlistenConfig = listen('ssh-config-changed', () => {
      loadConfig();
      loadStatus();
    });

    const unlistenSync = listen<SyncResult>('ssh-sync-completed', () => {
      loadStatus();
      setSyncProgress(null);
    });

    const unlistenWarning = listen<string>('ssh-sync-warning', (event) => {
      setSyncWarning(event.payload);
    });

    const unlistenProgress = listen<SyncProgress>('ssh-sync-progress', (event) => {
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
    syncWarning,
    syncProgress,
    loadConfig,
    loadStatus,
    saveConfig,
    sync,
    getDefaultMappings,
    dismissSyncWarning,
  };
}
