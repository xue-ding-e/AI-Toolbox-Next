import type { ClaudeDesktopProvider } from '@/types/claudedesktop';

// Keep in sync with tauri/src/coding/claude_desktop/constants.rs.
export const CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID = 'claude-desktop-official';

/** Pin identity is narrower than restore semantics: copies and imported rows
 * retain their own identity, even when their category is official. */
export function isClaudeDesktopPinnedOfficialProvider(provider?: Pick<ClaudeDesktopProvider, 'id'> | null): boolean {
  return provider?.id === CLAUDE_DESKTOP_OFFICIAL_PROVIDER_ID;
}

export function canDeleteClaudeDesktopProvider(provider?: Pick<ClaudeDesktopProvider, 'id'> | null): boolean {
  return Boolean(provider?.id) && !isClaudeDesktopPinnedOfficialProvider(provider);
}

type RestoreProvider = Pick<ClaudeDesktopProvider, 'id' | 'category' | 'settingsConfig'>;

/** Mirror the backend's seed-id override and official_restore_settings predicate.
 * Raw category alone is not a channel mode: imported official rows may have
 * upstream credentials. The legacy API_KEY is deliberately not a restore key.
 */
export function isClaudeDesktopOfficialRestoreProvider(provider?: RestoreProvider | null): boolean {
  if (!provider) return false;
  if (isClaudeDesktopPinnedOfficialProvider(provider)) return true;
  if (provider.category !== 'official') return false;

  let settings: unknown;
  try {
    settings = JSON.parse(provider.settingsConfig);
  } catch {
    return false;
  }
  const env = settings !== null && typeof settings === 'object'
    ? (settings as Record<string, unknown>).env
    : undefined;
  if (env === null || typeof env !== 'object' || Array.isArray(env)) return true;
  const credentials = env as Record<string, unknown>;
  // Rust str::trim uses Unicode White_Space, not JavaScript's extra BOM removal.
  const hasCredential = (value: unknown) =>
    typeof value === 'string' && /[^\p{White_Space}]/u.test(value);
  return !hasCredential(credentials.ANTHROPIC_BASE_URL)
    && !hasCredential(credentials.ANTHROPIC_AUTH_TOKEN);
}

/** Only unchanged credential pairs on existing, identifiable imported official
 * rows may retain an original missing field. New/copy/custom rows and every
 * credential edit keep the usual two required fields. Compare the exact form
 * representation: absent and empty strings both initialize to ''; no trimming.
 */
export function canPreserveImportedClaudeDesktopCredentials(
  provider: (RestoreProvider & Pick<ClaudeDesktopProvider, 'sourceProviderId'>) | null | undefined,
  values: { baseUrl: unknown; apiKey: unknown },
  isCopy: boolean,
): boolean {
  if (!provider || isCopy || provider.category !== 'official'
    || isClaudeDesktopOfficialRestoreProvider(provider)) return false;
  if (!/^(?:ccs:claude:|claude:).+$/.test(provider.sourceProviderId ?? '')) return false;
  let settings: unknown;
  try {
    settings = JSON.parse(provider.settingsConfig);
  } catch {
    return false;
  }
  const env = settings !== null && typeof settings === 'object'
    ? (settings as Record<string, unknown>).env
    : undefined;
  if (!env || typeof env !== 'object' || Array.isArray(env)) return false;
  const credentials = env as Record<string, unknown>;
  const baseUrl = credentials.ANTHROPIC_BASE_URL;
  const authToken = credentials.ANTHROPIC_AUTH_TOKEN;
  const legacyKey = credentials.ANTHROPIC_API_KEY;
  if ([baseUrl, authToken, legacyKey].some(value => value !== undefined && typeof value !== 'string')) return false;
  return values.baseUrl === (baseUrl || '')
    && values.apiKey === (authToken || legacyKey || '');
}
