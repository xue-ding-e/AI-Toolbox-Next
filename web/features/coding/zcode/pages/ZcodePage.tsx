import React from 'react';
import { Alert, Button, Collapse, Empty, Space, Spin, Tag, Typography, message } from 'antd';
import {
  DatabaseOutlined,
  EditOutlined,
  EllipsisOutlined,
  FileTextOutlined,
  FolderOpenOutlined,
  LinkOutlined,
  MessageOutlined,
  PlusOutlined,
  SyncOutlined,
} from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener';
import SectionSidebarLayout, {
  type SidebarSectionMarker,
} from '@/components/layout/SectionSidebarLayout/SectionSidebarLayout';
import SidebarSettingsModal from '@/components/common/SidebarSettingsModal';
import { useKeepAlive } from '@/components/layout/KeepAliveOutlet';
import RootDirectoryModal from '@/features/coding/shared/RootDirectoryModal';
import useRootDirectoryConfig from '@/features/coding/shared/useRootDirectoryConfig';
import { GlobalPromptSettings } from '@/features/coding/shared/prompt';
import { SessionManagerPanel } from '@/features/coding/shared/sessionManager';
import { useSettingsStore } from '@/stores';
import { refreshTrayMenu } from '@/services/appApi';
import {
  deleteZcodeProvider,
  getZcodeCommonConfig,
  getZcodeConfigFilePath,
  getZcodeGenerationStatus,
  getZcodeRootPathInfo,
  listZcodeProviders,
  revealZcodeConfigFolder,
  saveZcodeCommonConfig,
  selectZcodeProvider,
} from '@/services/zcodeApi';
import { zcodePromptApi } from '@/services/zcodePromptApi';
import type { ConfigPathInfo, ZcodeProvider } from '@/types/zcode';
import ZcodeProviderCard from '../components/ZcodeProviderCard';
import ZcodeProviderFormModal from '../components/ZcodeProviderFormModal';
import { resolveZcodeDefaultModelId } from '../utils/zcodeSettingsConfig';

const { Title, Text, Link } = Typography;

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

  const sidebarHidden = sidebarHiddenByPage.zcode ?? false;

  const sidebarSections = React.useMemo<SidebarSectionMarker[]>(
    () => [
      {
        id: 'zcode-providers',
        title: t('zcode.provider.title', { defaultValue: '供应商' }),
        order: 1,
      },
      {
        id: 'zcode-global-prompt',
        title: t('zcode.prompt.title', { defaultValue: '全局提示词' }),
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
      void message.error(t('zcode.loadFailed', { defaultValue: '加载 ZCode 配置失败' }));
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
        <div style={{ marginBottom: 16 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
            <div>
              <div style={{ marginBottom: 8 }}>
                <Title level={4} style={{ margin: 0, display: 'inline-block', marginRight: 8 }}>
                  {t('zcode.title', { defaultValue: 'ZCode 配置管理' })}
                </Title>
                <Link
                  type="secondary"
                  style={{ fontSize: 12 }}
                  onClick={(event) => {
                    event.stopPropagation();
                    openUrl('https://zcode.z.ai/cn/docs/configuration');
                  }}
                >
                  <LinkOutlined /> {t('zcode.viewDocs', { defaultValue: '查看文档' })}
                </Link>
              </div>
              <Space size="small">
                <Text type="secondary" style={{ fontSize: 12 }}>
                  {t('zcode.configPath', { defaultValue: '配置文件' })}:
                </Text>
                <Text code style={{ fontSize: 12 }}>
                  {configPath || '~/.zcode/v2/provider_config.json'}
                </Text>
                <Button
                  type="text"
                  size="small"
                  icon={<EditOutlined />}
                  onClick={() => setRootDirectoryModalOpen(true)}
                  style={{ padding: 0, fontSize: 12 }}
                >
                  {t('zcode.rootPathSource.customize', { defaultValue: '自定义根目录' })}
                </Button>
                <Button
                  type="text"
                  size="small"
                  icon={<FolderOpenOutlined />}
                  onClick={() => void handleOpenFolder()}
                  style={{ padding: 0, fontSize: 12 }}
                >
                  {t('zcode.openFolder', { defaultValue: '打开文件夹' })}
                </Button>
                <Button
                  type="text"
                  size="small"
                  icon={<SyncOutlined />}
                  onClick={() => void loadConfig()}
                  style={{ padding: 0, fontSize: 12 }}
                >
                  {t('zcode.refreshConfig', { defaultValue: '刷新' })}
                </Button>
              </Space>
            </div>
            <Space>
              <Button
                type="text"
                icon={<EllipsisOutlined />}
                onClick={() => setSettingsModalOpen(true)}
              >
                {t('common.moreOptions', { defaultValue: '更多选项' })}
              </Button>
            </Space>
          </div>
        </div>

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

        <div
          id="zcode-providers"
          data-sidebar-section="true"
          data-sidebar-title={t('zcode.provider.title', { defaultValue: '供应商' })}
        >
          <Collapse
            style={{ marginBottom: 16 }}
            activeKey={providerListCollapsed ? [] : ['providers']}
            onChange={(keys) => setProviderListCollapsed(keys.length === 0)}
            items={[
              {
                key: 'providers',
                label: (
                  <Space size="small">
                    <DatabaseOutlined />
                    <span>{t('zcode.provider.title', { defaultValue: '供应商' })}</span>
                    {providers.length > 0 && <Tag>{providers.length}</Tag>}
                  </Space>
                ),
                extra: (
                  <Button
                    type="primary"
                    size="small"
                    icon={<PlusOutlined />}
                    onClick={(event) => {
                      event.stopPropagation();
                      setEditingProvider(null);
                      setFormModalOpen(true);
                    }}
                  >
                    {t('zcode.addProvider', { defaultValue: '添加供应商' })}
                  </Button>
                ),
                children: (
                  <Spin spinning={loading}>
                    {providers.length === 0 ? (
                      <Empty description={t('zcode.emptyText', { defaultValue: '暂无供应商' })}>
                        <Button
                          type="primary"
                          icon={<PlusOutlined />}
                          onClick={() => {
                            setEditingProvider(null);
                            setFormModalOpen(true);
                          }}
                        >
                          {t('zcode.addProvider', { defaultValue: '添加供应商' })}
                        </Button>
                      </Empty>
                    ) : (
                      <Space orientation="vertical" style={{ width: '100%' }} size="middle">
                        {providers.map((provider) => (
                          <ZcodeProviderCard
                            key={provider.id}
                            provider={provider}
                            onEdit={() => {
                              setEditingProvider(provider);
                              setFormModalOpen(true);
                            }}
                            onApply={() => void handleApplyProvider(provider)}
                            onDelete={() => void handleDeleteProvider(provider)}
                          />
                        ))}
                      </Space>
                    )}
                  </Spin>
                ),
              },
            ]}
          />
        </div>

        <div
          id="zcode-global-prompt"
          data-sidebar-section="true"
          data-sidebar-title={t('zcode.prompt.title', { defaultValue: '全局提示词' })}
        >
          <GlobalPromptSettings
            key={`zcode-prompt-${promptExpandNonce}`}
            translationKeyPrefix="zcode.prompt"
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
