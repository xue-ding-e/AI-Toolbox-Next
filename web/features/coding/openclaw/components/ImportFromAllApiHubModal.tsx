import React from 'react';
import { message, Modal, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import ImportExternalProvidersModal from '@/components/common/ImportExternalProvidersModal';
import type { ExternalProviderDisplayItem } from '@/components/common/ImportExternalProvidersModal/types';
import {
  listOpenClawAllApiHubProviders,
  resolveOpenClawAllApiHubProviders,
  type OpenClawAllApiHubProvider,
  type OpenClawAllApiHubProvidersResult,
} from '@/services/openclawApi';
import type { OpenClawProviderConfig } from '@/types/openclaw';
import {
  getCachedAllApiHubProviderModelsState,
  refreshAllApiHubProviderModelsInBackground,
  type AllApiHubProviderModelsState,
} from '@/features/coding/shared/allApiHubModelsCache';

const { Text } = Typography;

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
  const [loading, setLoading] = React.useState(false);
  const [result, setResult] = React.useState<OpenClawAllApiHubProvidersResult | null>(null);
  const [providerModelsState, setProviderModelsState] = React.useState<Record<string, AllApiHubProviderModelsState>>({});

  const loadProviders = React.useCallback(async () => {
    setLoading(true);
    try {
      const data = await listOpenClawAllApiHubProviders();
      setResult(data);
      if (data.message && data.providers.length === 0) {
        message.warning(data.message);
      }
    } catch (error) {
      console.error('Failed to load All API Hub providers for OpenClaw:', error);
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

  const items = React.useMemo<ExternalProviderDisplayItem<OpenClawProviderConfig>[]>(
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
          config: provider.config,
          secondaryLabel: provider.apiProtocol,
        };
      }),
    [providerModelsState, result]
  );

  const handleResolveToken = async (providerId: string) => {
    const resolved = await resolveOpenClawAllApiHubProviders([providerId]);
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

  const handleImport = (selected: ExternalProviderDisplayItem<OpenClawProviderConfig>[]) => {
    const selectedProviders = (result?.providers || []).filter((provider) =>
      selected.some((item) => item.providerId === provider.providerId)
    );
    const guessedProtocolProviders = selectedProviders.filter(
      (provider) => provider.apiProtocol === 'openai-completions'
    );

    if (guessedProtocolProviders.length > 0) {
      Modal.confirm({
        title: t('openclaw.providers.importAllApiHubProtocolTitle'),
        content: (
          <div>
            <Text>{t('openclaw.providers.importAllApiHubProtocolDesc')}</Text>
            <div style={{ marginTop: 8 }}>
              <Text type="secondary">
                {guessedProtocolProviders.map((provider) => provider.name).join('、')}
              </Text>
            </div>
          </div>
        ),
        okText: t('openclaw.providers.importAllApiHubProtocolConfirm'),
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
      title={t('openclaw.providers.importFromAllApiHub')}
      loading={loading}
      items={items}
      existingProviderIds={existingProviderIds}
      emptyDescription={result?.message || t('openclaw.providers.noAllApiHubProviders')}
      cancelText={t('common.cancel')}
      importButtonText={t('openclaw.providers.importSelected')}
      selectAllText={t('openclaw.providers.selectAll')}
      deselectAllText={t('openclaw.providers.deselectAll')}
      existingTagText={t('openclaw.providers.alreadyExists')}
      noApiKeyTagText={t('openclaw.providers.apiKeyMissing')}
      disabledTagText={t('openclaw.providers.disabled')}
      balanceLabelText={t('openclaw.providers.balance')}
      accountLabelText={t('openclaw.providers.accountLabel')}
      modelsLabelText={t('openclaw.providers.models')}
      loadingModelsText={t('openclaw.providers.loadingModels')}
      emptyModelsText={t('openclaw.providers.emptyModels')}
      modelsErrorText={t('openclaw.providers.modelsLoadFailed')}
      unsupportedModelsText={t('openclaw.providers.unsupportedModels')}
      unsupportedImportText={t('openclaw.providers.unsupportedImport')}
      expandModelsText={t('openclaw.providers.expandModels')}
      collapseModelsText={t('openclaw.providers.collapseModels')}
      profileLabel={t('openclaw.providers.sourceProfile')}
      siteTypeLabel={t('openclaw.providers.siteType')}
      loadingTokenText={t('openclaw.providers.loadingApiKey')}
      tokenResolvedText={t('openclaw.providers.apiKeyReady')}
      retryResolveText={t('openclaw.providers.retryResolve')}
      searchPlaceholder={t('openclaw.providers.searchPlaceholder')}
      onCancel={onCancel}
      onImport={handleImport}
      onResolveToken={handleResolveToken}
    />
  );
};

export default ImportFromAllApiHubModal;
