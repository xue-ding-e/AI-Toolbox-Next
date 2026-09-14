/**
 * Backup API Service
 *
 * Handles all backup-related communication with the Tauri backend. Local, WebDAV,
 * and GitHub/Gitee repository channels share the same generation/restore pipeline;
 * encrypted backups are detected by file header and the password never persists in
 * the frontend.
 */

import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { BackupCustomEntry, BackupFileFilterRule } from './settingsApi';

/**
 * Backup file info from a remote storage channel (WebDAV/repository). Repository
 * entries additionally carry the blob SHA used to bind download/delete to the
 * exact listed file.
 */
export interface BackupFileInfo {
  filename: string;
  size: number;
  encrypted: boolean;
  sha?: string;
}

export interface RestoreWarning {
  tool: string;
  originalPath: string;
  fallbackPath: string;
}

export interface RestoreResult {
  warnings: RestoreWarning[];
  willReapplyApplied?: boolean;
}

export interface RestoreOptions {
  skipCliCustomRoots?: boolean;
  /**
   * One-shot decryption password for an encrypted backup. When omitted, the backend
   * falls back to the password stored in the local OS credential store.
   */
  restorePassword?: string;
}

export type BackupRepositoryPlatform = 'github' | 'gitee';

export interface BackupRepositoryConfigFE {
  platform: BackupRepositoryPlatform;
  owner: string;
  repository: string;
  branch: string;
  directory: string;
}

export interface BackupRepositoryView {
  config: BackupRepositoryConfigFE;
  has_token: boolean;
}

export interface BackupEncryptionStatus {
  enabled: boolean;
  has_password: boolean;
  /** False when the OS credential store could not be read: has_password is unknown. */
  password_known: boolean;
}

export interface BackupSettingsPayload {
  backup_type: 'local' | 'webdav' | 'repository';
  local_backup_path: string;
  webdav: {
    url: string;
    username: string;
    password: string;
    remote_path: string;
    host_label: string;
  };
  backup_encryption_enabled: boolean;
  /** Submit a new password to store; omit/empty to keep the stored credential. */
  encryption_password?: string;
  repository: BackupRepositoryConfigFE;
  repository_token?: string;
  backup_image_assets_enabled: boolean;
  backup_cli_config_files_enabled: boolean;
  backup_custom_entries: BackupCustomEntry[];
  backup_file_filter_rules: BackupFileFilterRule[];
  auto_backup_enabled: boolean;
  auto_backup_interval_days: number;
  auto_backup_max_keep: number;
}

export interface BackupSettingsSaveOutcome {
  repository: BackupRepositoryView;
  encryption: BackupEncryptionStatus;
}

/**
 * Backup database to a local zip file
 * @param backupPath - The directory to save the backup file
 * @returns The full path of the created backup file
 */
export const backupDatabase = async (backupPath: string): Promise<string> => {
  if (!backupPath) {
    throw new Error('Backup path is not configured');
  }

  const result = await invoke<string>('backup_database', { backupPath });
  return result;
};

/**
 * Restore database from a local backup file
 * @param zipFilePath - The path to the backup zip file
 */
export const restoreDatabase = async (
  zipFilePath: string,
  options?: RestoreOptions
): Promise<RestoreResult> => {
  return await invoke<RestoreResult>('restore_database', {
    zipFilePath,
    skipCliCustomRoots: options?.skipCliCustomRoots ?? false,
    restorePassword: options?.restorePassword ?? null,
  });
};

/**
 * Get the database directory path
 */
export const getDatabasePath = async (): Promise<string> => {
  const result = await invoke<string>('get_database_path');
  return result;
};

/**
 * Open file dialog to select a backup file for restore. Both plaintext `.zip` and
 * encrypted `.zip.enc` backups must be selectable; the backend detects the actual
 * format by file header.
 * @returns The selected file path, or null if cancelled
 */
export const selectBackupFile = async (): Promise<string | null> => {
  const selected = await open({
    multiple: false,
    filters: [
      {
        name: 'Backup Files',
        extensions: ['zip', 'enc'],
      },
    ],
    title: 'Select Backup File',
  });

  return selected as string | null;
};

/**
 * Backup database to WebDAV server
 */
export const backupToWebDAV = async (
  url: string,
  username: string,
  password: string,
  remotePath: string,
  hostLabel: string
): Promise<string> => {
  const result = await invoke<string>('backup_to_webdav', {
    url,
    username,
    password,
    remotePath,
    hostLabel,
  });
  return result;
};

/**
 * List backup files from WebDAV server
 */
export const listWebDAVBackups = async (
  url: string,
  username: string,
  password: string,
  remotePath: string
): Promise<BackupFileInfo[]> => {
  const result = await invoke<BackupFileInfo[]>('list_webdav_backups', {
    url,
    username,
    password,
    remotePath,
  });
  return result;
};

/**
 * Restore database from WebDAV server
 */
export const restoreFromWebDAV = async (
  url: string,
  username: string,
  password: string,
  remotePath: string,
  filename: string,
  options?: RestoreOptions
): Promise<RestoreResult> => {
  return await invoke<RestoreResult>('restore_from_webdav', {
    url,
    username,
    password,
    remotePath,
    filename,
    skipCliCustomRoots: options?.skipCliCustomRoots ?? false,
    restorePassword: options?.restorePassword ?? null,
  });
};

/**
 * Test WebDAV connection
 */
export const testWebDAVConnection = async (
  url: string,
  username: string,
  password: string,
  remotePath: string
): Promise<void> => {
  await invoke('test_webdav_connection', {
    url,
    username,
    password,
    remotePath,
  });
};

/**
 * Delete a backup file from WebDAV server
 */
export const deleteWebDAVBackup = async (
  url: string,
  username: string,
  password: string,
  remotePath: string,
  filename: string
): Promise<void> => {
  await invoke('delete_webdav_backup', {
    url,
    username,
    password,
    remotePath,
    filename,
  });
};

/**
 * Backup database to the configured GitHub/Gitee repository.
 * @returns The uploaded backup filename
 */
export const backupToRepository = async (): Promise<string> => {
  return await invoke<string>('backup_to_repository');
};

/**
 * List backup files stored in the configured repository directory.
 */
export const listRepositoryBackups = async (): Promise<BackupFileInfo[]> => {
  return await invoke<BackupFileInfo[]>('list_repository_backups');
};

/**
 * Restore database from a repository backup bound to its listed SHA.
 */
export const restoreFromRepository = async (
  filename: string,
  sha: string,
  options?: RestoreOptions
): Promise<RestoreResult> => {
  return await invoke<RestoreResult>('restore_from_repository', {
    filename,
    sha,
    skipCliCustomRoots: options?.skipCliCustomRoots ?? false,
    restorePassword: options?.restorePassword ?? null,
  });
};

/**
 * Delete one repository backup bound to its listed SHA.
 */
export const deleteRepositoryBackup = async (
  filename: string,
  sha: string
): Promise<void> => {
  await invoke('delete_repository_backup', { filename, sha });
};

/**
 * Test the repository connection using the submitted draft. An empty token falls
 * back to the stored credential.
 */
export const testRepositoryConnection = async (
  config: BackupRepositoryConfigFE,
  token?: string
): Promise<void> => {
  await invoke('test_backup_repository_connection', {
    config,
    token: token ?? null,
  });
};

/**
 * Load the stored repository connection (token never leaves the backend).
 */
export const getBackupRepositorySettings = async (): Promise<BackupRepositoryView> => {
  return await invoke<BackupRepositoryView>('get_backup_repository_settings');
};

/**
 * Report the encryption switch plus whether this machine has a stored password.
 */
export const getBackupEncryptionStatus = async (): Promise<BackupEncryptionStatus> => {
  return await invoke<BackupEncryptionStatus>('get_backup_encryption_status');
};

/**
 * Unified backup settings save: patches only backup-related settings fields, updates
 * the repository connection record, and stores a newly submitted encryption password
 * in the OS credential store.
 */
export const saveBackupSettings = async (
  payload: BackupSettingsPayload
): Promise<BackupSettingsSaveOutcome> => {
  return await invoke<BackupSettingsSaveOutcome>('save_backup_settings', { payload });
};
