import type { AntigravityProvider } from '@/types/antigravity';

export const ANTIGRAVITY_LOCAL_PROVIDER_ID = '__local__';

export function isAntigravityLocalProviderId(providerId: string | null | undefined): boolean {
  return providerId === ANTIGRAVITY_LOCAL_PROVIDER_ID;
}

export function shouldLoadAntigravityOfficialAccounts(
  provider: Pick<AntigravityProvider, 'id'>,
): boolean {
  return !isAntigravityLocalProviderId(provider.id);
}

export function shouldShowAntigravityOfficialAccounts(
  provider: Pick<AntigravityProvider, 'id' | 'category'>,
  officialAccountCount: number,
): boolean {
  return shouldLoadAntigravityOfficialAccounts(provider) && (
    provider.category === 'official' || officialAccountCount > 0
  );
}
