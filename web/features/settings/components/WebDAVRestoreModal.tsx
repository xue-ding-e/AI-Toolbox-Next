import React from 'react';
import { Modal, List, Empty, Spin, message, Button, Popconfirm, Tabs, Tag, Typography } from 'antd';
import { FileZipOutlined, DeleteOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { listWebDAVBackups, deleteWebDAVBackup, type BackupFileInfo } from '@/services';

const { Text } = Typography;

type BackupMatchType = 'current' | 'other' | 'unlabeled';

interface ParsedBackupFile extends BackupFileInfo {
  displayTime: string;
  hostLabel: string | null;
  matchType: BackupMatchType;
}

interface WebDAVRestoreModalProps {
  open: boolean;
  onClose: () => void;
  onSelect: (selection: {
    filename: string;
    hostLabel: string | null;
    matchType: BackupMatchType;
  }) => void;
  url: string;
  username: string;
  password: string;
  remotePath: string;
  currentHostLabel: string;
}

const WebDAVRestoreModal: React.FC<WebDAVRestoreModalProps> = ({
  open,
  onClose,
  onSelect,
  url,
  username,
  password,
  remotePath,
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

  React.useEffect(() => {
    if (open) {
      loadBackups();
    }
  }, [open]);

  React.useEffect(() => {
    if (!open) {
      return;
    }
    setActiveTabKey(showHostTabs ? 'current' : 'all');
  }, [open, showHostTabs]);

  const loadBackups = async () => {
    if (!url) {
      message.warning(t('settings.backupSettings.noWebDAVConfigured'));
      return;
    }

    setLoading(true);
    try {
      const files = await listWebDAVBackups(url, username, password, remotePath);
      files.sort((a, b) => b.filename.localeCompare(a.filename));
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

  const handleSelect = (selection: ParsedBackupFile) => {
    onSelect({
      filename: selection.filename,
      hostLabel: selection.hostLabel,
      matchType: showHostTabs ? selection.matchType : 'unlabeled',
    });
    onClose();
  };

  const handleDelete = async (filename: string, e: React.MouseEvent) => {
    e.stopPropagation(); // 阻止触发选择
    try {
      await deleteWebDAVBackup(url, username, password, remotePath, filename);
      message.success(t('common.success'));
      // 刷新列表
      setBackups((currentBackups) =>
        currentBackups.filter((backup) => backup.filename !== filename),
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

  const parseBackupFilename = (filename: string) => {
    const currentFormatMatch = filename.match(
      /^ai-toolbox-backup-(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})(?:_(.+))?\.zip$/,
    );
    if (currentFormatMatch) {
      const [, year, month, day, hour, minute, second, hostLabel] = currentFormatMatch;
      return {
        displayTime: `${year}-${month}-${day} ${hour}:${minute}:${second}`,
        hostLabel: hostLabel?.trim() || null,
      };
    }

    const legacyFormatMatch = filename.match(
      /^ai-toolbox-backup-(?:.+)-(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})\.zip$/,
    );
    if (legacyFormatMatch) {
      const [, year, month, day, hour, minute, second] = legacyFormatMatch;
      return {
        displayTime: `${year}-${month}-${day} ${hour}:${minute}:${second}`,
        hostLabel: null,
      };
    }

    return {
      displayTime: filename,
      hostLabel: null,
    };
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
      const parsed = parseBackupFilename(backup.filename);
      const normalizedBackupHostLabel = parsed.hostLabel?.trim() || null;

      let matchType: BackupMatchType = 'unlabeled';
      if (normalizedBackupHostLabel) {
        matchType =
          normalizedBackupHostLabel === normalizedCurrentHostLabel ? 'current' : 'other';
      }

      return {
        ...backup,
        displayTime: parsed.displayTime,
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
                onConfirm={(event) => handleDelete(item.filename, event as unknown as React.MouseEvent)}
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
    >
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
    </Modal>
  );
};

export default WebDAVRestoreModal;
