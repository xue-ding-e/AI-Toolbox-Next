/**
 * WSL Sync Types
 */

/**
 * Single file mapping for WSL sync
 */
export interface FileMapping {
  id: string;
  name: string;
  module: string; // "opencode" | "claude" | "codex"
  windowsPath: string;
  wslPath: string;
  enabled: boolean;
  isPattern: boolean;
  isDirectory: boolean;
}

/**
 * WSL sync configuration
 */
export interface WSLSyncConfig {
  enabled: boolean;
  distro: string;
  fileMappings: FileMapping[];
  lastSyncTime?: string;
  lastSyncStatus: string; // "success" | "error" | "never"
  lastSyncError?: string;
}

/**
 * Result of a sync operation
 */
export interface SyncResult {
  success: boolean;
  syncedFiles: string[];
  skippedFiles: string[];
  errors: string[];
}

/**
 * WSL detection result
 */
export interface WSLDetectResult {
  available: boolean;
  distros: string[];
  error?: string;
}

/**
 * WSL error result
 */
export interface WSLErrorResult {
  available: boolean;
  error?: string;
}

/**
 * WSL status result
 */
export interface WSLStatusResult {
  wslAvailable: boolean;
  lastSyncTime?: string;
  lastSyncStatus: string;
  lastSyncError?: string;
}
