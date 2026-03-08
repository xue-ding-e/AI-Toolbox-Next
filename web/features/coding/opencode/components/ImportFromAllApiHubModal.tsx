import React from 'react';
import { useTranslation } from 'react-i18next';
import type { ExternalProviderDisplayItem } from '@/components/common/ImportExternalProvidersModal/types';
import ImportFromAllApiHubModalBase from '@/features/coding/shared/allApiHub/ImportFromAllApiHubModal';
import {
  listOpenCodeAllApiHubProviders,
  resolveOpenCodeAllApiHubProviders,
  type OpenCodeAllApiHubProvider,
} from '@/services/opencodeApi';
import type { OpenCodeProvider } from '@/types/opencode';
import type { AllApiHubProviderModelsState } from '@/features/coding/shared/allApiHubModelsCache';

interface Props {
  open: boolean;
  existingProviderIds: string[];
  onClose: () => void;
  onImport: (providers: OpenCodeAllApiHubProvider[]) => void;
}

const ImportFromAllApiHubModal: React.FC<Props> = ({
  open,
  existingProviderIds,
  onClose,
  onImport,
}) => {
  const { t } = useTranslation();

  const texts = React.useMemo(
    () => ({
      title: t('opencode.provider.importAllApiHubModalTitle'),
      noProvidersText: t('opencode.provider.noAllApiHubProviders'),
      cancelText: t('common.cancel'),
      importButtonText: t('opencode.provider.importSelected'),
      selectAllText: t('opencode.provider.selectAllProviders'),
      deselectAllText: t('opencode.provider.deselectAllProviders'),
      existingTagText: t('opencode.provider.providerExists'),
      noApiKeyTagText: t('opencode.provider.apiKeyMissing'),
      disabledTagText: t('opencode.provider.disabled'),
      balanceLabelText: t('opencode.provider.balance'),
      modelsLabelText: t('opencode.provider.models'),
      loadingModelsText: t('opencode.provider.loadingModels'),
      emptyModelsText: t('opencode.provider.emptyModels'),
      modelsErrorText: t('opencode.provider.modelsLoadFailed'),
      unsupportedModelsText: t('opencode.provider.unsupportedModels'),
      expandModelsText: t('opencode.provider.expandModels'),
      collapseModelsText: t('opencode.provider.collapseModels'),
      profileLabel: t('opencode.provider.sourceProfile'),
      siteTypeLabel: t('opencode.provider.siteType'),
      loadingTokenText: t('opencode.provider.loadingApiKey'),
      tokenResolvedText: t('opencode.provider.apiKeyReady'),
      retryResolveText: t('opencode.provider.retryResolve'),
      searchPlaceholder: t('opencode.provider.searchPlaceholder'),
      confirmTitle: t('opencode.provider.importAllApiHubOpenAiCompatTitle'),
      confirmOkText: t('opencode.provider.importAllApiHubReviewConfirm'),
    }),
    [t]
  );

  const mapProviderToItem = React.useCallback(
    (
      provider: OpenCodeAllApiHubProvider,
      modelState?: AllApiHubProviderModelsState
    ): ExternalProviderDisplayItem<OpenCodeProvider> => ({
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
      config: provider.providerConfig,
      secondaryLabel: provider.npm,
    }),
    []
  );

  const getConfirmSections = React.useCallback(
    (providers: OpenCodeAllApiHubProvider[]) =>
      [
        providers.filter((provider) => provider.npm === '@ai-sdk/openai-compatible').length > 0
          ? {
              description: t('opencode.provider.importAllApiHubOpenAiCompatDesc'),
              providerNames: providers
                .filter((provider) => provider.npm === '@ai-sdk/openai-compatible')
                .map((provider) => provider.name),
            }
          : null,
        providers.filter((provider) => !provider.hasApiKey).length > 0
          ? {
              description: t('opencode.provider.importAllApiHubMissingApiKeyDesc'),
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
      listProviders={listOpenCodeAllApiHubProviders}
      resolveProviders={resolveOpenCodeAllApiHubProviders}
      onCancel={onClose}
      onImport={onImport}
      texts={texts}
      getProviderId={(provider) => provider.providerId}
      getProviderType={(provider) => provider.npm}
      mapProviderToItem={mapProviderToItem}
      getConfirmSections={getConfirmSections}
    />
  );
};

export default ImportFromAllApiHubModal;
