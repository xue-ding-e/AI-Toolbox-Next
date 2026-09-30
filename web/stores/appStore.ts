import { create } from 'zustand';
import i18n, {
  SYSTEM_LANGUAGE,
  detectLanguageSync,
  fromStoredLanguage,
  resolveLanguagePreference,
  resolveStoredLanguage,
  toStoredLanguage,
  type Language,
  type LanguagePreference,
} from '@/i18n';
import { getSettings, saveSettings, type AppSettings } from '@/services';
import { DEFAULT_MODULE } from '@/constants';

interface AppState {
  // Loading state
  isLoading: boolean;
  isInitialized: boolean;

  // App state
  currentModule: string;
  currentSubTab: string;
  /** The resolved language actually in use — always one of the shipped two. */
  language: Language;
  /** What the user chose: a language, or "follow the system". */
  languagePreference: LanguagePreference;

  // Actions
  initApp: () => Promise<void>;
  setCurrentModule: (module: string) => Promise<void>;
  setCurrentSubTab: (subTab: string) => Promise<void>;
  setLanguage: (preference: LanguagePreference) => Promise<void>;
}

export const useAppStore = create<AppState>()((set, get) => ({
  isLoading: false,
  isInitialized: false,
  currentModule: DEFAULT_MODULE.key,
  currentSubTab: DEFAULT_MODULE.subTabs[0]?.key || '',
  language: detectLanguageSync(),
  languagePreference: SYSTEM_LANGUAGE,

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
        languagePreference: fromStoredLanguage(settings.language),
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

  setLanguage: async (preference) => {
    // Reflect the choice straight away; resolving "system" costs an IPC call.
    set({ languagePreference: preference });

    const language = await resolveLanguagePreference(preference);
    if (i18n.language !== language) {
      await i18n.changeLanguage(language);
    }
    set({ language });

    try {
      const currentSettings = await getSettings();
      const newSettings: AppSettings = {
        ...currentSettings,
        language: toStoredLanguage(preference),
      };
      await saveSettings(newSettings);
    } catch (error) {
      console.error('Failed to save language:', error);
    }
  },
}));
