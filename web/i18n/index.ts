import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

import zhCN from './locales/zh-CN.json';
import enUS from './locales/en-US.json';
import { detectLanguageSync, type Language } from './language';

export type { Language, LanguagePreference } from './language';
// Re-exported here because this is the module the app imports from; the rest of
// './language' (the normalizers and validators it uses internally) is imported
// directly by whoever needs it, so nothing is exposed that has no consumer.
export {
  SYSTEM_LANGUAGE,
  detectLanguageSync,
  fromStoredLanguage,
  resolveLanguagePreference,
  resolveStoredLanguage,
  toStoredLanguage,
} from './language';

/**
 * Annotated with `Record<Language, ...>` rather than derived from the object
 * literal so that the union and the shipped translations are checked in both
 * directions: a missing language or an extra one fails to compile.
 */
export const resources: Record<Language, { translation: Record<string, unknown> }> = {
  'zh-CN': { translation: zhCN },
  'en-US': { translation: enUS },
};

export const languages: { value: Language; label: string }[] = [
  { value: 'zh-CN', label: '简体中文' },
  { value: 'en-US', label: 'English' },
];

i18n.use(initReactI18next).init({
  resources,
  // Best-effort at module load so the very first paint is close to right; the
  // authoritative value arrives with the settings (see `appStore.initApp`).
  lng: detectLanguageSync(),
  // Fall back to the international default rather than to a raw key.
  fallbackLng: 'en-US',
  interpolation: {
    escapeValue: false,
  },
});

export default i18n;
