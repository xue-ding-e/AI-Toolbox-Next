import type { GeminiCliProvider } from '@/types/geminicli';

export const GEMINI_CLI_LOCAL_PROVIDER_ID = '__local__';

export function isGeminiCliLocalProviderId(providerId: string | null | undefined): boolean {
  return providerId === GEMINI_CLI_LOCAL_PROVIDER_ID;
}

export function shouldLoadGeminiCliOfficialAccounts(
  provider: Pick<GeminiCliProvider, 'id'>,
): boolean {
  return !isGeminiCliLocalProviderId(provider.id);
}

export function shouldShowGeminiCliOfficialAccounts(
  provider: Pick<GeminiCliProvider, 'id' | 'category'>,
  officialAccountCount: number,
): boolean {
  return shouldLoadGeminiCliOfficialAccounts(provider) && (
    provider.category === 'official' || officialAccountCount > 0
  );
}
