import React from 'react';
import { Typography, Card, Button, Space, Empty, message, Modal, Spin } from 'antd';
import { PlusOutlined, FolderOpenOutlined, SettingOutlined, SyncOutlined, EyeOutlined, ExclamationCircleOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { useNavigate, useLocation } from 'react-router-dom';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { invoke } from '@tauri-apps/api/core';
import type { CodexProvider, CodexProviderFormValues, CodexSettingsConfig, ImportConflictInfo, ImportConflictAction } from '@/types/codex';
import {
  getCodexConfigFilePath,
  listCodexProviders,
  selectCodexProvider,
  applyCodexConfig,
  readCodexSettings,
  createCodexProvider,
  updateCodexProvider,
  deleteCodexProvider,
  getCodexCommonConfig,
} from '@/services/codexApi';
import { usePreviewStore, useAppStore } from '@/stores';
import CodexProviderCard from '../components/CodexProviderCard';
import CodexProviderFormModal from '../components/CodexProviderFormModal';
import CodexCommonConfigModal from '../components/CodexCommonConfigModal';
import ImportConflictDialog from '../components/ImportConflictDialog';

const { Title, Text, Link } = Typography;

const CodexPage: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const { setPreviewData } = usePreviewStore();
  const appStoreState = useAppStore.getState();
  const [loading, setLoading] = React.useState(false);
  const [configPath, setConfigPath] = React.useState<string>('');
  const [providers, setProviders] = React.useState<CodexProvider[]>([]);
  const [appliedProviderId, setAppliedProviderId] = React.useState<string>('');

  // Modal states
  const [providerModalOpen, setProviderModalOpen] = React.useState(false);
  const [editingProvider, setEditingProvider] = React.useState<CodexProvider | null>(null);
  const [isCopyMode, setIsCopyMode] = React.useState(false);
  const [modalDefaultTab, setModalDefaultTab] = React.useState<'manual' | 'import'>('manual');
  const [commonConfigModalOpen, setCommonConfigModalOpen] = React.useState(false);
  const [conflictDialogOpen, setConflictDialogOpen] = React.useState(false);
  const [conflictInfo, setConflictInfo] = React.useState<ImportConflictInfo | null>(null);
  const [pendingFormValues, setPendingFormValues] = React.useState<CodexProviderFormValues | null>(null);

  const loadConfig = async () => {
    setLoading(true);
    try {
      const [path, providerList] = await Promise.all([
        getCodexConfigFilePath(),
        listCodexProviders(),
      ]);
      setConfigPath(path);
      setProviders(providerList);
      const applied = providerList.find((p) => p.isApplied);
      setAppliedProviderId(applied?.id || '');
    } catch (error) {
      console.error('Failed to load config:', error);
      message.error(t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  React.useEffect(() => {
    loadConfig();
  }, []);

  const handleOpenFolder = async () => {
    if (!configPath) return;

    try {
      await revealItemInDir(configPath);
    } catch {
      try {
        const parentDir = configPath.replace(/[\\/][^\\/]+$/, '');
        await invoke('open_folder', { path: parentDir });
      } catch (error) {
        console.error('Failed to open folder:', error);
        message.error(t('common.error'));
      }
    }
  };

  const handleSelectProvider = async (provider: CodexProvider) => {
    try {
      await selectCodexProvider(provider.id);
      await applyCodexConfig(provider.id);
      message.success(t('codex.apply.success'));
      await loadConfig();
    } catch (error) {
      console.error('Failed to select provider:', error);
      message.error(t('common.error'));
    }
  };

  const handleAddProvider = () => {
    setEditingProvider(null);
    setIsCopyMode(false);
    setModalDefaultTab('manual');
    setProviderModalOpen(true);
  };

  const handleImportFromOpenCode = () => {
    setEditingProvider(null);
    setIsCopyMode(false);
    setModalDefaultTab('import');
    setProviderModalOpen(true);
  };

  const handleEditProvider = (provider: CodexProvider) => {
    setEditingProvider(provider);
    setIsCopyMode(false);
    setModalDefaultTab('manual');
    setProviderModalOpen(true);
  };

  const handleCopyProvider = (provider: CodexProvider) => {
    setEditingProvider({
      ...provider,
      id: `${provider.id}_copy`,
      name: `${provider.name}_copy`,
      isApplied: false,
    });
    setIsCopyMode(true);
    setModalDefaultTab('manual');
    setProviderModalOpen(true);
  };

  const handleDeleteProvider = (provider: CodexProvider) => {
    Modal.confirm({
      title: t('codex.provider.confirmDelete', { name: provider.name }),
      icon: <ExclamationCircleOutlined />,
      onOk: async () => {
        try {
          await deleteCodexProvider(provider.id);
          message.success(t('common.success'));
          await loadConfig();
        } catch (error) {
          console.error('Failed to delete provider:', error);
          message.error(t('common.error'));
        }
      },
    });
  };

  const handleProviderSubmit = async (values: CodexProviderFormValues) => {
    // Check for conflicts
    if (values.sourceProviderId && !editingProvider) {
      const existingProvider = providers.find(
        (p) => p.sourceProviderId === values.sourceProviderId
      );

      if (existingProvider) {
        setConflictInfo({
          existingProvider,
          newProviderName: values.name,
          sourceProviderId: values.sourceProviderId,
        });
        setPendingFormValues(values);
        setConflictDialogOpen(true);
        return;
      }
    }

    await doSaveProvider(values);
  };

  const handleConflictResolve = async (action: ImportConflictAction) => {
    if (!pendingFormValues || !conflictInfo) return;

    if (action === 'cancel') {
      setConflictDialogOpen(false);
      setConflictInfo(null);
      setPendingFormValues(null);
      return;
    }

    if (action === 'overwrite') {
      await doUpdateProvider(conflictInfo.existingProvider.id, pendingFormValues);
    } else {
      await doSaveProvider({
        ...pendingFormValues,
        sourceProviderId: undefined,
      });
    }

    setConflictDialogOpen(false);
    setConflictInfo(null);
    setPendingFormValues(null);
  };

  const doSaveProvider = async (values: CodexProviderFormValues) => {
    try {
      const generateId = (name: string): string => {
        const timestamp = Date.now().toString(36);
        const random = Math.random().toString(36).substring(2, 8);
        const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, '-');
        return `${slug}-${timestamp}-${random}`;
      };

      // Build settings config
      const settingsConfigObj: CodexSettingsConfig = {
        auth: {
          OPENAI_API_KEY: values.apiKey || '',
        },
      };

      // Build config.toml with base_url and model
      let configParts: string[] = [];

      if (values.baseUrl) {
        configParts.push(`base_url = "${values.baseUrl}"`);
      }

      if (values.model) {
        configParts.push(`[chat]
model = "${values.model}"`);
      }

      if (configParts.length > 0) {
        settingsConfigObj.config = configParts.join('\n');
      }

      if (values.configToml) {
        settingsConfigObj.config = (settingsConfigObj.config || '') + '\n' + values.configToml;
      }

      if (editingProvider && !isCopyMode) {
        await updateCodexProvider({
          id: editingProvider.id,
          name: values.name,
          category: values.category,
          settingsConfig: JSON.stringify(settingsConfigObj),
          sourceProviderId: values.sourceProviderId,
          notes: values.notes,
          isApplied: editingProvider.isApplied,
          createdAt: editingProvider.createdAt,
          updatedAt: editingProvider.updatedAt,
        });
      } else {
        await createCodexProvider({
          id: generateId(values.name),
          name: values.name,
          category: values.category,
          settingsConfig: JSON.stringify(settingsConfigObj),
          sourceProviderId: values.sourceProviderId,
          notes: values.notes,
          isApplied: false,
        });
      }

      message.success(t('common.success'));
      setProviderModalOpen(false);
      setIsCopyMode(false);
      await loadConfig();
    } catch (error) {
      console.error('Failed to save provider:', error);
      message.error(t('common.error'));
      throw error;
    }
  };

  const doUpdateProvider = async (id: string, values: CodexProviderFormValues) => {
    try {
      const existingProvider = providers.find((p) => p.id === id);
      if (!existingProvider) return;

      const settingsConfigObj: CodexSettingsConfig = {
        auth: {
          OPENAI_API_KEY: values.apiKey || '',
        },
      };

      // Build config.toml with base_url and model
      let configParts: string[] = [];

      if (values.baseUrl) {
        configParts.push(`base_url = "${values.baseUrl}"`);
      }

      if (values.model) {
        configParts.push(`[chat]
model = "${values.model}"`);
      }

      if (configParts.length > 0) {
        settingsConfigObj.config = configParts.join('\n');
      }

      if (values.configToml) {
        settingsConfigObj.config = (settingsConfigObj.config || '') + '\n' + values.configToml;
      }

      const providerData: CodexProvider = {
        ...existingProvider,
        name: values.name,
        category: values.category,
        settingsConfig: JSON.stringify(settingsConfigObj),
        notes: values.notes,
        createdAt: existingProvider.createdAt,
        updatedAt: existingProvider.updatedAt,
      };

      await updateCodexProvider(providerData);
      message.success(t('common.success'));
      setProviderModalOpen(false);
      await loadConfig();
    } catch (error) {
      console.error('Failed to update provider:', error);
      message.error(t('common.error'));
      throw error;
    }
  };

  const handlePreviewCurrentConfig = async () => {
    try {
      const settings = await readCodexSettings();
      appStoreState.setCurrentModule('coding');
      appStoreState.setCurrentSubTab('codex');
      setPreviewData(t('codex.preview.currentConfigTitle'), settings, location.pathname);
      navigate('/preview/config');
    } catch (error) {
      console.error('Failed to preview config:', error);
      message.error(t('common.error'));
    }
  };

  const handlePreviewProvider = async (provider: CodexProvider) => {
    try {
      if (provider.isApplied) {
        const settings = await readCodexSettings();
        appStoreState.setCurrentModule('coding');
        appStoreState.setCurrentSubTab('codex');
        setPreviewData(t('codex.preview.providerConfigTitle', { name: provider.name }), settings, location.pathname);
        navigate('/preview/config');
      } else {
        const commonConfig = await getCodexCommonConfig();
        let commonConfigToml = '';
        if (commonConfig?.config) {
          commonConfigToml = commonConfig.config;
        }

        const providerConfig: CodexSettingsConfig = JSON.parse(provider.settingsConfig);

        appStoreState.setCurrentModule('coding');
        appStoreState.setCurrentSubTab('codex');
        setPreviewData(
          t('codex.preview.providerConfigTitle', { name: provider.name }),
          { commonConfig: commonConfigToml, providerConfig },
          location.pathname
        );
        navigate('/preview/config');
      }
    } catch (error) {
      console.error('Failed to preview provider config:', error);
      message.error(t('common.error'));
    }
  };

  return (
    <div>
      {/* Page Header */}
      <div style={{ marginBottom: 16 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
          <div>
            <div style={{ marginBottom: 8 }}>
              <Title level={4} style={{ margin: 0, display: 'inline-block', marginRight: 8 }}>
                {t('codex.title')}
              </Title>
              <Link
                type="secondary"
                style={{ fontSize: 12 }}
                onClick={(e) => {
                  e.stopPropagation();
                  handlePreviewCurrentConfig();
                }}
              >
                <EyeOutlined /> {t('common.previewConfig')}
              </Link>
            </div>
            <Space size="small">
              <Text type="secondary" style={{ fontSize: 12 }}>
                {t('codex.configPath')}:
              </Text>
              <Text code style={{ fontSize: 12 }}>
                {configPath || '~/.codex/config.toml'}
              </Text>
              <Button
                type="link"
                size="small"
                icon={<FolderOpenOutlined />}
                onClick={handleOpenFolder}
                style={{ padding: 0, fontSize: 12 }}
              >
                {t('codex.openFolder')}
              </Button>
            </Space>
          </div>
          <Space>
            <Button icon={<SettingOutlined />} onClick={() => setCommonConfigModalOpen(true)}>
              {t('codex.commonConfigButton')}
            </Button>
          </Space>
        </div>
      </div>

      {/* Action Bar */}
      <div style={{ marginBottom: 16 }}>
        <Space>
          <Button icon={<SyncOutlined />} onClick={handleImportFromOpenCode}>
            {t('codex.importFromOpenCode')}
          </Button>
          <Button type="primary" icon={<PlusOutlined />} onClick={handleAddProvider}>
            {t('codex.addProvider')}
          </Button>
        </Space>
      </div>

      {/* Provider List */}
      <Spin spinning={loading}>
        {providers.length === 0 ? (
          <Card>
            <Empty description={t('codex.emptyText')} style={{ padding: '60px 0' }} />
          </Card>
        ) : (
          <div>
            {providers.map((provider) => (
              <CodexProviderCard
                key={provider.id}
                provider={provider}
                isApplied={provider.id === appliedProviderId}
                onEdit={handleEditProvider}
                onDelete={handleDeleteProvider}
                onCopy={handleCopyProvider}
                onSelect={handleSelectProvider}
                onPreview={handlePreviewProvider}
              />
            ))}
          </div>
        )}
      </Spin>

      {/* Modals */}
      <CodexProviderFormModal
        open={providerModalOpen}
        provider={editingProvider}
        isCopy={isCopyMode}
        defaultTab={modalDefaultTab}
        onCancel={() => {
          setProviderModalOpen(false);
          setEditingProvider(null);
          setIsCopyMode(false);
        }}
        onSubmit={handleProviderSubmit}
      />

      <CodexCommonConfigModal
        open={commonConfigModalOpen}
        onCancel={() => setCommonConfigModalOpen(false)}
        onSuccess={() => {
          setCommonConfigModalOpen(false);
        }}
      />

      <ImportConflictDialog
        open={conflictDialogOpen}
        conflictInfo={conflictInfo}
        onResolve={handleConflictResolve}
        onCancel={() => {
          setConflictDialogOpen(false);
          setConflictInfo(null);
          setPendingFormValues(null);
        }}
      />
    </div>
  );
};

export default CodexPage;
