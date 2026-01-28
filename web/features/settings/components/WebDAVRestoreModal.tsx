import React from 'react';
import { Modal, List, Empty, Spin, message, Button, Popconfirm } from 'antd';
import { FileZipOutlined, DeleteOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { listWebDAVBackups, deleteWebDAVBackup } from '@/services';

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
  const [backups, setBackups] = React.useState<string[]>([]);

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
      setBackups(backups.filter(b => b !== filename));
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
              onClick={() => handleSelect(item)}
              actions={[
                <Popconfirm
                  key="delete"
                  title={t('common.confirm')}
                  description={t('settings.backupSettings.confirmDeleteBackup')}
                  onConfirm={(e) => handleDelete(item, e as unknown as React.MouseEvent)}
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
                title={formatBackupName(item)}
                description={item}
              />
            </List.Item>
          )}
        />
      )}
    </Modal>
  );
};

export default WebDAVRestoreModal;
