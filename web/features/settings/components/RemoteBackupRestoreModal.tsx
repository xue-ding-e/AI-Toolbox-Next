import React from 'react';
import { Modal, List, Empty, Spin, message, Button, Popconfirm, Tabs, Tag, Typography } from 'antd';
import { FileZipOutlined, DeleteOutlined, LockOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { BackupFileInfo } from '@/services';
import { describeBackupFilename } from '../utils/backupFilename';
import ScrollFadeHint from './ScrollFadeHint';

const { Text } = Typography;

type BackupMatchType = 'current' | 'other' | 'unlabeled';

export interface ParsedBackupFile extends BackupFileInfo {
  displayTime: string;
  hostLabel: string | null;
  matchType: BackupMatchType;
}

export interface RemoteBackupSelection {
  file: ParsedBackupFile;
}

interface RemoteBackupRestoreModalProps {
  open: boolean;
  onClose: () => void;
  /** Load the current remote backup files (sorted by the shared filename contract). */
  loadFiles: () => Promise<BackupFileInfo[]>;
  /** Delete one remote backup file. */
  deleteFile: (file: ParsedBackupFile) => Promise<void>;
  /** Select a file for restore; the parent runs the restore + password retry flow. */
  onSelect: (selection: RemoteBackupSelection) => void;
  currentHostLabel: string;
}

/**
 * Shared backup file list for WebDAV and repository channels: time, host label,
 * filename, size, encryption badge, restore, and delete. Consumes only the unified
 * `BackupFileInfo` entries — no channel-specific filename rules live here.
 */
const RemoteBackupRestoreModal: React.FC<RemoteBackupRestoreModalProps> = ({
  open,
  onClose,
  loadFiles,
  deleteFile,
  onSelect,
  currentHostLabel,
}) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [backups, setBackups] = React.useState<BackupFileInfo[]>([]);

  const normalizedCurrentHostLabel = currentHostLabel.trim();
  const showHostTabs = normalizedCurrentHostLabel.length > 0;
  const [activeTabKey, setActiveTabKey] = React.useState<'all' | 'current' | 'other'>(
    showHostTabs ? 'current' : 'all',
  );

  // Bottom scroll-fade hint: the backup list can exceed the modal body height. A
  // callback ref binds the real scroll container once the portal has mounted.
  const listBodyRef = React.useRef<HTMLDivElement | null>(null);
  const [scrollContainer, setScrollContainer] = React.useState<HTMLElement | null>(null);
  const attachListBody = React.useCallback((node: HTMLDivElement | null) => {
    listBodyRef.current = node;
    setScrollContainer(node?.closest<HTMLElement>('.ant-modal-body') ?? null);
  }, []);
  React.useEffect(() => {
    if (!open) {
      setScrollContainer(null);
    }
  }, [open]);

  React.useEffect(() => {
    if (open) {
      loadBackups();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  React.useEffect(() => {
    if (!open) {
      return;
    }
    setActiveTabKey(showHostTabs ? 'current' : 'all');
  }, [open, showHostTabs]);

  const loadBackups = async () => {
    setLoading(true);
    try {
      const files = await loadFiles();
      setBackups(files);
    } catch (error) {
      console.error('Failed to list backups:', error);

      // Parse error if it's JSON
      let errorMessage = t('settings.backupSettings.listBackupsFailed');
      try {
        const errorObj = JSON.parse(String(error));
        if (errorObj.suggestion) {
          errorMessage = `${t('settings.backupSettings.listBackupsFailed')}: ${t(errorObj.suggestion)}`;
        }
      } catch {
        errorMessage = `${t('settings.backupSettings.listBackupsFailed')}: ${String(error)}`;
      }

      message.error(errorMessage);
    } finally {
      setLoading(false);
    }
  };

  const handleSelect = (file: ParsedBackupFile) => {
    onSelect({ file });
    onClose();
  };

  const handleDelete = async (file: ParsedBackupFile, e: React.MouseEvent) => {
    e.stopPropagation(); // 阻止触发选择
    try {
      await deleteFile(file);
      message.success(t('common.success'));
      // 刷新列表
      setBackups((currentBackups) =>
        currentBackups.filter((backup) => backup.filename !== file.filename),
      );
    } catch (error) {
      console.error('Failed to delete backup:', error);

      let errorMessage = t('common.error');
      try {
        const errorObj = JSON.parse(String(error));
        if (errorObj.suggestion) {
          errorMessage = t(errorObj.suggestion);
        }
      } catch {
        errorMessage = String(error);
      }

      message.error(errorMessage);
    }
  };

  // Format file size to KB/MB/GB with 1 decimal place
  const formatFileSize = (bytes: number): string => {
    if (bytes === 0) return '0 B';

    const units = ['B', 'KB', 'MB', 'GB'];
    let size = bytes;
    let unitIndex = 0;

    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex++;
    }

    // For bytes, don't show decimal
    if (unitIndex === 0) {
      return `${size} ${units[unitIndex]}`;
    }

    // For KB/MB/GB, show 1 decimal place
    return `${size.toFixed(1)} ${units[unitIndex]}`;
  };

  const parsedBackups = React.useMemo<ParsedBackupFile[]>(() => {
    return backups.map((backup) => {
      const described = describeBackupFilename(backup.filename);
      const normalizedBackupHostLabel = described.hostLabel?.trim() || null;
      const encrypted = backup.encrypted || described.encrypted;

      let matchType: BackupMatchType = 'unlabeled';
      if (normalizedBackupHostLabel) {
        matchType =
          normalizedBackupHostLabel === normalizedCurrentHostLabel ? 'current' : 'other';
      }

      return {
        ...backup,
        encrypted,
        displayTime: described.displayTime,
        hostLabel: normalizedBackupHostLabel,
        matchType,
      };
    });
  }, [backups, normalizedCurrentHostLabel]);

  const currentHostBackups = React.useMemo(
    () => parsedBackups.filter((backup) => backup.matchType === 'current'),
    [parsedBackups],
  );

  const otherHostBackups = React.useMemo(
    () => parsedBackups.filter((backup) => backup.matchType !== 'current'),
    [parsedBackups],
  );

  const renderList = (dataSource: ParsedBackupFile[], emptyDescription: string) => {
    if (dataSource.length === 0) {
      return <Empty description={emptyDescription} style={{ padding: '24px 0' }} />;
    }

    return (
      <List
        dataSource={dataSource}
        renderItem={(item) => (
          <List.Item
            style={{ cursor: 'pointer' }}
            onClick={() => handleSelect(item)}
            actions={[
              <Popconfirm
                key="delete"
                title={t('common.confirm')}
                description={t('settings.backupSettings.confirmDeleteBackup')}
                onConfirm={(event) => handleDelete(item, event as unknown as React.MouseEvent)}
                onCancel={(event) => event?.stopPropagation()}
                okText={t('common.confirm')}
                cancelText={t('common.cancel')}
              >
                <Button
                  type="text"
                  danger
                  icon={<DeleteOutlined />}
                  size="small"
                  onClick={(event) => event.stopPropagation()}
                />
              </Popconfirm>,
            ]}
          >
            <List.Item.Meta
              avatar={<FileZipOutlined style={{ fontSize: 24 }} />}
              title={
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12 }}>
                  <Text strong>{item.displayTime}</Text>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap', justifyContent: 'flex-end' }}>
                    {item.encrypted && (
                      <Tag icon={<LockOutlined />}>{t('settings.backupSettings.encryptedBadge')}</Tag>
                    )}
                    {item.hostLabel && (
                      <Tag>{item.hostLabel}</Tag>
                    )}
                    <Text type="secondary">{formatFileSize(item.size)}</Text>
                  </div>
                </div>
              }
              description={
                <Text type="secondary">{item.filename}</Text>
              }
            />
          </List.Item>
        )}
      />
    );
  };

  return (
    <Modal
      title={t('settings.backupSettings.selectBackupFile')}
      open={open}
      onCancel={onClose}
      footer={null}
      width={500}
      afterOpenChange={(opened) => {
        if (opened && listBodyRef.current) {
          setScrollContainer(
            listBodyRef.current.closest<HTMLElement>('.ant-modal-body') ?? null,
          );
        }
      }}
    >
      <div ref={attachListBody}>
        {!showHostTabs && (
          <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 12 }}>
            {t('settings.backupSettings.restoreOverwriteNotice')}
          </Text>
        )}
        {loading ? (
          <div style={{ display: 'flex', justifyContent: 'center', padding: 40 }}>
            <Spin />
          </div>
        ) : parsedBackups.length === 0 ? (
          <Empty description={t('settings.backupSettings.noBackupsFound')} />
        ) : showHostTabs ? (
          <Tabs
            activeKey={activeTabKey}
            onChange={(key) => setActiveTabKey(key as 'current' | 'other' | 'all')}
            items={[
              {
                key: 'current',
                label: t('settings.backupSettings.currentHostBackups'),
                children: renderList(
                  currentHostBackups,
                  t('settings.backupSettings.currentHostEmpty', {
                    hostLabel: normalizedCurrentHostLabel,
                  }),
                ),
              },
              {
                key: 'other',
                label: t('settings.backupSettings.otherHostBackups'),
                children: (
                  <>
                    <Text type="warning" style={{ fontSize: 12, display: 'block', marginBottom: 8 }}>
                      {t('settings.backupSettings.otherHostRestoreHint')}
                    </Text>
                    {renderList(
                      otherHostBackups,
                      t('settings.backupSettings.otherHostEmpty'),
                    )}
                  </>
                ),
              },
            ]}
          />
        ) : (
          renderList(parsedBackups, t('settings.backupSettings.noBackupsFound'))
        )}
        <ScrollFadeHint scrollContainer={scrollContainer} />
      </div>
    </Modal>
  );
};

export default RemoteBackupRestoreModal;
