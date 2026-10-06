import React from 'react';
import { Alert, message } from 'antd';
import { DatabaseOutlined, FileTextOutlined, MessageOutlined } from '@ant-design/icons';
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
import type { ProviderConnectivityStatusItem } from '@/components/common/ProviderCard/types';
import { useSettingsStore } from '@/stores';
import { refreshTrayMenu } from '@/services/appApi';
import {
  deleteZcodeProvider,
  getZcodeCommonConfig,
  getZcodeConfigFilePath,
  getZcodeGenerationStatus,
  getZcodeRootPathInfo,
  listZcodeProviders,
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
  } | null>(null);

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

  const handleDeleteProvider = async (provider: ZcodeProvider) => {
    try {
      await deleteZcodeProvider(provider.id);
      await refreshTrayMenu();
      await loadConfig();
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
        // ZCode has no favorites store to back up into, so the backup step is
        // a no-op — the confirmation dialog is the only guard.
        await backupProvidersBeforeDelete(
          providersToDelete,
          async () => undefined,
          (provider) => t('common.batch.backupFailed', { name: provider.name }),
        );
        for (const provider of providersToDelete) {
          await deleteZcodeProvider(provider.id);
        }
        await refreshTrayMenu();
        await loadConfig();
        void message.success(t('common.success'));
        return true;
      } catch (error) {
        console.error('Failed to batch delete ZCode providers:', error);
        void message.error(error instanceof Error ? error.message : String(error));
        return false;
      }
    },
    [providers, loadConfig, t],
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
                    onDeleteModel={(modelId) => void handleDeleteModel(provider, modelId)}
                    onSetPrimaryModel={(modelId) => void handleSetPrimaryModel(provider, modelId)}
                    onReorderModels={(orderedModelIds) =>
                      void handleReorderModels(provider, orderedModelIds)
                    }
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
          initialValues={
            modelModal.modelIndex === null
              ? undefined
              : (parseZcodeProviderSettings(modelModal.provider.settingsConfig)?.models ?? [])[
                  modelModal.modelIndex
                ]
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
