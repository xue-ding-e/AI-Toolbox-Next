import React from 'react';
import { Alert, Button, Descriptions, Modal, Segmented, Space, Spin, Tag, Typography, message } from 'antd';
import { DatabaseOutlined, FolderOpenOutlined, ReloadOutlined, SyncOutlined, UndoOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import type { CodexHistorySourceMode, CodexHistorySyncStatus } from '@/types/codex';
import {
  backupCodexHistory,
  getCodexHistorySyncStatus,
  restoreLatestCodexHistoryBackup,
  syncCodexHistory,
} from '@/services/codexApi';

const { Text, Paragraph } = Typography;

interface CodexHistorySyncModalProps {
  open: boolean;
  sourceMode: CodexHistorySourceMode;
  onCancel: () => void;
  onChanged?: () => void;
}

const resolveInitialHistorySourceMode = (sourceMode: CodexHistorySourceMode): CodexHistorySourceMode => (
  sourceMode === 'wsl' ? 'wsl' : 'all'
);

const CodexHistorySyncModal: React.FC<CodexHistorySyncModalProps> = ({
  open,
  sourceMode,
  onCancel,
  onChanged,
}) => {
  const { t } = useTranslation();
  const [status, setStatus] = React.useState<CodexHistorySyncStatus | null>(null);
  const [loading, setLoading] = React.useState(false);
  const [actionLoading, setActionLoading] = React.useState<null | 'sync' | 'backup' | 'restore' | 'open'>(null);
  const [selectedSourceMode, setSelectedSourceMode] = React.useState<CodexHistorySourceMode>(() => resolveInitialHistorySourceMode(sourceMode));

  const loadStatus = React.useCallback(async (
    showSuccess = false,
    sourceModeToLoad: CodexHistorySourceMode,
  ) => {
    try {
      setLoading(true);
      const result = await getCodexHistorySyncStatus(sourceModeToLoad);
      setStatus(result);
      if (showSuccess) {
        message.success(t('codex.historySync.statusSuccess'));
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      message.error(errorMessage || t('common.error'));
    } finally {
      setLoading(false);
    }
  }, [t]);

  React.useEffect(() => {
    if (open) {
      const nextSourceMode = resolveInitialHistorySourceMode(sourceMode);
      setSelectedSourceMode(nextSourceMode);
      setStatus(null);
      void loadStatus(false, nextSourceMode);
    }
  }, [loadStatus, open, sourceMode]);

  const sourceLabel = React.useCallback((sourceStatus: CodexHistorySyncStatus | null) => {
    if (sourceStatus?.runtimeSource === 'wsl') {
      return sourceStatus.runtimeDistro
        ? t('codex.historySync.sourceWslWithDistro', { distro: sourceStatus.runtimeDistro })
        : t('codex.historySync.sourceWsl');
    }
    return t('codex.historySync.sourceLocal');
  }, [t]);

  const availableSources = status?.availableSources ?? [];
  const wslSource = availableSources.find((item) => item.source === 'wsl');
  const showSourceSwitcher = availableSources.some((item) => item.source === 'local') && Boolean(wslSource);
  const activeSourceValue = status?.runtimeSource === 'wsl' ? 'wsl' : 'local';
  const sourceOptions = React.useMemo(() => [
    { label: t('codex.historySync.sourceLocal'), value: 'local' },
    {
      label: wslSource?.distro
        ? t('codex.historySync.sourceWslWithDistro', { distro: wslSource.distro })
        : t('codex.historySync.sourceWsl'),
      value: 'wsl',
    },
  ], [t, wslSource?.distro]);

  const handleSourceChange = React.useCallback((value: string | number) => {
    const nextSourceMode = value as CodexHistorySourceMode;
    setSelectedSourceMode(nextSourceMode);
    setStatus(null);
    void loadStatus(false, nextSourceMode);
  }, [loadStatus]);

  const openBackupDir = async () => {
    if (!status) {
      return;
    }
    try {
      setActionLoading('open');
      try {
        await revealItemInDir(status.backupDir);
      } catch {
        await invoke('open_folder', { path: status.backupDir });
      }
      message.success(t('codex.historySync.openBackupDirSuccess'));
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      message.error(errorMessage || t('common.error'));
    } finally {
      setActionLoading(null);
    }
  };

  const createBackup = async () => {
    try {
      setActionLoading('backup');
      const result = await backupCodexHistory(selectedSourceMode);
      message.success(t('codex.historySync.backupSuccess', { path: result.backupPath }));
      await loadStatus(false, selectedSourceMode);
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      message.error(errorMessage || t('common.error'));
    } finally {
      setActionLoading(null);
    }
  };

  const performSync = async () => {
    if (!status) {
      return;
    }
    if (!status.hasWork) {
      message.info(t('codex.historySync.noWork'));
      return;
    }
    Modal.confirm({
      title: t('codex.historySync.syncConfirmTitle'),
      content: t('codex.historySync.syncConfirmContent', {
        provider: status.currentProvider,
        source: sourceLabel(status),
      }),
      okText: t('codex.historySync.sync'),
      cancelText: t('common.cancel'),
      onOk: async () => {
        try {
          setActionLoading('sync');
          const result = await syncCodexHistory(selectedSourceMode);
          if (result.partialSuccess) {
            message.warning(t('codex.historySync.syncPartialSuccess', {
              threads: result.updatedThreadRows,
              files: result.updatedSessionFiles,
              failed: result.failedSessionFiles,
              error: result.firstSessionFileError || '',
            }));
          } else {
            message.success(t('codex.historySync.syncSuccess', {
              threads: result.updatedThreadRows,
              files: result.updatedSessionFiles,
              index: result.rewrittenIndexEntries,
            }));
          }
          message.info(t('codex.historySync.duration', {
            ms: result.durationMs,
            wait: result.lockWaitMs,
            attempts: result.attempts,
          }));
          setStatus(result.status);
          onChanged?.();
        } catch (error) {
          const errorMessage = error instanceof Error ? error.message : String(error);
          message.error(errorMessage || t('common.error'));
        } finally {
          setActionLoading(null);
        }
      },
    });
  };

  const restoreLatest = async () => {
    if (!status?.latestBackupPath) {
      message.info(t('codex.historySync.noBackup'));
      return;
    }
    Modal.confirm({
      title: t('codex.historySync.restoreConfirmTitle'),
      content: t('codex.historySync.restoreConfirmContent'),
      okText: t('codex.historySync.restoreLatest'),
      okButtonProps: { danger: true },
      cancelText: t('common.cancel'),
      onOk: async () => {
        try {
          setActionLoading('restore');
          const result = await restoreLatestCodexHistoryBackup(selectedSourceMode);
          message.success(t('codex.historySync.restoreSuccess', { path: result.safetyBackupPath }));
          message.info(t('codex.historySync.duration', {
            ms: result.durationMs,
            wait: result.lockWaitMs,
            attempts: result.attempts,
          }));
          setStatus(result.status);
          onChanged?.();
        } catch (error) {
          const errorMessage = error instanceof Error ? error.message : String(error);
          message.error(errorMessage || t('common.error'));
        } finally {
          setActionLoading(null);
        }
      },
    });
  };

  const renderStatus = () => {
    if (!status) {
      return null;
    }
    return (
      <div style={{ width: '100%', display: 'flex', flexDirection: 'column', gap: 16 }}>
        <Alert type="info" showIcon description={t('codex.historySync.description')} />
        {showSourceSwitcher ? (
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12 }}>
            <Text type="secondary">{t('codex.historySync.source')}</Text>
            <Segmented
              size="small"
              value={activeSourceValue}
              options={sourceOptions}
              disabled={loading || Boolean(actionLoading)}
              onChange={handleSourceChange}
            />
          </div>
        ) : null}
        <Descriptions size="small" bordered column={1} title={t('codex.historySync.currentRuntime')}>
          <Descriptions.Item label={t('codex.historySync.source')}>
            <Tag color={status.runtimeSource === 'wsl' ? 'blue' : 'default'}>{sourceLabel(status)}</Tag>
          </Descriptions.Item>
          <Descriptions.Item label={t('codex.historySync.codexHome')}>
            <Text code copyable>{status.codexHome}</Text>
          </Descriptions.Item>
          <Descriptions.Item label={t('codex.historySync.currentProvider')}>
            <Text code>{status.currentProvider}</Text>
          </Descriptions.Item>
          <Descriptions.Item label={t('codex.historySync.currentModel')}>
            <Text code>{status.currentModel || t('codex.historySync.notDetected')}</Text>
          </Descriptions.Item>
        </Descriptions>
        <Descriptions size="small" bordered column={2} title={t('codex.historySync.historyState')}>
          <Descriptions.Item label={t('codex.historySync.totalThreads')}>{status.totalThreads}</Descriptions.Item>
          <Descriptions.Item label={t('codex.historySync.providerMismatchThreads')}>{status.providerMismatchThreads}</Descriptions.Item>
          <Descriptions.Item label={t('codex.historySync.sessionFileCount')}>{status.sessionFileCount}</Descriptions.Item>
          <Descriptions.Item label={t('codex.historySync.sessionMetaMismatchCount')}>{status.sessionMetaMismatchCount}</Descriptions.Item>
          <Descriptions.Item label={t('codex.historySync.indexedThreads')}>{status.indexedThreads}</Descriptions.Item>
          <Descriptions.Item label={t('codex.historySync.missingSessionIndexEntries')}>{status.missingSessionIndexEntries}</Descriptions.Item>
          <Descriptions.Item label={t('codex.historySync.backupCount')}>{status.backupCount}</Descriptions.Item>
        </Descriptions>
        {status.latestBackupPath ? (
          <Paragraph style={{ marginBottom: 0 }}>
            <Text type="secondary">{t('codex.historySync.latestBackupPath')}: </Text>
            <Text code copyable>{status.latestBackupPath}</Text>
          </Paragraph>
        ) : null}
      </div>
    );
  };

  return (
    <Modal
      title={<Space><DatabaseOutlined />{t('codex.historySync.title')}</Space>}
      open={open}
      onCancel={onCancel}
      width={760}
      footer={[
        <Button key="close" onClick={onCancel}>{t('common.close')}</Button>,
        <Button key="refresh" icon={<ReloadOutlined />} loading={loading} onClick={() => void loadStatus(true, selectedSourceMode)}>
          {t('codex.historySync.refresh')}
        </Button>,
        <Button key="backup" icon={<DatabaseOutlined />} loading={actionLoading === 'backup'} onClick={() => void createBackup()}>
          {t('codex.historySync.backup')}
        </Button>,
        <Button key="open" icon={<FolderOpenOutlined />} loading={actionLoading === 'open'} onClick={() => void openBackupDir()} disabled={!status}>
          {t('codex.historySync.openBackupDir')}
        </Button>,
        <Button key="restore" danger icon={<UndoOutlined />} loading={actionLoading === 'restore'} onClick={() => void restoreLatest()} disabled={!status?.latestBackupPath}>
          {t('codex.historySync.restoreLatest')}
        </Button>,
        <Button key="sync" type="primary" icon={<SyncOutlined />} loading={actionLoading === 'sync'} onClick={() => void performSync()} disabled={!status?.hasWork}>
          {t('codex.historySync.sync')}
        </Button>,
      ]}
    >
      <Spin spinning={loading && !status}>{renderStatus()}</Spin>
    </Modal>
  );
};

export default CodexHistorySyncModal;
