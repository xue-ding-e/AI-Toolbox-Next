import React from 'react';
import { message } from 'antd';
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

  const items = React.useMemo<ExternalProviderDisplayItem<OpenCodeProvider>[]>(
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
        config: provider.providerConfig,
        secondaryLabel: provider.npm,
      })),
    [result]
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
      profileLabel={t('opencode.provider.sourceProfile')}
      siteTypeLabel={t('opencode.provider.siteType')}
      loadingTokenText={t('opencode.provider.loadingApiKey')}
      tokenResolvedText={t('opencode.provider.apiKeyReady')}
      searchPlaceholder={t('opencode.provider.searchPlaceholder')}
      onCancel={onClose}
      onImport={handleImport}
      onResolveToken={handleResolveToken}
    />
  );
};

export default ImportFromAllApiHubModal;
