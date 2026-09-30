import { locale } from '@tauri-apps/plugin-os';

/**
 * The UI languages the app ships.
 *
 * Canonical definition of the union — `resources` in './index' is annotated
 * against it, so the two cannot drift apart.
 */
export type Language = 'zh-CN' | 'en-US';

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

/** Whether a persisted value is a language the app actually ships. */
export function isSupportedLanguage(value: unknown): value is Language {
  return value === 'zh-CN' || value === 'en-US';
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
 */
export async function detectSystemLanguage(): Promise<Language> {
  try {
    const tag = await locale();
    if (tag) return normalizeLanguage(tag);
  } catch {
    // Not running under Tauri (plain vite dev server, Node test suite).
  }
  return detectLanguageSync();
}

/**
 * Pick the UI language for a stored setting.
 *
 * An empty or unrecognized value means the user never made a choice — a fresh
 * install stores `""`, not a language — so fall back to the system locale
 * instead of a fixed language. An explicit choice is always preserved, even
 * when it disagrees with the system locale.
 */
export async function resolveStoredLanguage(stored: unknown): Promise<Language> {
  if (isSupportedLanguage(stored)) return stored;
  return detectSystemLanguage();
}
