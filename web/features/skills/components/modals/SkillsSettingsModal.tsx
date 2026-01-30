import React from 'react';
import { Modal, Input, InputNumber, Button, Space, message } from 'antd';
import { FolderOpenOutlined, DeleteOutlined } from '@ant-design/icons';
import { open } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';
import { useSkillsStore } from '../../stores/skillsStore';
import * as api from '../../services/skillsApi';
import styles from './SkillsSettingsModal.module.less';

interface SkillsSettingsModalProps {
  open: boolean;
  onClose: () => void;
}

export const SkillsSettingsModal: React.FC<SkillsSettingsModalProps> = ({
  open: isOpen,
  onClose,
}) => {
  const { t } = useTranslation();
  const { loadCentralRepoPath } = useSkillsStore();
  const [path, setPath] = React.useState('');
  const [originalPath, setOriginalPath] = React.useState('');
  const [cleanupDays, setCleanupDays] = React.useState(30);
  const [ttlSecs, setTtlSecs] = React.useState(60);
  const [loading, setLoading] = React.useState(false);
  const [clearingCache, setClearingCache] = React.useState(false);

  // Load settings on open
  React.useEffect(() => {
    if (isOpen) {
      // Always fetch fresh path from API
      api.getCentralRepoPath().then((p) => {
        setPath(p);
        setOriginalPath(p);
      }).catch(console.error);
      api.getGitCacheCleanupDays().then(setCleanupDays).catch(console.error);
      api.getGitCacheTtlSecs().then(setTtlSecs).catch(console.error);
    }
  }, [isOpen]);

  const handleBrowse = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: t('skills.selectStoragePath'),
    });
    if (selected && typeof selected === 'string') {
      setPath(selected);
    }
  };

  const handleSave = async () => {
    setLoading(true);
    try {
      if (path && path !== originalPath) {
        await api.setCentralRepoPath(path);
        await loadCentralRepoPath();
      }
      await api.setGitCacheCleanupDays(cleanupDays);
      // Note: ttlSecs setting would need a backend command if we want to save it
      message.success(t('common.success'));
      onClose();
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoading(false);
    }
  };

  const handleClearCache = async () => {
    setClearingCache(true);
    try {
      const count = await api.clearGitCache();
      message.success(t('skills.status.gitCacheCleared', { count }));
    } catch (error) {
      message.error(String(error));
    } finally {
      setClearingCache(false);
    }
  };

  return (
    <Modal
      title={t('skills.settings')}
      open={isOpen}
      onCancel={onClose}
      footer={null}
      width={640}
      destroyOnClose
    >
      <div className={styles.section}>
        <div className={styles.labelArea}>
          <label className={styles.label}>{t('skills.skillsStoragePath')}</label>
        </div>
        <div className={styles.inputArea}>
          <Space.Compact style={{ width: '100%' }}>
            <Input
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder={t('skills.storagePath')}
            />
            <Button icon={<FolderOpenOutlined />} onClick={handleBrowse}>
              {t('common.browse')}
            </Button>
          </Space.Compact>
          <p className={styles.hint}>{t('skills.skillsStorageHint')}</p>
        </div>
      </div>

      <div className={styles.section}>
        <div className={styles.labelArea}>
          <label className={styles.label}>{t('skills.gitCacheCleanupDays')}</label>
        </div>
        <div className={styles.inputArea}>
          <InputNumber
            min={0}
            max={365}
            value={cleanupDays}
            onChange={(v) => setCleanupDays(v || 0)}
            style={{ width: 120 }}
          />
          <p className={styles.hint}>{t('skills.gitCacheCleanupHint')}</p>
        </div>
      </div>

      <div className={styles.section}>
        <div className={styles.labelArea}>
          <label className={styles.label}>{t('skills.gitCacheTtlSecs')}</label>
        </div>
        <div className={styles.inputArea}>
          <InputNumber
            min={0}
            max={3600}
            value={ttlSecs}
            onChange={(v) => setTtlSecs(v || 0)}
            style={{ width: 120 }}
          />
          <p className={styles.hint}>{t('skills.gitCacheTtlHint')}</p>
        </div>
      </div>

      <div className={styles.section}>
        <div className={styles.labelArea}>
          <label className={styles.label}>{t('skills.maintenance')}</label>
        </div>
        <div className={styles.inputArea}>
          <Button
            icon={<DeleteOutlined />}
            onClick={handleClearCache}
            loading={clearingCache}
          >
            {t('skills.cleanNow')}
          </Button>
        </div>
      </div>

      <div className={styles.footer}>
        <Button onClick={onClose}>{t('common.cancel')}</Button>
        <Button type="primary" onClick={handleSave} loading={loading}>
          {t('common.save')}
        </Button>
      </div>
    </Modal>
  );
};
