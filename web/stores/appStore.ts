import { create } from 'zustand';
import i18n, { detectLanguageSync, resolveStoredLanguage, type Language } from '@/i18n';
import { getSettings, saveSettings, type AppSettings } from '@/services';
import { DEFAULT_MODULE } from '@/constants';

interface AppState {
  // Loading state
  isLoading: boolean;
  isInitialized: boolean;

  // App state
  currentModule: string;
  currentSubTab: string;
  language: Language;

  // Actions
  initApp: () => Promise<void>;
  setCurrentModule: (module: string) => Promise<void>;
  setCurrentSubTab: (subTab: string) => Promise<void>;
  setLanguage: (language: Language) => Promise<void>;
}

export const useAppStore = create<AppState>()((set, get) => ({
  isLoading: false,
  isInitialized: false,
  currentModule: DEFAULT_MODULE.key,
  currentSubTab: DEFAULT_MODULE.subTabs[0]?.key || '',
  language: detectLanguageSync(),

  initApp: async () => {
    if (get().isInitialized) return;

    set({ isLoading: true });
    try {
      const settings = await getSettings();
      const language = await resolveStoredLanguage(settings.language);
      // Apply before the loading gate drops so the first painted frame is
      // already in the right language. The effect in `providers.tsx` only
      // covers later, user-initiated switches.
      if (i18n.language !== language) {
        await i18n.changeLanguage(language);
      }
      set({
        currentModule: settings.current_module || DEFAULT_MODULE.key,
        currentSubTab: settings.current_sub_tab || DEFAULT_MODULE.subTabs[0]?.key || '',
        language,
        isInitialized: true,
      });
    } catch (error) {
      console.error('Failed to load app settings:', error);
    } finally {
      set({ isLoading: false });
    }
  },

  setCurrentModule: async (currentModule) => {
    set({ currentModule });

    try {
      const currentSettings = await getSettings();
      const newSettings: AppSettings = {
        ...currentSettings,
        current_module: currentModule,
      };
      await saveSettings(newSettings);
    } catch (error) {
      console.error('Failed to save current module:', error);
    }
  },

  setCurrentSubTab: async (currentSubTab) => {
    set({ currentSubTab });

    try {
      const currentSettings = await getSettings();
      const newSettings: AppSettings = {
        ...currentSettings,
        current_sub_tab: currentSubTab,
      };
      await saveSettings(newSettings);
    } catch (error) {
      console.error('Failed to save current sub tab:', error);
    }
  },

  setLanguage: async (language) => {
    set({ language });

    try {
      const currentSettings = await getSettings();
      const newSettings: AppSettings = {
        ...currentSettings,
        language,
      };
      await saveSettings(newSettings);
    } catch (error) {
      console.error('Failed to save language:', error);
    }
  },
}));
