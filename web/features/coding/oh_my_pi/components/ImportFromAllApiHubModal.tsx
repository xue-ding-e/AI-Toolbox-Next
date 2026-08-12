import React from 'react';
import { useTranslation } from 'react-i18next';

import type { ExternalProviderDisplayItem } from '@/components/common/ImportExternalProvidersModal/types';
import ImportFromAllApiHubModalBase from '@/features/coding/shared/allApiHub/ImportFromAllApiHubModal';
import type { AllApiHubProviderModelsState } from '@/features/coding/shared/allApiHubModelsCache';
import {
  listOpenCodeAllApiHubProviders,
  resolveOpenCodeAllApiHubProviders,
  type OpenCodeAllApiHubProvider,
} from '@/services/opencodeApi';
import type { OpenCodeProvider } from '@/types/opencode';

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
      title: t('ohMyPi.provider.importAllApiHubModalTitle'),
      noProvidersText: t('ohMyPi.provider.noAllApiHubProviders'),
      cancelText: t('common.cancel'),
      importButtonText: t('ohMyPi.provider.importSelected'),
      selectAllText: t('ohMyPi.provider.selectAllProviders'),
      deselectAllText: t('ohMyPi.provider.deselectAllProviders'),
      existingTagText: t('ohMyPi.provider.providerExists'),
      noApiKeyTagText: t('ohMyPi.provider.apiKeyMissing'),
      disabledTagText: t('ohMyPi.provider.disabled'),
      balanceLabelText: t('ohMyPi.provider.balance'),
      modelsLabelText: t('ohMyPi.provider.models'),
      loadingModelsText: t('ohMyPi.provider.loadingModels'),
      emptyModelsText: t('ohMyPi.provider.emptyModels'),
      modelsErrorText: t('ohMyPi.provider.modelsLoadFailed'),
      unsupportedModelsText: t('ohMyPi.provider.unsupportedModels'),
      expandModelsText: t('ohMyPi.provider.expandModels'),
      collapseModelsText: t('ohMyPi.provider.collapseModels'),
      profileLabel: t('ohMyPi.provider.sourceProfile'),
      siteTypeLabel: t('ohMyPi.provider.siteType'),
      loadingTokenText: t('ohMyPi.provider.loadingApiKey'),
      tokenResolvedText: t('ohMyPi.provider.apiKeyReady'),
      retryResolveText: t('ohMyPi.provider.retryResolve'),
      searchPlaceholder: t('ohMyPi.provider.searchPlaceholder'),
      confirmTitle: t('ohMyPi.provider.importAllApiHubOpenAiCompatTitle'),
      confirmOkText: t('ohMyPi.provider.importAllApiHubReviewConfirm'),
    }),
    [t],
  );

  const mapProviderToItem = React.useCallback(
    (
      provider: OpenCodeAllApiHubProvider,
      modelState?: AllApiHubProviderModelsState,
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
    [],
  );

  const getConfirmSections = React.useCallback(
    (providers: OpenCodeAllApiHubProvider[]) =>
      [
        providers.filter((provider) => provider.npm === '@ai-sdk/openai-compatible').length > 0
          ? {
              description: t('ohMyPi.provider.importAllApiHubOpenAiCompatDesc'),
              providerNames: providers
                .filter((provider) => provider.npm === '@ai-sdk/openai-compatible')
                .map((provider) => provider.name),
            }
          : null,
        providers.filter((provider) => !provider.hasApiKey).length > 0
          ? {
              description: t('ohMyPi.provider.importAllApiHubMissingApiKeyDesc'),
              providerNames: providers
                .filter((provider) => !provider.hasApiKey)
                .map((provider) => provider.name),
            }
          : null,
      ].filter((section): section is { description: string; providerNames: string[] } => !!section),
    [t],
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
