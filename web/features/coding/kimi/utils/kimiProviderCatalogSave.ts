import type { KimiProvider } from '@/types/kimi';
import {
  saveProviderWithGatewayReengage,
  type GatewayReengageMode,
} from '@/features/coding/shared/gateway';

interface SaveKimiProviderCatalogOptions<TStatus> {
  provider: KimiProvider;
  settingsConfig: string;
  /** Active gateway mode captured before the save; null when not taken over. */
  gatewayMode: GatewayReengageMode;
  updateProvider: (provider: KimiProvider) => Promise<KimiProvider>;
  restoreDirect: () => Promise<TStatus>;
  engageSingle: () => Promise<TStatus>;
  engageFailover: () => Promise<TStatus>;
  onGatewayStatusChange?: (status: TStatus) => void;
}

/**
 * Persist a Kimi provider with catalog edits, replaying an active gateway
 * takeover around the write.
 *
 * Kimi has no aggregate mode (that is Codex-only), so only single and failover
 * takeovers are replayed — passing an aggregate mode here would make the shared
 * helper reject the replay for a missing `engageAggregate`.
 */
export async function saveKimiProviderCatalogWithGatewayReengage<TStatus>({
  provider,
  settingsConfig,
  gatewayMode,
  updateProvider,
  restoreDirect,
  engageSingle,
  engageFailover,
  onGatewayStatusChange,
}: SaveKimiProviderCatalogOptions<TStatus>): Promise<KimiProvider> {
  const shouldReengageGateway = Boolean(provider.isApplied)
    && (gatewayMode === 'single' || gatewayMode === 'failover');

  return saveProviderWithGatewayReengage({
    gatewayMode: shouldReengageGateway ? gatewayMode : null,
    restoreDirect,
    engageSingle,
    engageFailover,
    onGatewayStatusChange,
    saveProvider: () => updateProvider({ ...provider, settingsConfig }),
  });
}
