import type { CodexProvider } from '@/types/codex';

export const CODEX_LOCAL_PROVIDER_ID = '__local__';

export function isCodexLocalProviderId(providerId: string | null | undefined): boolean {
  return providerId === CODEX_LOCAL_PROVIDER_ID;
}

export function shouldLoadCodexOfficialAccounts(provider: Pick<CodexProvider, 'id'>): boolean {
  return !isCodexLocalProviderId(provider.id);
}

export function shouldShowCodexOfficialAccounts(
  provider: Pick<CodexProvider, 'id' | 'category'>,
  officialAccountCount: number,
): boolean {
  return shouldLoadCodexOfficialAccounts(provider) && (
    provider.category === 'official' || officialAccountCount > 0
  );
}
