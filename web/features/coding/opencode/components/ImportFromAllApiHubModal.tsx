import React from 'react';
import { message, Modal, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import ImportExternalProvidersModal from '@/components/common/ImportExternalProvidersModal';
import type { ExternalProviderDisplayItem } from '@/components/common/ImportExternalProvidersModal/types';
import {
  listOpenCodeAllApiHubProviders,
  resolveOpenCodeAllApiHubProviders,
  type OpenCodeAllApiHubProvider,
  type OpenCodeAllApiHubProvidersResult,
} from '@/services/opencodeApi';
import type { OpenCodeProvider } from '@/types/opencode';
import {
  getCachedAllApiHubProviderModelsState,
  refreshAllApiHubProviderModelsInBackground,
  type AllApiHubProviderModelsState,
} from '@/features/coding/shared/allApiHubModelsCache';

const { Text } = Typography;

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
  const [loading, setLoading] = React.useState(false);
  const [result, setResult] = React.useState<OpenCodeAllApiHubProvidersResult | null>(null);
  const [providerModelsState, setProviderModelsState] = React.useState<Record<string, AllApiHubProviderModelsState>>({});

  const loadProviders = React.useCallback(async () => {
    setLoading(true);
    try {
      const data = await listOpenCodeAllApiHubProviders();
      setResult(data);
      if (data.message && data.providers.length === 0) {
        message.warning(data.message);
      }
    } catch (error) {
      console.error('Failed to load All API Hub providers:', error);
      message.error(t('common.error'));
    } finally {
      setLoading(false);
    }
  }, [t]);

  React.useEffect(() => {
    if (open) {
      loadProviders();
    }
  }, [open, loadProviders]);

  const providerIdsKey = React.useMemo(
    () => (result?.providers || []).map((provider) => provider.providerId).join('|'),
    [result]
  );

  React.useEffect(() => {
    if (!open || !result?.providers.length) {
      setProviderModelsState({});
      return;
    }

    const providerIds = result.providers.map((provider) => provider.providerId);
    const cachedState = Object.fromEntries(
      providerIds
        .map((providerId) => [providerId, getCachedAllApiHubProviderModelsState(providerId)])
        .filter((entry): entry is [string, AllApiHubProviderModelsState] => !!entry[1])
    );
    setProviderModelsState(cachedState);

    let cancelled = false;

    providerIds.forEach((providerId) => {
      setProviderModelsState((prev) => ({
        ...prev,
        [providerId]: {
          models: prev[providerId]?.models || cachedState[providerId]?.models || [],
          status: 'loading',
          error: prev[providerId]?.error || cachedState[providerId]?.error,
          updatedAt: prev[providerId]?.updatedAt || cachedState[providerId]?.updatedAt,
        },
      }));
    });

    void refreshAllApiHubProviderModelsInBackground(providerIds, (providerId, state) => {
      if (cancelled) {
        return;
      }
      setProviderModelsState((prev) => ({
        ...prev,
        [providerId]: state,
      }));
    });

    return () => {
      cancelled = true;
    };
  }, [open, providerIdsKey, result]);

  const items = React.useMemo<ExternalProviderDisplayItem<OpenCodeProvider>[]>(
    () =>
      (result?.providers || []).map((provider) => {
        const modelState = providerModelsState[provider.providerId];
        return {
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
        };
      }),
    [providerModelsState, result]
  );

  const handleResolveToken = async (providerId: string) => {
    const resolved = await resolveOpenCodeAllApiHubProviders([providerId]);
    if (resolved.length === 0) {
      return false;
    }

    setResult((prev) => {
      if (!prev) return prev;
      return {
        ...prev,
        providers: prev.providers.map((provider) =>
          provider.providerId === providerId ? resolved[0] : provider
        ),
      };
    });
    return resolved[0].hasApiKey;
  };

  const handleImport = (selected: ExternalProviderDisplayItem<OpenCodeProvider>[]) => {
    const selectedProviders = (result?.providers || []).filter((provider) =>
      selected.some((item) => item.providerId === provider.providerId)
    );
    const openaiCompatibleProviders = selectedProviders.filter(
      (provider) => provider.npm === '@ai-sdk/openai-compatible'
    );

    if (openaiCompatibleProviders.length > 0) {
      Modal.confirm({
        title: t('opencode.provider.importAllApiHubOpenAiCompatTitle'),
        content: (
          <div>
            <Text>{t('opencode.provider.importAllApiHubOpenAiCompatDesc')}</Text>
            <div style={{ marginTop: 8 }}>
              <Text type="secondary">
                {openaiCompatibleProviders.map((provider) => provider.name).join('、')}
              </Text>
            </div>
          </div>
        ),
        okText: t('opencode.provider.importAllApiHubOpenAiCompatConfirm'),
        cancelText: t('common.cancel'),
        onOk: () => {
          onImport(selectedProviders);
        },
      });
      return;
    }

    onImport(selectedProviders);
  };

  return (
    <ImportExternalProvidersModal
      open={open}
      title={t('opencode.provider.importAllApiHubModalTitle')}
      loading={loading}
      items={items}
      existingProviderIds={existingProviderIds}
      emptyDescription={result?.message || t('opencode.provider.noAllApiHubProviders')}
      cancelText={t('common.cancel')}
      importButtonText={t('opencode.provider.importSelected')}
      selectAllText={t('opencode.provider.selectAllProviders')}
      deselectAllText={t('opencode.provider.deselectAllProviders')}
      existingTagText={t('opencode.provider.providerExists')}
      noApiKeyTagText={t('opencode.provider.apiKeyMissing')}
      disabledTagText={t('opencode.provider.disabled')}
      balanceLabelText={t('opencode.provider.balance')}
      accountLabelText={t('opencode.provider.accountLabel')}
      modelsLabelText={t('opencode.provider.models')}
      loadingModelsText={t('opencode.provider.loadingModels')}
      emptyModelsText={t('opencode.provider.emptyModels')}
      modelsErrorText={t('opencode.provider.modelsLoadFailed')}
      unsupportedModelsText={t('opencode.provider.unsupportedModels')}
      unsupportedImportText={t('opencode.provider.unsupportedImport')}
      expandModelsText={t('opencode.provider.expandModels')}
      collapseModelsText={t('opencode.provider.collapseModels')}
      profileLabel={t('opencode.provider.sourceProfile')}
      siteTypeLabel={t('opencode.provider.siteType')}
      loadingTokenText={t('opencode.provider.loadingApiKey')}
      tokenResolvedText={t('opencode.provider.apiKeyReady')}
      retryResolveText={t('opencode.provider.retryResolve')}
      searchPlaceholder={t('opencode.provider.searchPlaceholder')}
      onCancel={onClose}
      onImport={handleImport}
      onResolveToken={handleResolveToken}
    />
  );
};

export default ImportFromAllApiHubModal;
