import type { KimiOfficialAccount, KimiProvider } from '@/types/kimi';
import { KIMI_LOCAL_PROVIDER_ID } from '../../../../types/kimi.ts';

export function canDeleteKimiProvider(
  provider: KimiProvider,
  officialAccounts: readonly KimiOfficialAccount[],
): boolean {
  return provider.id !== KIMI_LOCAL_PROVIDER_ID
    && !provider.isApplied
    && !officialAccounts.some((account) => account.providerId === provider.id);
}
