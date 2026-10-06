import React from 'react';
import { Alert, Button, Modal, Space, message } from 'antd';
import {
  DatabaseOutlined,
  FileTextOutlined,
  ImportOutlined,
  MessageOutlined,
} from '@ant-design/icons';
import { DndContext, closestCenter, type DragEndEvent } from '@dnd-kit/core';
import { restrictToVerticalAxis } from '@dnd-kit/modifiers';
import {
  SortableContext,
  arrayMove,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { useTranslation } from 'react-i18next';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import SectionSidebarLayout, {
  type SidebarSectionMarker,
} from '@/components/layout/SectionSidebarLayout/SectionSidebarLayout';
import SidebarSettingsModal from '@/components/common/SidebarSettingsModal';
import { useKeepAlive } from '@/components/layout/KeepAliveOutlet';
import CodingPageHeader from '@/features/coding/shared/CodingPageHeader';
import ProviderListSection from '@/features/coding/shared/ProviderListSection';
import RootDirectoryModal from '@/features/coding/shared/RootDirectoryModal';
import useRootDirectoryConfig from '@/features/coding/shared/useRootDirectoryConfig';
import { GlobalPromptSettings } from '@/features/coding/shared/prompt';
import { SessionManagerPanel } from '@/features/coding/shared/sessionManager';
import {
  PROVIDER_SORT_MODES,
  backupProvidersBeforeDelete,
  filterProviderItems,
  sortProviderItems,
  useProviderBatchSelection,
  useProviderListSort,
} from '@/features/coding/shared/providerList';
import {
  buildProviderConnectivityBatchTarget,
  runProviderConnectivityBatch,
} from '@/features/coding/shared/providerConnectivity/batchTest';
import {
  buildFavoriteProviderStorageKey,
  dedupeFavoriteProvidersByPayload,
  getFavoriteProviderPayload,
  isFavoriteProviderForSource,
  type ZcodeFavoriteProviderPayload,
} from '@/features/coding/shared/favoriteProviders';
import {
  deleteFavoriteProvider,
  listFavoriteProviders,
  upsertFavoriteProvider,
  type OpenCodeFavoriteProvider,
} from '@/services/opencodeApi';
import ImportProviderModal from '@/components/common/ImportProviderModal';
import ImportFromCcSwitchModal from '@/features/coding/shared/ccSwitch/ImportFromCcSwitchModal';
import AllApiHubIcon from '@/components/common/AllApiHubIcon';
import ZcodeImportFromAllApiHubModal from '../components/ImportFromAllApiHubModal';
import { hasAllApiHubExtension } from '@/services/appApi';
import type { OpenCodeAllApiHubProvider } from '@/services/opencodeApi';
import { hasCcSwitchDb, type CcSwitchProviderCandidate } from '@/services/ccSwitchApi';
import { buildZcodeFavoriteProviderConfig } from '../utils/zcodeFavoriteProvider';
import {
  buildZcodeSettingsConfigFromImport,
  extractZcodeProviderFromAllApiHub,
  extractZcodeProviderFromCcSwitch,
} from '../utils/zcodeImportMapping';
import type { ProviderConnectivityStatusItem } from '@/components/common/ProviderCard/types';
import FetchModelsModal from '@/components/common/FetchModelsModal';
import JsonPreviewModal from '@/components/common/JsonPreviewModal';
import type { FetchModelsApplyResult } from '@/components/common/FetchModelsModal/types';
import { useSettingsStore } from '@/stores';
import { refreshTrayMenu } from '@/services/appApi';
import {
  createZcodeProvider,
  deleteZcodeProvider,
  getZcodeCommonConfig,
  getZcodeConfigFilePath,
  getZcodeGenerationStatus,
  getZcodeRootPathInfo,
  listZcodeProviders,
  readZcodeSettings,
  reorderZcodeProviders,
  revealZcodeConfigFolder,
  saveZcodeCommonConfig,
  saveZcodeProvider,
  selectZcodeProvider,
  updateZcodeProvider,
} from '@/services/zcodeApi';
import { zcodePromptApi } from '@/services/zcodePromptApi';
import type {
  ConfigPathInfo,
  ZcodeModelRow,
  ZcodeProvider,
  ZcodeSettingsConfig,
} from '@/types/zcode';
import ZcodeProviderCard from '../components/ZcodeProviderCard';
import ZcodeCommonConfigModal from '../components/ZcodeCommonConfigModal';
import ZcodeModelFormModal from '../components/ZcodeModelFormModal';
import ZcodeProviderFormModal from '../components/ZcodeProviderFormModal';
import { parseZcodeProviderSettings, resolveZcodeDefaultModelId } from '../utils/zcodeSettingsConfig';
import {
  buildZcodeModelRowFromPreset,
  preferredPresetNpmTypes,
} from '../utils/zcodeModelFields';
import { findPresetModelById } from '@/constants/presetModels';


const ZcodePage: React.FC = () => {
  const { t } = useTranslation();
  const { sidebarHiddenByPage, setSidebarHidden } = useSettingsStore();
  const { isActive } = useKeepAlive();

  const [loading, setLoading] = React.useState(false);
  const [configPath, setConfigPath] = React.useState('');
  const [rootPathInfo, setRootPathInfo] = React.useState<ConfigPathInfo | null>(null);
  const [providers, setProviders] = React.useState<ZcodeProvider[]>([]);
  const [hasNewGenerationRegistry, setHasNewGenerationRegistry] = React.useState(true);
  const [providerListCollapsed, setProviderListCollapsed] = React.useState(false);
  const [promptExpandNonce, setPromptExpandNonce] = React.useState(0);
  const [sessionManagerExpandNonce, setSessionManagerExpandNonce] = React.useState(0);
  const [formModalOpen, setFormModalOpen] = React.useState(false);
  const [editingProvider, setEditingProvider] = React.useState<ZcodeProvider | null>(null);
  const [settingsModalOpen, setSettingsModalOpen] = React.useState(false);
  const [commonConfigModalOpen, setCommonConfigModalOpen] = React.useState(false);
  const [providerKeyword, setProviderKeyword] = React.useState('');
  const [connectivityStatuses, setConnectivityStatuses] = React.useState<
    Record<string, ProviderConnectivityStatusItem>
  >({});
  const [batchTestingProviders, setBatchTestingProviders] = React.useState(false);
  const [modelModal, setModelModal] = React.useState<{
    provider: ZcodeProvider;
    /** `null` means "add"; otherwise the index of the row being edited. */
    modelIndex: number | null;
    /**
     * Seed row for "add", used by copy. A copy cannot reuse the source id —
     * ZCode keys catalog rows by model id — so it opens the *add* flow
     * prefilled and lets the user name the new one.
     */
    prefill?: ZcodeModelRow;
  } | null>(null);
  const [fetchModelsProviderId, setFetchModelsProviderId] = React.useState<string | null>(null);
  const [fetchModelsModalOpen, setFetchModelsModalOpen] = React.useState(false);
  const [previewModalOpen, setPreviewModalOpen] = React.useState(false);
  const [previewData, setPreviewData] = React.useState<unknown>(null);
  const [importModalOpen, setImportModalOpen] = React.useState(false);
  const [allApiHubImportModalOpen, setAllApiHubImportModalOpen] = React.useState(false);
  const [allApiHubAvailable, setAllApiHubAvailable] = React.useState(false);
  const [ccSwitchImportModalOpen, setCcSwitchImportModalOpen] = React.useState(false);
  const [ccSwitchAvailable, setCcSwitchAvailable] = React.useState(false);
  const [testingModelsFor, setTestingModelsFor] = React.useState<string | null>(null);
  /** Provider whose model list is in batch-delete mode, if any. */
  const [modelBatchDeleteProviderId, setModelBatchDeleteProviderId] = React.useState<string | null>(
    null,
  );
  const [selectedModelIdsByProvider, setSelectedModelIdsByProvider] = React.useState<
    Record<string, string[]>
  >({});

  const sidebarHidden = sidebarHiddenByPage.zcode ?? false;

  const { sortMode, setSortMode, lastUsedAt, noteProviderUsed } = useProviderListSort('zcode');

  // Search and non-custom sort modes bypass sort_index, so dragging would write
  // a stale custom order — dnd is only enabled in custom mode.
  const providerDragDisabled = sortMode !== 'custom' || providerKeyword.trim() !== '';
  const visibleProviders = React.useMemo(
    () =>
      sortProviderItems(
        filterProviderItems(providers, providerKeyword, (provider) => [
          provider.name,
          provider.notes ?? '',
          provider.websiteUrl ?? '',
        ]),
        sortMode,
        { name: (provider) => provider.name, createdAt: (provider) => provider.createdAt },
        (provider) => lastUsedAt(provider.id),
      ),
    [providers, providerKeyword, sortMode, lastUsedAt],
  );

  const sidebarSections = React.useMemo<SidebarSectionMarker[]>(
    () => [
      {
        id: 'zcode-providers',
        title: t('zcode.provider.title', { defaultValue: '供应商' }),
        order: 1,
      },
      {
        id: 'zcode-global-prompt',
        title: t('common.prompt.title'),
        order: 2,
      },
      {
        id: 'zcode-session-manager',
        title: t('sessionManager.title', { defaultValue: '会话管理' }),
        order: 3,
      },
    ],
    [t],
  );

  const loadConfig = React.useCallback(async () => {
    setLoading(true);
    try {
      const [path, nextRootPathInfo, generation, nextProviders] = await Promise.all([
        getZcodeConfigFilePath(),
        getZcodeRootPathInfo(),
        getZcodeGenerationStatus(),
        listZcodeProviders(),
      ]);
      setConfigPath(path);
      setRootPathInfo(nextRootPathInfo);
      setHasNewGenerationRegistry(generation);
      setProviders(nextProviders);
    } catch (error) {
      console.error('Failed to load ZCode config:', error);
      const detail = error instanceof Error ? error.message : String(error);
      void message.error(detail ? `${t('zcode.loadFailed')}：${detail}` : t('zcode.loadFailed'));
    } finally {
      setLoading(false);
    }
  }, [t]);

  React.useEffect(() => {
    if (isActive) {
      void loadConfig();
    }
  }, [isActive, loadConfig]);

  const {
    rootDirectoryModalOpen,
    setRootDirectoryModalOpen,
    getRootDirectoryModalProps,
    handleSaveRootDirectory,
    handleResetRootDirectory,
  } = useRootDirectoryConfig({
    t,
    translationKeyPrefix: 'zcode',
    defaultConfig: '{}',
    loadConfig,
    getCommonConfig: getZcodeCommonConfig,
    saveCommonConfig: async ({ config, rootDir, clearRootDir }) => {
      await saveZcodeCommonConfig({ config, rootDir, clearRootDir });
    },
  });

  const handleOpenFolder = async () => {
    try {
      if (configPath) {
        await revealItemInDir(configPath);
      } else {
        await revealZcodeConfigFolder();
      }
    } catch {
      await revealZcodeConfigFolder();
    }
  };

  const handleApplyProvider = async (provider: ZcodeProvider) => {
    try {
      const modelId = resolveZcodeDefaultModelId(provider.settingsConfig);
      if (!modelId) {
        void message.warning(
          t('zcode.apply.noModel', { defaultValue: '该供应商还没有模型，请先添加模型再应用。' }),
        );
        return;
      }
      await selectZcodeProvider(provider.id, modelId);
      await refreshTrayMenu();
      await loadConfig();
      void message.success(t('zcode.apply.success', { defaultValue: '已设为默认供应商' }));
    } catch (error) {
      console.error('Failed to apply ZCode provider:', error);
      void message.error(String(error));
    }
  };

  /**
   * Copies a provider into the favorites store before it is deleted.
   *
   * The store is shared across CLIs, so a ZCode provider becomes restorable from
   * the provider list's import entry — the only undo a delete has.
   */
  const backUpProviderToFavorites = React.useCallback(async (provider: ZcodeProvider) => {
    await upsertFavoriteProvider(
      buildFavoriteProviderStorageKey('zcode', provider.id),
      buildZcodeFavoriteProviderConfig(provider),
    );
  }, []);

  /**
   * Drops favorites that hold the same provider twice.
   *
   * A record is written both when a provider is deleted and when one is
   * imported, so the same config can land under two keys. The store is shared
   * with the other CLIs, so the duplicates are cleaned here rather than left for
   * whichever tool reads the list next. Nothing else in this page reads the
   * favorites list — the import dialog fetches its own.
   */
  const pruneDuplicateFavoriteProviders = React.useCallback(async () => {
    try {
      const all = await listFavoriteProviders();
      const zcodeFavorites = all.filter((provider) =>
        isFavoriteProviderForSource('zcode', provider),
      );
      const currentStorageKeys = new Set(
        providers.map((provider) => buildFavoriteProviderStorageKey('zcode', provider.id)),
      );
      const { duplicateIds } = dedupeFavoriteProvidersByPayload(zcodeFavorites, currentStorageKeys);
      await Promise.all(
        duplicateIds.map(async (providerId) => {
          try {
            await deleteFavoriteProvider(providerId);
          } catch (error) {
            console.error('Failed to delete duplicate ZCode favorite provider:', error);
          }
        }),
      );
    } catch (error) {
      console.error('Failed to load ZCode favorite providers:', error);
    }
  }, [providers]);

  React.useEffect(() => {
    void pruneDuplicateFavoriteProviders();
  }, [pruneDuplicateFavoriteProviders]);

  /** Adopts providers picked from the shared favorites list. */
  const handleImportFavoriteProviders = React.useCallback(
    async (providersToImport: OpenCodeFavoriteProvider[]) => {
      let importedCount = 0;
      for (const favoriteProvider of providersToImport) {
        const payload = getFavoriteProviderPayload<ZcodeFavoriteProviderPayload>(favoriteProvider);
        if (!payload) {
          continue;
        }
        try {
          const created = await createZcodeProvider({
            name: payload.name,
            category: payload.category,
            settingsConfig: payload.settingsConfig,
            notes: payload.notes,
          });
          // Keep the imported row in the store so it survives a later delete.
          try {
            await upsertFavoriteProvider(
              buildFavoriteProviderStorageKey('zcode', created.id),
              buildZcodeFavoriteProviderConfig(created),
            );
          } catch (favoriteError) {
            console.error('Failed to keep the imported ZCode favorite provider:', favoriteError);
          }
          importedCount += 1;
        } catch (error) {
          console.error('Failed to import ZCode favorite provider:', error);
        }
      }
      void message.success(t('common.success'));
      setImportModalOpen(false);
      await loadConfig();
      await pruneDuplicateFavoriteProviders();
      await refreshTrayMenu();
      return importedCount;
    },
    [loadConfig, pruneDuplicateFavoriteProviders, t],
  );

  /** Whether the two optional import sources are present on this machine. */
  React.useEffect(() => {
    const checkSources = async () => {
      try {
        setAllApiHubAvailable(await hasAllApiHubExtension());
      } catch {
        setAllApiHubAvailable(false);
      }
      try {
        setCcSwitchAvailable(await hasCcSwitchDb());
      } catch {
        setCcSwitchAvailable(false);
      }
    };
    void checkSources();
  }, []);

  const handleImportFromAllApiHub = React.useCallback(
    async (imported: OpenCodeAllApiHubProvider[]) => {
      let ok = 0;
      let fail = 0;
      for (const item of imported) {
        const mapped = extractZcodeProviderFromAllApiHub(item);
        if (!mapped) {
          continue;
        }
        try {
          await createZcodeProvider({
            name: mapped.name,
            category: 'custom',
            settingsConfig: buildZcodeSettingsConfigFromImport(mapped),
            // Remembers where the row came from, so re-importing the same
            // source provider is recognised instead of duplicated.
            sourceProviderId: item.providerId,
          });
          ok += 1;
        } catch (error) {
          console.error('Failed to import ZCode provider from All API Hub:', item.providerId, error);
          fail += 1;
        }
      }
      setAllApiHubImportModalOpen(false);
      if (ok > 0 && fail === 0) {
        void message.success(t('common.allApiHub.importSuccess', { count: ok }));
      } else if (fail > 0) {
        void message.error(t('common.error'));
      }
      await loadConfig();
      await refreshTrayMenu();
    },
    [loadConfig, t],
  );

  const handleImportFromCcSwitch = React.useCallback(
    async (imported: CcSwitchProviderCandidate[]) => {
      const existingSourceIds = new Set(
        providers.map((provider) => provider.sourceProviderId).filter(Boolean),
      );
      let ok = 0;
      let fail = 0;
      for (const candidate of imported) {
        const sourceProviderId = candidate.sourceProviderId ?? candidate.providerId;
        if (existingSourceIds.has(sourceProviderId)) {
          continue;
        }
        const mapped = extractZcodeProviderFromCcSwitch(candidate);
        if (!mapped) {
          continue;
        }
        try {
          await createZcodeProvider({
            name: mapped.name,
            category: 'custom',
            settingsConfig: buildZcodeSettingsConfigFromImport(mapped),
            sourceProviderId,
          });
          ok += 1;
        } catch (error) {
          console.error('Failed to import ZCode provider from CC Switch:', candidate.providerId, error);
          fail += 1;
        }
      }
      setCcSwitchImportModalOpen(false);
      if (ok > 0 && fail === 0) {
        void message.success(t('common.ccSwitch.importSuccess', { count: ok }));
      } else if (ok > 0 && fail > 0) {
        void message.warning(t('common.ccSwitch.importPartial', { ok, fail }));
      } else if (fail > 0) {
        void message.error(t('common.error'));
      }
      await loadConfig();
      await refreshTrayMenu();
    },
    [providers, loadConfig, t],
  );

  const handleDeleteProvider = async (provider: ZcodeProvider) => {
    try {
      await backUpProviderToFavorites(provider);
      await deleteZcodeProvider(provider.id);
      await refreshTrayMenu();
      await loadConfig();
      await pruneDuplicateFavoriteProviders();
    } catch (error) {
      console.error('Failed to delete ZCode provider:', error);
      void message.error(String(error));
    }
  };

  const handleDragEnd = async (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) {
      return;
    }
    const oldIndex = providers.findIndex((provider) => provider.id === active.id);
    const newIndex = providers.findIndex((provider) => provider.id === over.id);
    if (oldIndex < 0 || newIndex < 0) {
      return;
    }
    const nextOrder = arrayMove(providers, oldIndex, newIndex).map((provider) => provider.id);
    // Optimistic: the list reorders immediately and reloads from the backend
    // afterwards, so a failure still converges on the stored order.
    setProviders(arrayMove(providers, oldIndex, newIndex));
    try {
      await reorderZcodeProviders(nextOrder);
      await loadConfig();
    } catch (error) {
      console.error('Failed to reorder ZCode providers:', error);
      void message.error(String(error));
      await loadConfig();
    }
  };

  const handleBatchDeleteProviders = React.useCallback(
    async (ids: string[]): Promise<boolean> => {
      const providersToDelete = providers.filter((provider) => ids.includes(provider.id));
      if (providersToDelete.length === 0) {
        return false;
      }
      try {
        await backupProvidersBeforeDelete(
          providersToDelete,
          backUpProviderToFavorites,
          (provider) => t('common.batch.backupFailed', { name: provider.name }),
        );
        for (const provider of providersToDelete) {
          await deleteZcodeProvider(provider.id);
        }
        await refreshTrayMenu();
        await loadConfig();
        await pruneDuplicateFavoriteProviders();
        void message.success(t('common.success'));
        return true;
      } catch (error) {
        console.error('Failed to batch delete ZCode providers:', error);
        void message.error(error instanceof Error ? error.message : String(error));
        return false;
      }
    },
    [providers, loadConfig, pruneDuplicateFavoriteProviders, backUpProviderToFavorites, t],
  );

  /**
   * Persists a provider's model catalog.
   *
   * ZCode stores models inside `settingsConfig`, so every model edit rewrites
   * that blob and re-projects the provider — the same "DB row alone does not
   * reach ZCode" rule the provider form follows.
   */
  const persistProviderModels = React.useCallback(
    async (provider: ZcodeProvider, nextModels: ZcodeModelRow[]) => {
      const settings = parseZcodeProviderSettings(provider.settingsConfig);
      if (!settings) {
        return;
      }
      const nextSettings: ZcodeSettingsConfig = {
        ...settings,
        models: nextModels,
        defaultModelId: nextModels.find((model) => model.isDefault)?.modelId,
      };
      const settingsConfig = JSON.stringify(nextSettings);
      try {
        const updated = await updateZcodeProvider({
          ...provider,
          settingsConfig,
        });
        await saveZcodeProvider({
          id: updated.id,
          name: updated.name,
          category: updated.category,
          settingsConfig: updated.settingsConfig,
          notes: updated.notes,
        });
        await loadConfig();
      } catch (error) {
        console.error('Failed to save ZCode provider models:', error);
        void message.error(error instanceof Error ? error.message : String(error));
      }
    },
    [loadConfig],
  );

  const handleAddModel = React.useCallback((provider: ZcodeProvider) => {
    setModelModal({ provider, modelIndex: null });
  }, []);

  const handleEditModel = React.useCallback((provider: ZcodeProvider, modelId: string) => {
    const models = parseZcodeProviderSettings(provider.settingsConfig)?.models ?? [];
    const modelIndex = models.findIndex((model) => model.modelId === modelId);
    if (modelIndex >= 0) {
      setModelModal({ provider, modelIndex });
    }
  }, []);

  const handleSubmitModel = React.useCallback(
    async (model: ZcodeModelRow) => {
      if (!modelModal) {
        return;
      }
      const { provider, modelIndex } = modelModal;
      const models = parseZcodeProviderSettings(provider.settingsConfig)?.models ?? [];
      const nextModels =
        modelIndex === null
          ? [
              ...models,
              // The first model becomes the default so a fresh provider is
              // immediately usable.
              { ...model, isDefault: models.length === 0 },
            ]
          : models.map((existing, index) =>
              index === modelIndex ? { ...model, isDefault: existing.isDefault } : existing,
            );
      setModelModal(null);
      await persistProviderModels(provider, nextModels);
    },
    [modelModal, persistProviderModels],
  );

  /**
 * Shows the provider file the CLI will actually read.
 *
 * ZCode has no single applied provider — every enabled one is written at once —
 * so the preview is the whole generated file, not one provider's contribution.
 * A missing file reads as an empty config rather than an error.
 */
  const handlePreviewCurrentConfig = async () => {
    try {
      setPreviewData(await readZcodeSettings());
      setPreviewModalOpen(true);
    } catch (error) {
      console.error('Failed to preview ZCode config:', error);
      message.error(error instanceof Error ? error.message : String(error));
    }
  };

  const handleCopyModel = React.useCallback(
    (provider: ZcodeProvider, modelId: string) => {
      const models = parseZcodeProviderSettings(provider.settingsConfig)?.models ?? [];
      const source = models.find((model) => model.modelId === modelId);
      if (!source) {
        return;
      }
      setModelModal({
        provider,
        modelIndex: null,
        prefill: {
          ...source,
          displayName: `${source.displayName?.trim() || source.modelId.trim()} copy`,
          isDefault: false,
        },
      });
    },
    [],
  );

  const handleDeleteModel = React.useCallback(
    async (provider: ZcodeProvider, modelId: string) => {
      const models = parseZcodeProviderSettings(provider.settingsConfig)?.models ?? [];
      const nextModels = models.filter((model) => model.modelId !== modelId);
      // Dropping the default model would leave the provider unappliable, so the
      // first remaining row inherits the flag.
      if (!nextModels.some((model) => model.isDefault) && nextModels.length > 0) {
        nextModels[0] = { ...nextModels[0], isDefault: true };
      }
      await persistProviderModels(provider, nextModels);
    },
    [persistProviderModels],
  );

  const handleSetPrimaryModel = React.useCallback(
    async (provider: ZcodeProvider, modelId: string) => {
      const models = parseZcodeProviderSettings(provider.settingsConfig)?.models ?? [];
      await persistProviderModels(
        provider,
        models.map((model) => ({ ...model, isDefault: model.modelId === modelId })),
      );
    },
    [persistProviderModels],
  );

  const handleReorderModels = React.useCallback(
    async (provider: ZcodeProvider, orderedModelIds: string[]) => {
      const models = parseZcodeProviderSettings(provider.settingsConfig)?.models ?? [];
      const byId = new Map(models.map((model) => [model.modelId, model]));
      const ordered = orderedModelIds
        .map((id) => byId.get(id))
        .filter((model): model is ZcodeModelRow => Boolean(model));
      // Rows missing from the incoming order keep their previous position.
      for (const model of models) {
        if (!ordered.includes(model)) {
          ordered.push(model);
        }
      }
      await persistProviderModels(provider, ordered);
    },
    [persistProviderModels],
  );

  const batchSelectableIds = React.useMemo(
    () => visibleProviders.map((provider) => provider.id),
    [visibleProviders],
  );
  const providerBatch = useProviderBatchSelection({
    allIds: batchSelectableIds,
    onBatchDelete: handleBatchDeleteProviders,
  });
  const providerBatchDragDisabled = providerDragDisabled || providerBatch.selectionMode;

  const handleBatchTestProviders = React.useCallback(async () => {
    setBatchTestingProviders(true);
    try {
      const targets = visibleProviders.map((provider) => {
        const settings = parseZcodeProviderSettings(provider.settingsConfig);
        const modelIds = (settings?.models ?? []).map((model) => model.modelId);
        return buildProviderConnectivityBatchTarget(
          {
            providerId: provider.id,
            providerName: provider.name,
            providerConfig: {
              options: {
                baseURL: settings?.config?.api?.baseUrl ?? '',
                apiKey: settings?.config?.access?.apiKey ?? '',
                headers: settings?.config?.api?.headers,
              },
            },
            modelIds,
          },
          {
            requireBaseUrl: true,
            requireApiKey: true,
            errorMessages: {
              missingBaseUrl: t('zcode.test.missingBaseUrl', {
                name: provider.name,
              }),
              missingApiKey: t('zcode.test.missingApiKey', { name: provider.name }),
              missingModel: t('zcode.test.missingModel', { name: provider.name }),
            },
          },
        );
      });
      setConnectivityStatuses({});
      await runProviderConnectivityBatch(targets, (providerId, status) => {
        setConnectivityStatuses((previous) => ({ ...previous, [providerId]: status }));
      });
    } finally {
      setBatchTestingProviders(false);
    }
  }, [visibleProviders, t]);

  /** Enters or leaves batch-delete mode for one provider's model list. */
  const handleToggleModelBatchDeleteMode = React.useCallback(
    (provider: ZcodeProvider) => {
      if (modelBatchDeleteProviderId === provider.id) {
        setSelectedModelIdsByProvider({});
        setModelBatchDeleteProviderId(null);
        return;
      }
      setSelectedModelIdsByProvider({});
      setModelBatchDeleteProviderId(provider.id);
    },
    [modelBatchDeleteProviderId],
  );

  const handleToggleModelSelection = React.useCallback(
    (provider: ZcodeProvider, modelId: string, selected: boolean) => {
      setSelectedModelIdsByProvider((previous) => {
        const current = previous[provider.id] ?? [];
        const next = selected
          ? Array.from(new Set([...current, modelId]))
          : current.filter((id) => id !== modelId);
        if (next.length === 0) {
          const nextState = { ...previous };
          delete nextState[provider.id];
          return nextState;
        }
        return { ...previous, [provider.id]: next };
      });
    },
    [],
  );

  const handleBatchDeleteModels = React.useCallback(
    (provider: ZcodeProvider) => {
      const selectedIds = selectedModelIdsByProvider[provider.id] ?? [];
      if (selectedIds.length === 0) {
        return;
      }
      Modal.confirm({
        title: t('common.model.batchDeleteConfirmTitle'),
        content: t('common.model.batchDeleteConfirmContent', { count: selectedIds.length }),
        okText: t('common.confirm'),
        cancelText: t('common.cancel'),
        onOk: async () => {
          const settings = parseZcodeProviderSettings(provider.settingsConfig);
          if (!settings) {
            return;
          }
          const removed = new Set(selectedIds);
          const nextModels = settings.models.filter((model) => !removed.has(model.modelId));
          await persistProviderModels(provider, nextModels);
          setSelectedModelIdsByProvider((previous) => {
            if (!(provider.id in previous)) {
              return previous;
            }
            const nextState = { ...previous };
            delete nextState[provider.id];
            return nextState;
          });
          setModelBatchDeleteProviderId((current) => (current === provider.id ? null : current));
          message.success(t('common.success'));
        },
      });
    },
    [persistProviderModels, selectedModelIdsByProvider, t],
  );

  /** Runs the connectivity probe for one provider's catalog only. */
  const handleTestProviderModels = React.useCallback(
    async (provider: ZcodeProvider) => {
      const settings = parseZcodeProviderSettings(provider.settingsConfig);
      const modelIds = (settings?.models ?? []).map((model) => model.modelId);
      setTestingModelsFor(provider.id);
      setConnectivityStatuses((previous) => {
        const next = { ...previous };
        delete next[provider.id];
        return next;
      });
      try {
        await runProviderConnectivityBatch(
          [
            buildProviderConnectivityBatchTarget(
              {
                providerId: provider.id,
                providerName: provider.name,
                providerConfig: {
                  options: {
                    baseURL: settings?.config?.api?.baseUrl ?? '',
                    apiKey: settings?.config?.access?.apiKey ?? '',
                    headers: settings?.config?.api?.headers,
                  },
                },
                modelIds,
              },
              {
                requireBaseUrl: true,
                requireApiKey: true,
                errorMessages: {
                  missingBaseUrl: t('zcode.test.missingBaseUrl', { name: provider.name }),
                  missingApiKey: t('zcode.test.missingApiKey', { name: provider.name }),
                  missingModel: t('zcode.test.missingModel', { name: provider.name }),
                },
              },
            ),
          ],
          (providerId, status) => {
            setConnectivityStatuses((previous) => ({ ...previous, [providerId]: status }));
          },
        );
      } finally {
        setTestingModelsFor(null);
      }
    },
    [t],
  );

  const fetchModelsProvider = React.useMemo(
    () => providers.find((provider) => provider.id === fetchModelsProviderId) ?? null,
    [fetchModelsProviderId, providers],
  );

  const fetchModelsProviderInfo = React.useMemo(() => {
    if (!fetchModelsProvider) {
      return null;
    }
    const settings = parseZcodeProviderSettings(fetchModelsProvider.settingsConfig);
    return {
      providerId: fetchModelsProvider.id,
      name: fetchModelsProvider.name,
      baseUrl: settings?.config?.api?.baseUrl ?? '',
      apiKey: settings?.config?.access?.apiKey ?? '',
      existingModelIds: (settings?.models ?? []).map((model) => model.modelId),
    };
  }, [fetchModelsProvider]);

  /**
   * Merges the fetched models into the provider's catalog.
   *
   * Rows are written as `smart` rules and filled from the preset catalog, the
   * same defaults a manually added row gets — a fetched list otherwise lands as
   * bare ids and the user has to look every parameter up by hand. A model the
   * catalog does not know keeps the name the provider's API reported and leaves
   * ZCode's built-in rules to supply the rest.
   *
   * Ids already in the catalog are skipped, so re-fetching never overwrites
   * edits the user made by hand. Rows the modal reports as gone upstream are
   * dropped — the modal only fills that list when the user opts in.
   */
  const handleFetchModelsApply = React.useCallback(
    async (result: FetchModelsApplyResult) => {
      if (!fetchModelsProvider) {
        return;
      }
      const settings = parseZcodeProviderSettings(fetchModelsProvider.settingsConfig);
      if (!settings) {
        return;
      }
      const preferredNpm = preferredPresetNpmTypes(settings.config?.api?.type)[0];
      const removedIds = new Set(result.removedModelIds);
      const keptModels = settings.models.filter((model) => !removedIds.has(model.modelId));
      const existingIds = new Set(keptModels.map((model) => model.modelId));
      const added: ZcodeModelRow[] = result.selectedModels
        .filter((model) => !existingIds.has(model.id))
        .map((model) => {
          const row = buildZcodeModelRowFromPreset(
            model.id,
            'smart',
            findPresetModelById(model.id, preferredNpm),
          );
          // A preset name is more precise than the provider's label; the label
          // is still better than no name at all.
          return { ...row, displayName: row.displayName ?? (model.name || undefined) };
        });
      if (added.length === 0 && keptModels.length === settings.models.length) {
        setFetchModelsModalOpen(false);
        setFetchModelsProviderId(null);
        return;
      }
      await persistProviderModels(fetchModelsProvider, [...keptModels, ...added]);
      setFetchModelsModalOpen(false);
      setFetchModelsProviderId(null);
    },
    [fetchModelsProvider, persistProviderModels],
  );

  return (
    <SectionSidebarLayout
      sidebarTitle={t('zcode.title', { defaultValue: 'ZCode 配置管理' })}
      sidebarHidden={sidebarHidden}
      sections={sidebarSections}
      getIcon={(id) => {
        switch (id) {
          case 'zcode-providers':
            return <DatabaseOutlined />;
          case 'zcode-global-prompt':
            return <FileTextOutlined />;
          case 'zcode-session-manager':
            return <MessageOutlined />;
          default:
            return null;
        }
      }}
      onSectionSelect={(id) => {
        switch (id) {
          case 'zcode-providers':
            setProviderListCollapsed(false);
            break;
          case 'zcode-global-prompt':
            setPromptExpandNonce((value) => value + 1);
            break;
          case 'zcode-session-manager':
            setSessionManagerExpandNonce((value) => value + 1);
            break;
          default:
            break;
        }
      }}
    >
      <div>
        <CodingPageHeader
          title={t('zcode.title')}
          docsUrl="https://zcode.z.ai/cn/docs/configuration"
          configPath={configPath || '~/.zcode/v2/provider_config.json'}
          onPreviewConfig={() => void handlePreviewCurrentConfig()}
          onCustomizeConfig={() => setRootDirectoryModalOpen(true)}
          onOpenFolder={() => void handleOpenFolder()}
          onRefresh={() => void loadConfig()}
          onMoreOptions={() => setSettingsModalOpen(true)}
        />

        {!hasNewGenerationRegistry && (
          <Alert
            type="warning"
            showIcon
            style={{ marginBottom: 16 }}
            title={t('zcode.legacyGeneration.title', {
              defaultValue: 'ZCode 尚未迁移到新版供应商配置',
            })}
            description={t('zcode.legacyGeneration.description', {
              defaultValue:
                '当前 ZCode 仍读取旧版 config.json。请先启动一次 ZCode 桌面端完成迁移，否则这里的改动不会生效。',
            })}
          />
        )}

        <ProviderListSection
          sectionId="zcode-providers"
          collapsed={providerListCollapsed}
          onCollapsedChange={setProviderListCollapsed}
          loading={loading}
          providerCount={providers.length}
          visibleCount={visibleProviders.length}
          batch={providerBatch}
          batchSelectableIds={batchSelectableIds}
          keyword={providerKeyword}
          onKeywordChange={setProviderKeyword}
          sortMode={sortMode}
          sortModes={PROVIDER_SORT_MODES}
          onSortModeChange={setSortMode}
          onBatchTest={handleBatchTestProviders}
          batchTesting={batchTestingProviders}
          onOpenCommonConfig={() => setCommonConfigModalOpen(true)}
          onAddProvider={() => {
            setEditingProvider(null);
            setFormModalOpen(true);
          }}
          hint={
            <div
              style={{
                fontSize: 12,
                color: 'var(--color-text-secondary)',
                borderLeft: '2px solid var(--color-border)',
                paddingLeft: 8,
                marginBottom: 12,
              }}
            >
              <div>{t('zcode.pageHint')}</div>
              <div>{t('zcode.pageWarning')}</div>
            </div>
          }
          footer={
            <Space wrap>
              <Button
                type="dashed"
                icon={<ImportOutlined />}
                onClick={() => setImportModalOpen(true)}
              >
                {t('opencode.provider.importFavorite')}
              </Button>
              {allApiHubAvailable && (
                <Button
                  type="dashed"
                  icon={<AllApiHubIcon />}
                  onClick={() => setAllApiHubImportModalOpen(true)}
                >
                  {t('common.allApiHub.importFromAllApiHub')}
                </Button>
              )}
              {ccSwitchAvailable && (
                <Button
                  type="dashed"
                  icon={<ImportOutlined />}
                  onClick={() => setCcSwitchImportModalOpen(true)}
                >
                  {t('common.ccSwitch.importFromCcSwitch')}
                </Button>
              )}
            </Space>
          }
        >
          <DndContext
            sensors={providerBatchDragDisabled ? [] : undefined}
            collisionDetection={closestCenter}
            modifiers={[restrictToVerticalAxis]}
            onDragEnd={(event) => void handleDragEnd(event)}
          >
            <SortableContext
              items={providers.map((provider) => provider.id)}
              strategy={verticalListSortingStrategy}
            >
              <div>
                {visibleProviders.map((provider) => (
                  <ZcodeProviderCard
                    key={provider.id}
                    provider={provider}
                    onEdit={() => {
                      setEditingProvider(provider);
                      setFormModalOpen(true);
                    }}
                    onApply={() => {
                      noteProviderUsed(provider.id);
                      void handleApplyProvider(provider);
                    }}
                    onDelete={() => void handleDeleteProvider(provider)}
                    selectable={
                      providerBatch.selectionMode && providerBatch.isSelectable(provider.id)
                    }
                    selected={providerBatch.selectedIds.has(provider.id)}
                    onSelectChange={(selected) => providerBatch.toggleSelect(provider.id, selected)}
                    connectivityStatus={connectivityStatuses[provider.id]}
                    onAddModel={() => handleAddModel(provider)}
                    onEditModel={(modelId) => handleEditModel(provider, modelId)}
                    onCopyModel={(modelId) => handleCopyModel(provider, modelId)}
                    onDeleteModel={(modelId) => void handleDeleteModel(provider, modelId)}
                    onSetPrimaryModel={(modelId) => void handleSetPrimaryModel(provider, modelId)}
                    onReorderModels={(orderedModelIds) =>
                      void handleReorderModels(provider, orderedModelIds)
                    }
                    modelSelectionMode={modelBatchDeleteProviderId === provider.id}
                    selectedModelIds={selectedModelIdsByProvider[provider.id] ?? []}
                    onToggleModelSelection={(modelId, selected) =>
                      handleToggleModelSelection(provider, modelId, selected)
                    }
                    onToggleBatchDeleteMode={() => handleToggleModelBatchDeleteMode(provider)}
                    onBatchDeleteModels={() => handleBatchDeleteModels(provider)}
                    onTestModels={() => void handleTestProviderModels(provider)}
                    testModelsDisabled={
                      testingModelsFor === provider.id ||
                      !(parseZcodeProviderSettings(provider.settingsConfig)?.models ?? []).length
                    }
                    testModelsDisabledTooltip={t('common.modelMissing')}
                    onFetchModels={() => {
                      setFetchModelsProviderId(provider.id);
                      setFetchModelsModalOpen(true);
                    }}
                  />
                ))}
              </div>
            </SortableContext>
          </DndContext>
        </ProviderListSection>

        <div
          id="zcode-global-prompt"
          data-sidebar-section="true"
          data-sidebar-title={t('common.prompt.title')}
        >
          <GlobalPromptSettings
            key={`zcode-prompt-${promptExpandNonce}`}
            toolName="ZCode"
            promptFileName="AGENTS.md"
            service={zcodePromptApi}
            collapseKey="zcode-prompt"
            defaultExpanded={promptExpandNonce > 0}
            onUpdated={loadConfig}
          />
        </div>

        <div
          id="zcode-session-manager"
          data-sidebar-section="true"
          data-sidebar-title={t('sessionManager.title', { defaultValue: '会话管理' })}
        >
          <SessionManagerPanel
            tool="zcode"
            expandNonce={sessionManagerExpandNonce}
            refreshNonce={0}
          />
        </div>
      </div>

      {rootDirectoryModalOpen && (
        <RootDirectoryModal
          open={rootDirectoryModalOpen}
          {...getRootDirectoryModalProps(rootPathInfo)}
          onCancel={() => setRootDirectoryModalOpen(false)}
          onSubmit={handleSaveRootDirectory}
          onReset={handleResetRootDirectory}
        />
      )}

      {formModalOpen && (
        <ZcodeProviderFormModal
          open={formModalOpen}
          provider={editingProvider}
          onCancel={() => {
            setFormModalOpen(false);
            setEditingProvider(null);
          }}
          onSaved={async () => {
            setFormModalOpen(false);
            setEditingProvider(null);
            await refreshTrayMenu();
            await loadConfig();
          }}
        />
      )}

      {modelModal && (
        <ZcodeModelFormModal
          open
          isEdit={modelModal.modelIndex !== null}
          apiType={
            parseZcodeProviderSettings(modelModal.provider.settingsConfig)?.config?.api?.type
          }
          initialValues={
            modelModal.prefill ??
            (modelModal.modelIndex === null
              ? undefined
              : (parseZcodeProviderSettings(modelModal.provider.settingsConfig)?.models ?? [])[
                  modelModal.modelIndex
                ])
          }
          onCancel={() => setModelModal(null)}
          onSubmit={handleSubmitModel}
        />
      )}

      {commonConfigModalOpen && (
        <ZcodeCommonConfigModal
          open={commonConfigModalOpen}
          onCancel={() => setCommonConfigModalOpen(false)}
          onSuccess={() => {
            setCommonConfigModalOpen(false);
          }}
        />
      )}

      {fetchModelsProviderInfo && (
        <FetchModelsModal
          open={fetchModelsModalOpen}
          providerId={fetchModelsProviderInfo.providerId}
          providerName={fetchModelsProviderInfo.name}
          baseUrl={fetchModelsProviderInfo.baseUrl}
          apiKey={fetchModelsProviderInfo.apiKey || undefined}
          existingModelIds={fetchModelsProviderInfo.existingModelIds}
          onCancel={() => {
            setFetchModelsModalOpen(false);
            setFetchModelsProviderId(null);
          }}
          onSuccess={(result) => void handleFetchModelsApply(result)}
        />
      )}

      <JsonPreviewModal
        open={previewModalOpen}
        onClose={() => setPreviewModalOpen(false)}
        data={previewData}
      />

      <ImportProviderModal
        open={importModalOpen}
        onClose={() => setImportModalOpen(false)}
        onImport={(imported) => void handleImportFavoriteProviders(imported)}
        existingProviderIds={providers.map((provider) =>
          buildFavoriteProviderStorageKey('zcode', provider.id),
        )}
        providerFilter={(provider) => isFavoriteProviderForSource('zcode', provider)}
      />

      {allApiHubAvailable && (
        <ZcodeImportFromAllApiHubModal
          open={allApiHubImportModalOpen}
          existingProviderIds={providers.map((provider) => provider.sourceProviderId || provider.id)}
          onCancel={() => setAllApiHubImportModalOpen(false)}
          onImport={(imported) => void handleImportFromAllApiHub(imported)}
        />
      )}

      {ccSwitchAvailable && (
        <ImportFromCcSwitchModal
          open={ccSwitchImportModalOpen}
          // CC Switch only stores Claude-shaped providers; `claude` is the app
          // type those rows live under, and what other Anthropic-family CLIs
          // read here too.
          appType="claude"
          existingProviderIds={providers
            .map((provider) => provider.sourceProviderId)
            .filter((id): id is string => Boolean(id))}
          onClose={() => setCcSwitchImportModalOpen(false)}
          onImport={(imported) => void handleImportFromCcSwitch(imported)}
        />
      )}

      <SidebarSettingsModal
        open={settingsModalOpen}
        onClose={() => setSettingsModalOpen(false)}
        sidebarVisible={!sidebarHidden}
        onSidebarVisibleChange={(visible) => setSidebarHidden('zcode', !visible)}
      />
    </SectionSidebarLayout>
  );
};

export default ZcodePage;
