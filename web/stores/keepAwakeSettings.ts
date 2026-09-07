import type { AppSettings } from '../services/settingsApi';

export interface KeepAwakeSettingsState {
  keepComputerAwake: boolean;
  keepComputerAwakeUpdating: boolean;
  keepComputerAwakeError: string | null;
  setKeepComputerAwake: (enabled: boolean) => Promise<void>;
}

interface KeepAwakeDependencies {
  getSettings: () => Promise<AppSettings>;
  saveSettings: (settings: AppSettings) => Promise<void>;
  setKeepAwake: (enabled: boolean) => Promise<void>;
}

export function createKeepAwakeSettingsSlice(
  set: (state: Partial<KeepAwakeSettingsState>) => void,
  dependencies: KeepAwakeDependencies,
): KeepAwakeSettingsState {
  let updateQueue = Promise.resolve();
  let pendingUpdates = 0;

  return {
    keepComputerAwake: false,
    keepComputerAwakeUpdating: false,
    keepComputerAwakeError: null,

    setKeepComputerAwake: (enabled) => {
      pendingUpdates += 1;
      set({ keepComputerAwakeUpdating: true });

      // Serialize the entire read/save/apply operation. save_settings can finish
      // out of order because it awaits a coalesced tray refresh after writing.
      const update = updateQueue.then(async () => {
        set({ keepComputerAwakeError: null });
        try {
          const currentSettings = await dependencies.getSettings();
          await dependencies.saveSettings({
            ...currentSettings,
            keep_computer_awake: enabled,
          });
          // This switch represents the persisted preference. An OS failure
          // must retain that preference and expose the failure separately.
          set({ keepComputerAwake: enabled });
          await dependencies.setKeepAwake(enabled);
        } catch (error) {
          set({
            keepComputerAwakeError: error instanceof Error ? error.message : String(error),
          });
          throw error;
        } finally {
          pendingUpdates -= 1;
          set({ keepComputerAwakeUpdating: pendingUpdates > 0 });
        }
      });

      // One failed operation must not prevent a later retry or disable.
      updateQueue = update.catch(() => {});
      return update;
    },
  };
}
