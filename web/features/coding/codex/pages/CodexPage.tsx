import React from 'react';
import { Typography, Card, Button, Space, Empty, message, Modal, Spin } from 'antd';
import { PlusOutlined, FolderOpenOutlined, SettingOutlined, SyncOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import type { CodexProvider } from '@/types/codex';
import {
  getCodexConfigPath,
  listCodexProviders,
  selectCodexProvider,
  applyCodexConfig,
  revealCodexConfigFolder,
  readCodexSettings,
} from '@/services/codexApi';
import { usePreviewStore, useAppStore } from '@/stores';

const { Title, Text } = Typography;

const CodexPage: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { setPreviewData } = usePreviewStore();
  const appStoreState = useAppStore.getState();
  const [loading, setLoading] = React.useState(false);
  const [configPath, setConfigPath] = React.useState<string>('');
  const [providers, setProviders] = React.useState<CodexProvider[]>([]);
  const [appliedProviderId, setAppliedProviderId] = React.useState<string>('');
  const [providerModalOpen, setProviderModalOpen] = React.useState(false);

  const loadConfig = async () => {
    setLoading(true);
    try {
      const [path, providerList] = await Promise.all([
        getCodexConfigPath(),
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
    try {
      await revealCodexConfigFolder();
    } catch (error) {
      console.error('Failed to open folder:', error);
      message.error(t('common.error'));
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

  const handlePreviewCurrentConfig = async () => {
    try {
      const settings = await readCodexSettings();
      appStoreState.setCurrentModule('coding');
      appStoreState.setCurrentSubTab('codex');
      setPreviewData(t('codex.preview.currentConfigTitle'), settings, '/coding/codex');
      navigate('/preview/config');
    } catch (error) {
      console.error('Failed to preview config:', error);
      message.error(t('common.error'));
    }
  };

  return (
    <div>
      <div style={{ marginBottom: 16 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
          <div>
            <Title level={4} style={{ margin: 0 }}>{t('codex.title')}</Title>
            <Space size="small" style={{ marginTop: 8 }}>
              <Text type="secondary" style={{ fontSize: 12 }}>{t('codex.configPath')}:</Text>
              <Text code style={{ fontSize: 12 }}>{configPath || '~/.codex/'}</Text>
              <Button type="link" size="small" icon={<FolderOpenOutlined />} onClick={handleOpenFolder} style={{ padding: 0, fontSize: 12 }}>
                {t('codex.openFolder')}
              </Button>
            </Space>
          </div>
          <Space>
            <Button onClick={handlePreviewCurrentConfig}>{t('common.previewConfig')}</Button>
            <Button icon={<SettingOutlined />}>{t('codex.commonConfigButton')}</Button>
          </Space>
        </div>
      </div>

      <div style={{ marginBottom: 16 }}>
        <Space>
          <Button icon={<SyncOutlined />}>{t('codex.importFromOpenCode')}</Button>
          <Button type="primary" icon={<PlusOutlined />} onClick={() => setProviderModalOpen(true)}>{t('codex.addProvider')}</Button>
        </Space>
      </div>

      <Spin spinning={loading}>
        {providers.length === 0 ? (
          <Card><Empty description={t('codex.emptyText')} style={{ padding: '60px 0' }} /></Card>
        ) : (
          <div>
            {providers.map((provider) => (
              <Card key={provider.id} style={{ marginBottom: 8 }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <div>
                    <Text strong>{provider.name}</Text>
                    {provider.id === appliedProviderId && <span style={{ marginLeft: 8, color: '#52c41a' }}>✓ {t('codex.applied')}</span>}
                  </div>
                  <Space>
                    <Button size="small" onClick={() => handleSelectProvider(provider)} disabled={provider.id === appliedProviderId}>
                      {t('codex.apply')}
                    </Button>
                    <Button size="small">{t('common.edit')}</Button>
                    <Button size="small">{t('common.copy')}</Button>
                    <Button size="small" danger>{t('common.delete')}</Button>
                  </Space>
                </div>
              </Card>
            ))}
          </div>
        )}
      </Spin>

      {providerModalOpen && (
        <Modal open={providerModalOpen} title={t('codex.addProvider')} footer={null} onCancel={() => setProviderModalOpen(false)}>
          <p>TODO: CodexProviderFormModal</p>
        </Modal>
      )}
    </div>
  );
};

export default CodexPage;
