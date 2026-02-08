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
  /** Sync MCP configuration to WSL (default: true) */
  syncMcp: boolean;
  /** Sync Skills to WSL (default: true) */
  syncSkills: boolean;
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

/**
 * Sync progress event payload
 */
export interface SyncProgress {
  /** Current phase: "files" | "mcp" | "skills" */
  phase: string;
  /** Current item being processed */
  currentItem: string;
  /** Current item index (1-based) */
  current: number;
  /** Total items in this phase */
  total: number;
  /** Overall progress message */
  message: string;
}
