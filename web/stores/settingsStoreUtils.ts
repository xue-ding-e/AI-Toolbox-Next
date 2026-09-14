import type {
  AppSettings,
  BackupCustomEntry,
  BackupFileFilterRule,
} from '../services/settingsApi';

export const buildLaunchOnStartupSettings = (
  currentSettings: AppSettings,
  enabled: boolean,
): AppSettings => ({
  ...currentSettings,
  launch_on_startup: enabled,
  start_minimized: enabled ? currentSettings.start_minimized : false,
});

/**
 * Structural view of the unified backup settings save, so the saved-state patch
 * below stays testable without importing the aliased store module. The real store
 * payload (`BackupSettingsFormValues`) satisfies this shape.
 */
interface BackupSettingsSavedConfig {
  backupType: 'local' | 'webdav' | 'repository';
  localBackupPath: string;
  webdav: {
    url: string;
    username: string;
    password: string;
    remotePath: string;
    hostLabel: string;
  };
  backupImageAssetsEnabled: boolean;
  backupCliConfigFilesEnabled: boolean;
  backupCustomEntries: BackupCustomEntry[];
  backupFileFilterRules: BackupFileFilterRule[];
  autoBackupEnabled: boolean;
  autoBackupIntervalDays: number;
  autoBackupMaxKeep: number;
}

/**
 * State patch for a successful unified backup settings save. Every submitted
 * field must be synced back into the store — including the auto-backup
 * parameters — so reopening the modal never reads stale values and a follow-up
 * save cannot silently overwrite what was just persisted.
 */
export const backupSettingsStatePatch = (
  config: BackupSettingsSavedConfig,
  outcome: { encryption: { enabled: boolean } },
) => ({
  backupType: config.backupType,
  localBackupPath: config.localBackupPath,
  webdav: config.webdav,
  backupImageAssetsEnabled: config.backupImageAssetsEnabled,
  backupCliConfigFilesEnabled: config.backupCliConfigFilesEnabled,
  backupCustomEntries: config.backupCustomEntries,
  backupFileFilterRules: config.backupFileFilterRules,
  autoBackupEnabled: config.autoBackupEnabled,
  autoBackupIntervalDays: config.autoBackupIntervalDays,
  autoBackupMaxKeep: config.autoBackupMaxKeep,
  backupEncryptionEnabled: outcome.encryption.enabled,
});
