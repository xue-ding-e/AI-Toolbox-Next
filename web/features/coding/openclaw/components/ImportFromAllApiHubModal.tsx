import React from 'react';
import { useTranslation } from 'react-i18next';
import type { ExternalProviderDisplayItem } from '@/components/common/ImportExternalProvidersModal/types';
import ImportFromAllApiHubModalBase from '@/features/coding/shared/allApiHub/ImportFromAllApiHubModal';
import type { AllApiHubProviderModelsState } from '@/features/coding/shared/allApiHubModelsCache';
import {
  listOpenClawAllApiHubProviders,
  resolveOpenClawAllApiHubProviders,
  type OpenClawAllApiHubProvider,
} from '@/services/openclawApi';
import type { OpenClawProviderConfig } from '@/types/openclaw';

interface Props {
  open: boolean;
  existingProviderIds: string[];
  onCancel: () => void;
  onImport: (providers: OpenClawAllApiHubProvider[]) => void;
}

const ImportFromAllApiHubModal: React.FC<Props> = ({
  open,
  existingProviderIds,
  onCancel,
  onImport,
}) => {
  const { t } = useTranslation();

  const texts = React.useMemo(
    () => ({
      title: t('openclaw.providers.importFromAllApiHub'),
      noProvidersText: t('openclaw.providers.noAllApiHubProviders'),
      cancelText: t('common.cancel'),
      importButtonText: t('openclaw.providers.importSelected'),
      selectAllText: t('openclaw.providers.selectAll'),
      deselectAllText: t('openclaw.providers.deselectAll'),
      existingTagText: t('openclaw.providers.alreadyExists'),
      noApiKeyTagText: t('openclaw.providers.apiKeyMissing'),
      disabledTagText: t('openclaw.providers.disabled'),
      balanceLabelText: t('openclaw.providers.balance'),
      modelsLabelText: t('openclaw.providers.models'),
      loadingModelsText: t('openclaw.providers.loadingModels'),
      emptyModelsText: t('openclaw.providers.emptyModels'),
      modelsErrorText: t('openclaw.providers.modelsLoadFailed'),
      unsupportedModelsText: t('openclaw.providers.unsupportedModels'),
      expandModelsText: t('openclaw.providers.expandModels'),
      collapseModelsText: t('openclaw.providers.collapseModels'),
      profileLabel: t('openclaw.providers.sourceProfile'),
      siteTypeLabel: t('openclaw.providers.siteType'),
      loadingTokenText: t('openclaw.providers.loadingApiKey'),
      tokenResolvedText: t('openclaw.providers.apiKeyReady'),
      retryResolveText: t('openclaw.providers.retryResolve'),
      searchPlaceholder: t('openclaw.providers.searchPlaceholder'),
      confirmTitle: t('openclaw.providers.importAllApiHubProtocolTitle'),
      confirmOkText: t('openclaw.providers.importAllApiHubReviewConfirm'),
    }),
    [t]
  );

  const mapProviderToItem = React.useCallback(
    (
      provider: OpenClawAllApiHubProvider,
      modelState?: AllApiHubProviderModelsState
    ): ExternalProviderDisplayItem<OpenClawProviderConfig> => ({
      providerId: provider.providerId,
      name: provider.name,
      baseUrl: provider.baseUrl || undefined,
      accountLabel: provider.accountLabel,
      siteName: provider.siteName || undefined,
      siteType: provider.siteType || undefined,
      sourceProfileName: provider.sourceProfileName,
      sourceExtensionId: provider.sourceExtensionId,
      requiresBrowserOpen: provider.requiresBrowserOpen,
      isDisabled: provider.isDisabled,
      hasApiKey: provider.hasApiKey,
      apiKeyPreview: provider.apiKeyPreview,
      balanceUsd: provider.balanceUsd,
      balanceCny: provider.balanceCny,
      models: modelState?.models || [],
      modelsStatus: modelState?.status || 'idle',
      modelsError: modelState?.error,
      config: provider.config,
      secondaryLabel: provider.apiProtocol,
    }),
    []
  );

  const getConfirmSections = React.useCallback(
    (providers: OpenClawAllApiHubProvider[]) =>
      [
        providers.filter((provider) => provider.apiProtocol === 'openai-completions').length > 0
          ? {
              description: t('openclaw.providers.importAllApiHubProtocolDesc'),
              providerNames: providers
                .filter((provider) => provider.apiProtocol === 'openai-completions')
                .map((provider) => provider.name),
            }
          : null,
        providers.filter((provider) => !provider.hasApiKey).length > 0
          ? {
              description: t('openclaw.providers.importAllApiHubMissingApiKeyDesc'),
              providerNames: providers
                .filter((provider) => !provider.hasApiKey)
                .map((provider) => provider.name),
            }
          : null,
      ].filter((section): section is { description: string; providerNames: string[] } => !!section),
    [t]
  );

  return (
    <ImportFromAllApiHubModalBase
      open={open}
      providerTypes={[]}
      existingProviderIds={existingProviderIds}
      listProviders={listOpenClawAllApiHubProviders}
      resolveProviders={resolveOpenClawAllApiHubProviders}
      onCancel={onCancel}
      onImport={onImport}
      texts={texts}
      getProviderId={(provider) => provider.providerId}
      getProviderType={(provider) => provider.apiProtocol}
      mapProviderToItem={mapProviderToItem}
      getConfirmSections={getConfirmSections}
    />
  );
};

export default ImportFromAllApiHubModal;
