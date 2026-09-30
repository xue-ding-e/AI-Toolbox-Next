import { locale } from '@tauri-apps/plugin-os';

/**
 * The UI languages the app ships.
 *
 * Canonical definition of the union — `resources` in './index' is annotated
 * against it, so the two cannot drift apart.
 */
export type Language = 'zh-CN' | 'en-US';

/** What the user picked in settings: a language, or "follow the system". */
export type LanguagePreference = 'system' | Language;

/** The preference value meaning "follow the system locale". */
export const SYSTEM_LANGUAGE = 'system';

/** Whether a persisted value is a language the app actually ships. */
export function isSupportedLanguage(value: unknown): value is Language {
  return value === 'zh-CN' || value === 'en-US';
}

/**
 * Collapse any BCP-47 tag to one of the supported UI languages.
 *
 * Must always return a supported language: `providers.tsx` indexes
 * `antdLocales` with this value unguarded, so anything else would leave antd
 * on English while i18next stayed on Chinese.
 *
 * Empty and unrecognized tags resolve to English. That is also what a `C` or
 * `POSIX` locale has to produce — the AppImageHub catalogue runs the app under
 * the C locale and requires it to start in English.
 */
export function normalizeLanguage(tag: string | null | undefined): Language {
  return tag?.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en-US';
}

/** Best-effort detection for callers that cannot await (initial render, recovery shell). */
export function detectLanguageSync(): Language {
  return normalizeLanguage(typeof navigator !== 'undefined' ? navigator.language : undefined);
}

/**
 * The OS locale, normalized.
 *
 * `sys-locale` (what plugin-os wraps) reads `LC_ALL` / `LC_MESSAGES` / `LANG`
 * on Linux, which is the only source that is deterministic under a C locale.
 *
 * The query is a parameter so the plugin-os path can be tested at all: outside
 * a Tauri window it throws, so a test that only calls the default would always
 * be measuring the `navigator.language` fallback instead.
 */
export async function detectSystemLanguage(
  query: () => Promise<string | null> = locale,
): Promise<Language> {
  try {
    const tag = await query();
    if (tag) return normalizeLanguage(tag);
    // A null locale is a real answer, not an error: no locale variable was set.
    return detectLanguageSync();
  } catch (error) {
    // Not running under Tauri (plain vite dev server, Node test suite) or the
    // IPC failed. Say which source won, since the two can disagree.
    console.warn('[i18n] OS locale unavailable, using navigator.language:', error);
    return detectLanguageSync();
  }
}

/**
 * Read the stored setting back into a preference.
 *
 * Anything unrecognized — including the empty string a fresh install stores —
 * means "follow the system", which is also what the settings UI shows.
 */
export function fromStoredLanguage(stored: unknown): LanguagePreference {
  return isSupportedLanguage(stored) ? stored : SYSTEM_LANGUAGE;
}

/**
 * What to persist for a preference.
 *
 * "system" is written as an empty string: that is the sentinel the backend
 * already uses (`AppSettings::default()` in types.rs) and what the tray's
 * `effective_language` treats as unset, so neither side has to learn a new
 * value to stay in agreement.
 */
export function toStoredLanguage(preference: LanguagePreference): string {
  return preference === SYSTEM_LANGUAGE ? '' : preference;
}

/** The language a preference resolves to right now. */
export async function resolveLanguagePreference(
  preference: LanguagePreference,
): Promise<Language> {
  return preference === SYSTEM_LANGUAGE ? detectSystemLanguage() : preference;
}

/** The language a stored setting resolves to. */
export async function resolveStoredLanguage(stored: unknown): Promise<Language> {
  return resolveLanguagePreference(fromStoredLanguage(stored));
}
