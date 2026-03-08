import React from 'react';
import { message, Modal, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import ImportExternalProvidersModal from '@/components/common/ImportExternalProvidersModal';
import type { ExternalProviderDisplayItem } from '@/components/common/ImportExternalProvidersModal/types';
import {
  getCachedAllApiHubProviderModelsState,
  refreshAllApiHubProviderModelsInBackground,
  type AllApiHubProviderModelsState,
} from '../allApiHubModelsCache';

const { Text } = Typography;

interface AllApiHubProvidersResultLike<TProvider> {
  providers: TProvider[];
  message?: string;
}

interface ConfirmSection {
  description: string;
  providerNames: string[];
}

interface ModalTexts {
  title: string;
  noProvidersText: string;
  cancelText: string;
  importButtonText: string;
  selectAllText: string;
  deselectAllText: string;
  existingTagText: string;
  noApiKeyTagText: string;
  disabledTagText: string;
  balanceLabelText: string;
  modelsLabelText: string;
  loadingModelsText: string;
  emptyModelsText: string;
  modelsErrorText: string;
  unsupportedModelsText: string;
  expandModelsText: string;
  collapseModelsText: string;
  profileLabel: string;
  siteTypeLabel: string;
  loadingTokenText: string;
  tokenResolvedText: string;
  retryResolveText: string;
  searchPlaceholder: string;
  confirmTitle: string;
  confirmOkText: string;
}

interface Props<
  TProvider,
  TConfig,
  TResult extends AllApiHubProvidersResultLike<TProvider> = AllApiHubProvidersResultLike<TProvider>,
> {
  open: boolean;
  providerTypes: string[];
  existingProviderIds: string[];
  listProviders: () => Promise<TResult>;
  resolveProviders: (providerIds: string[]) => Promise<TProvider[]>;
  onCancel: () => void;
  onImport: (providers: TProvider[]) => void;
  texts: ModalTexts;
  getProviderId: (provider: TProvider) => string;
  getProviderType: (provider: TProvider) => string | undefined;
  mapProviderToItem: (
    provider: TProvider,
    modelState?: AllApiHubProviderModelsState
  ) => ExternalProviderDisplayItem<TConfig>;
  getConfirmSections: (providers: TProvider[]) => ConfirmSection[];
}

function ImportFromAllApiHubModal<
  TProvider,
  TConfig,
  TResult extends AllApiHubProvidersResultLike<TProvider> = AllApiHubProvidersResultLike<TProvider>,
>({
  open,
  providerTypes,
  existingProviderIds,
  listProviders,
  resolveProviders,
  onCancel,
  onImport,
  texts,
  getProviderId,
  getProviderType,
  mapProviderToItem,
  getConfirmSections,
}: Props<TProvider, TConfig, TResult>) {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [result, setResult] = React.useState<TResult | null>(null);
  const [providerModelsState, setProviderModelsState] = React.useState<Record<string, AllApiHubProviderModelsState>>({});
  const hasProviderTypeFilter = providerTypes.length > 0;

  const filteredProviders = React.useMemo(
    () =>
      (result?.providers || []).filter(
        (provider) =>
          !hasProviderTypeFilter || providerTypes.includes(getProviderType(provider) || '')
      ),
    [getProviderType, hasProviderTypeFilter, providerTypes, result]
  );

  const loadProviders = React.useCallback(async () => {
    setLoading(true);
    try {
      const data = await listProviders();
      setResult(data);
      const matched = data.providers.filter(
        (provider) =>
          !hasProviderTypeFilter || providerTypes.includes(getProviderType(provider) || '')
      );
      if (data.message && matched.length === 0) {
        message.warning(data.message);
      }
    } catch (error) {
      console.error('Failed to load All API Hub providers:', error);
      message.error(t('common.error'));
    } finally {
      setLoading(false);
    }
  }, [getProviderType, hasProviderTypeFilter, listProviders, providerTypes, t]);

  React.useEffect(() => {
    if (open) {
      loadProviders();
    }
  }, [open, loadProviders]);

  const providerIdsKey = React.useMemo(
    () => filteredProviders.map((provider) => getProviderId(provider)).join('|'),
    [filteredProviders, getProviderId]
  );

  React.useEffect(() => {
    if (!open || filteredProviders.length === 0) {
      setProviderModelsState({});
      return;
    }

    const providerIds = filteredProviders.map((provider) => getProviderId(provider));
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
  }, [filteredProviders, getProviderId, open, providerIdsKey]);

  const items = React.useMemo<ExternalProviderDisplayItem<TConfig>[]>(
    () => filteredProviders.map((provider) => mapProviderToItem(provider, providerModelsState[getProviderId(provider)])),
    [filteredProviders, getProviderId, mapProviderToItem, providerModelsState]
  );

  const handleResolveToken = React.useCallback(async (providerId: string) => {
    const resolved = await resolveProviders([providerId]);
    const matched = resolved.find(
      (provider) =>
        getProviderId(provider) === providerId &&
        (!hasProviderTypeFilter || providerTypes.includes(getProviderType(provider) || ''))
    );

    if (!matched) {
      return false;
    }

    setResult((prev) => {
      if (!prev) {
        return prev;
      }

      return {
        ...prev,
        providers: prev.providers.map((provider) =>
          getProviderId(provider) === providerId ? matched : provider
        ),
      };
    });

    const matchedItem = mapProviderToItem(matched);
    return matchedItem.hasApiKey;
  }, [getProviderId, getProviderType, hasProviderTypeFilter, mapProviderToItem, providerTypes, resolveProviders]);

  const handleImport = (selected: ExternalProviderDisplayItem<TConfig>[]) => {
    const selectedProviders = filteredProviders.filter((provider) =>
      selected.some((item) => item.providerId === getProviderId(provider))
    );
    const confirmSections = getConfirmSections(selectedProviders);

    if (confirmSections.length > 0) {
      Modal.confirm({
        title: texts.confirmTitle,
        content: (
          <div>
            {confirmSections.map((section, index) => (
              <div key={section.description} style={{ marginTop: index > 0 ? 16 : 0 }}>
                <Text>
                  {confirmSections.length > 1 ? `${index + 1}. ${section.description}` : section.description}
                </Text>
                <div style={{ marginTop: 8 }}>
                  <Text type="secondary">{section.providerNames.join('、')}</Text>
                </div>
              </div>
            ))}
          </div>
        ),
        okText: texts.confirmOkText,
        cancelText: texts.cancelText,
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
      title={texts.title}
      loading={loading}
      items={items}
      existingProviderIds={existingProviderIds}
      emptyDescription={result?.message || texts.noProvidersText}
      cancelText={texts.cancelText}
      importButtonText={texts.importButtonText}
      selectAllText={texts.selectAllText}
      deselectAllText={texts.deselectAllText}
      existingTagText={texts.existingTagText}
      noApiKeyTagText={texts.noApiKeyTagText}
      disabledTagText={texts.disabledTagText}
      balanceLabelText={texts.balanceLabelText}
      modelsLabelText={texts.modelsLabelText}
      loadingModelsText={texts.loadingModelsText}
      emptyModelsText={texts.emptyModelsText}
      modelsErrorText={texts.modelsErrorText}
      unsupportedModelsText={texts.unsupportedModelsText}
      expandModelsText={texts.expandModelsText}
      collapseModelsText={texts.collapseModelsText}
      profileLabel={texts.profileLabel}
      siteTypeLabel={texts.siteTypeLabel}
      loadingTokenText={texts.loadingTokenText}
      tokenResolvedText={texts.tokenResolvedText}
      retryResolveText={texts.retryResolveText}
      searchPlaceholder={texts.searchPlaceholder}
      onCancel={onCancel}
      onImport={handleImport}
      onResolveToken={handleResolveToken}
    />
  );
}

export default ImportFromAllApiHubModal;
