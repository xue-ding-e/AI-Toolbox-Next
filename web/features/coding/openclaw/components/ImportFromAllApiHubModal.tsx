import React from 'react';
import { message } from 'antd';
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

  const items = React.useMemo<ExternalProviderDisplayItem<OpenClawProviderConfig>[]>(
    () =>
      (result?.providers || []).map((provider) => ({
        providerId: provider.providerId,
        name: provider.name,
        baseUrl: provider.baseUrl || undefined,
        accountLabel: provider.accountLabel,
        siteName: provider.siteName || undefined,
        siteType: provider.siteType || undefined,
        sourceProfileName: provider.sourceProfileName,
        sourceExtensionId: provider.sourceExtensionId,
        isDisabled: provider.isDisabled,
        hasApiKey: provider.hasApiKey,
        apiKeyPreview: provider.apiKeyPreview,
        balanceUsd: provider.balanceUsd,
        balanceCny: provider.balanceCny,
        config: provider.config,
        secondaryLabel: provider.apiProtocol,
      })),
    [result]
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
      profileLabel={t('openclaw.providers.sourceProfile')}
      siteTypeLabel={t('openclaw.providers.siteType')}
      loadingTokenText={t('openclaw.providers.loadingApiKey')}
      tokenResolvedText={t('openclaw.providers.apiKeyReady')}
      searchPlaceholder={t('openclaw.providers.searchPlaceholder')}
      onCancel={onCancel}
      onImport={handleImport}
      onResolveToken={handleResolveToken}
    />
  );
};

export default ImportFromAllApiHubModal;
