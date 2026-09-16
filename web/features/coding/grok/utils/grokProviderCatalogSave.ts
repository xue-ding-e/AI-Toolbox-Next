import {
  saveProviderWithGatewayReengage,
  type GatewayAggregateReengageConfig,
  type GatewayReengageMode,
} from '../../shared/gateway/providerSaveReengage';
import type { GrokProvider } from '../../../../types/grok';

interface SaveGrokProviderCatalogOptions<TStatus> {
  provider: GrokProvider;
  settingsConfig: string;
  gatewayMode: GatewayReengageMode;
  updateProvider: (provider: GrokProvider) => Promise<GrokProvider>;
  restoreDirect: () => Promise<TStatus>;
  engageSingle: () => Promise<TStatus>;
  engageFailover: () => Promise<TStatus>;
  engageAggregate?: (config: GatewayAggregateReengageConfig) => Promise<TStatus>;
  aggregateConfig?: GatewayAggregateReengageConfig | null;
  onGatewayStatusChange?: (status: TStatus) => void;
}

export async function saveGrokProviderCatalogWithGatewayReengage<TStatus>({
  provider,
  settingsConfig,
  gatewayMode,
  updateProvider,
  restoreDirect,
  engageSingle,
  engageFailover,
  engageAggregate,
  aggregateConfig,
  onGatewayStatusChange,
}: SaveGrokProviderCatalogOptions<TStatus>): Promise<GrokProvider> {
  const shouldReengageGateway = provider.isApplied
    && (gatewayMode === 'single' || gatewayMode === 'failover' || gatewayMode === 'aggregate');

  return saveProviderWithGatewayReengage({
    gatewayMode: shouldReengageGateway ? gatewayMode : null,
    aggregateConfig: shouldReengageGateway ? aggregateConfig : null,
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
