import {
  saveProviderWithGatewayReengage,
  type GatewayAggregateReengageConfig,
  type GatewayReengageMode,
} from '../../shared/gateway/providerSaveReengage';
import type { CodexProvider } from '../../../../types/codex';

interface SaveCodexProviderCatalogOptions<TStatus> {
  provider: CodexProvider;
  settingsConfig: string;
  gatewayMode: GatewayReengageMode;
  /** Required when `gatewayMode` is `aggregate`; ignored otherwise. */
  aggregateConfig?: GatewayAggregateReengageConfig | null;
  updateProvider: (provider: CodexProvider) => Promise<CodexProvider>;
  restoreDirect: () => Promise<TStatus>;
  engageSingle: () => Promise<TStatus>;
  engageFailover: () => Promise<TStatus>;
  engageAggregate?: (config: GatewayAggregateReengageConfig) => Promise<TStatus>;
  onGatewayStatusChange?: (status: TStatus) => void;
}

/**
 * Persist a Codex provider with catalog edits and replay any active gateway
 * takeover. Codex has a four-state gateway (direct -> single -> failover ->
 * aggregate), so unlike Grok this must also replay an aggregate takeover or the
 * cross-site catalog would be dropped by the restore-direct round trip.
 */
export async function saveCodexProviderCatalogWithGatewayReengage<TStatus>({
  provider,
  settingsConfig,
  gatewayMode,
  aggregateConfig,
  updateProvider,
  restoreDirect,
  engageSingle,
  engageFailover,
  engageAggregate,
  onGatewayStatusChange,
}: SaveCodexProviderCatalogOptions<TStatus>): Promise<CodexProvider> {
  const shouldReengageGateway = provider.isApplied
    && (gatewayMode === 'single' || gatewayMode === 'failover' || gatewayMode === 'aggregate');

  return saveProviderWithGatewayReengage({
    gatewayMode: shouldReengageGateway ? gatewayMode : null,
    aggregateConfig: shouldReengageGateway && gatewayMode === 'aggregate' ? aggregateConfig : null,
    restoreDirect,
    engageSingle,
    engageFailover,
    engageAggregate,
    onGatewayStatusChange,
    saveProvider: () => updateProvider({
      ...provider,
      settingsConfig,
    }),
  });
}
