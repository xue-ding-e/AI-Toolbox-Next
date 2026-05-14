import type { AppSettings } from '../services/settingsApi';

export const buildLaunchOnStartupSettings = (
  currentSettings: AppSettings,
  enabled: boolean,
): AppSettings => ({
  ...currentSettings,
  launch_on_startup: enabled,
  start_minimized: enabled ? currentSettings.start_minimized : false,
});
