import React from 'react';
import { useTranslation } from 'react-i18next';
import type { ExternalProviderDisplayItem } from '@/components/common/ImportExternalProvidersModal/types';
import ImportFromAllApiHubModalBase from './ImportFromAllApiHubModal';
import type { AllApiHubProviderModelsState } from '../allApiHubModelsCache';
import type {
  AllApiHubProviderItem,
  AllApiHubProvidersResult,
} from '@/types/allApiHub';

interface Props {
  open: boolean;
  existingProviderIds: string[];
  onCancel: () => void;
  onImport: (providers: AllApiHubProviderItem[]) => void;
  listProviders: () => Promise<AllApiHubProvidersResult>;
  resolveProviders: (providerIds: string[]) => Promise<AllApiHubProviderItem[]>;
}

/**
 * 面向任意工具的共享 All API Hub 导入弹窗。
 * 后端各模块的 list/resolve 命令返回统一的 `AllApiHubProviderItem`(config 已按工具转换),
 * 因此弹窗逻辑与展示父共享;页面仅需传入该工具的 list/resolve service。
 */
const ImportFromAllApiHubModalForTool: React.FC<Props> = ({
  open,
  existingProviderIds,
  onCancel,
  onImport,
  listProviders,
  resolveProviders,
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
      provider: AllApiHubProviderItem,
      modelState?: AllApiHubProviderModelsState
    ): ExternalProviderDisplayItem<Record<string, unknown>> => ({
      providerId: provider.providerId,
      name: provider.name,
      baseUrl: provider.baseUrl,
      accountLabel: provider.accountLabel,
      siteName: provider.siteName,
      siteType: provider.siteType,
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
    (providers: AllApiHubProviderItem[]) => {
      const sections: { description: string; providerNames: string[] }[] = [];
      const openaiProtocol = providers.filter(
        (provider) => provider.apiProtocol === 'openai-completions'
      );
      if (openaiProtocol.length > 0) {
        sections.push({
          description: t('openclaw.providers.importAllApiHubProtocolDesc'),
          providerNames: openaiProtocol.map((provider) => provider.name),
        });
      }
      const noKey = providers.filter((provider) => !provider.hasApiKey);
      if (noKey.length > 0) {
        sections.push({
          description: t('openclaw.providers.importAllApiHubMissingApiKeyDesc'),
          providerNames: noKey.map((provider) => provider.name),
        });
      }
      return sections;
    },
    [t]
  );

  return (
    <ImportFromAllApiHubModalBase
      open={open}
      providerTypes={[]}
      existingProviderIds={existingProviderIds}
      listProviders={listProviders}
      resolveProviders={resolveProviders}
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

export default ImportFromAllApiHubModalForTool;