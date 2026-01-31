import React from 'react';
import { Modal, List, Empty, Spin, message, Button, Popconfirm } from 'antd';
import { FileZipOutlined, DeleteOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { listWebDAVBackups, deleteWebDAVBackup, type BackupFileInfo } from '@/services';

interface WebDAVRestoreModalProps {
  open: boolean;
  onClose: () => void;
  onSelect: (filename: string) => void;
  url: string;
  username: string;
  password: string;
  remotePath: string;
}

const WebDAVRestoreModal: React.FC<WebDAVRestoreModalProps> = ({
  open,
  onClose,
  onSelect,
  url,
  username,
  password,
  remotePath,
}) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [backups, setBackups] = React.useState<BackupFileInfo[]>([]);

  React.useEffect(() => {
    if (open) {
      loadBackups();
    }
  }, [open]);

  const loadBackups = async () => {
    if (!url) {
      message.warning(t('settings.backupSettings.noWebDAVConfigured'));
      return;
    }

    setLoading(true);
    try {
      const files = await listWebDAVBackups(url, username, password, remotePath);
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

  const handleSelect = (filename: string) => {
    onSelect(filename);
    onClose();
  };

  const handleDelete = async (filename: string, e: React.MouseEvent) => {
    e.stopPropagation(); // 阻止触发选择
    try {
      await deleteWebDAVBackup(url, username, password, remotePath, filename);
      message.success(t('common.success'));
      // 刷新列表
      setBackups(backups.filter(b => b.filename !== filename));
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

  // Extract date from filename for display
  const formatBackupName = (filename: string) => {
    // ai-toolbox-backup-20260101-120000.zip -> 2026-01-01 12:00:00
    const match = filename.match(/ai-toolbox-backup-(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})\.zip/);
    if (match) {
      const [, year, month, day, hour, min, sec] = match;
      return `${year}-${month}-${day} ${hour}:${min}:${sec}`;
    }
    return filename;
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

  return (
    <Modal
      title={t('settings.backupSettings.selectBackupFile')}
      open={open}
      onCancel={onClose}
      footer={null}
      width={480}
    >
      {loading ? (
        <div style={{ display: 'flex', justifyContent: 'center', padding: 40 }}>
          <Spin />
        </div>
      ) : backups.length === 0 ? (
        <Empty description={t('settings.backupSettings.noBackupsFound')} />
      ) : (
        <List
          dataSource={backups}
          renderItem={(item) => (
            <List.Item
              style={{ cursor: 'pointer' }}
              onClick={() => handleSelect(item.filename)}
              actions={[
                <Popconfirm
                  key="delete"
                  title={t('common.confirm')}
                  description={t('settings.backupSettings.confirmDeleteBackup')}
                  onConfirm={(e) => handleDelete(item.filename, e as unknown as React.MouseEvent)}
                  onCancel={(e) => e?.stopPropagation()}
                  okText={t('common.confirm')}
                  cancelText={t('common.cancel')}
                >
                  <Button
                    type="text"
                    danger
                    icon={<DeleteOutlined />}
                    size="small"
                    onClick={(e) => e.stopPropagation()}
                  />
                </Popconfirm>
              ]}
            >
              <List.Item.Meta
                avatar={<FileZipOutlined style={{ fontSize: 24, color: '#1890ff' }} />}
                title={
                  <>
                    {formatBackupName(item.filename)}{' '}
                    <span style={{ fontSize: '12px', opacity: 0.65 }}>
                      ({formatFileSize(item.size)})
                    </span>
                  </>
                }
                description={item.filename}
              />
            </List.Item>
          )}
        />
      )}
    </Modal>
  );
};

export default WebDAVRestoreModal;
