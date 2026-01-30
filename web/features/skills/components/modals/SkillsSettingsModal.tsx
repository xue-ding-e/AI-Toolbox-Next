import React from 'react';
import { Modal, InputNumber, Button, Checkbox, Tag, message } from 'antd';
import { FolderOpenOutlined, DeleteOutlined } from '@ant-design/icons';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { useTranslation } from 'react-i18next';
import type { ToolInfo } from '../../types';
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
  const [path, setPath] = React.useState('');
  const [cleanupDays, setCleanupDays] = React.useState(30);
  const [ttlSecs, setTtlSecs] = React.useState(60);
  const [loading, setLoading] = React.useState(false);
  const [clearingCache, setClearingCache] = React.useState(false);
  const [allTools, setAllTools] = React.useState<ToolInfo[]>([]);
  const [preferredTools, setPreferredTools] = React.useState<string[]>([]);

  // Load settings on open
  React.useEffect(() => {
    if (isOpen) {
      api.getCentralRepoPath().then(setPath).catch(console.error);
      api.getGitCacheCleanupDays().then(setCleanupDays).catch(console.error);
      api.getGitCacheTtlSecs().then(setTtlSecs).catch(console.error);

      // Load tools and preferred tools together
      Promise.all([api.getToolStatus(), api.getPreferredTools()])
        .then(([status, saved]) => {
          // Sort: installed tools first
          const sorted = [...status.tools].sort((a, b) => {
            if (a.installed === b.installed) return 0;
            return a.installed ? -1 : 1;
          });
          setAllTools(sorted);

          // null = never set before, default to all installed tools
          if (saved === null) {
            setPreferredTools(status.installed);
          } else {
            setPreferredTools(saved);
          }
        })
        .catch(console.error);
    }
  }, [isOpen]);

  const handleOpenFolder = async () => {
    if (path) {
      try {
        await revealItemInDir(path);
      } catch (error) {
        message.error(String(error));
      }
    }
  };

  const handleToolToggle = (toolKey: string, checked: boolean) => {
    setPreferredTools((prev) =>
      checked ? [...prev, toolKey] : prev.filter((k) => k !== toolKey)
    );
  };

  const handleSave = async () => {
    setLoading(true);
    try {
      await api.setGitCacheCleanupDays(cleanupDays);
      await api.setPreferredTools(preferredTools);
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

  const handleOpenCacheFolder = async () => {
    try {
      const cachePath = await api.getGitCachePath();
      await revealItemInDir(cachePath);
    } catch (error) {
      message.error(String(error));
    }
  };

  return (
    <Modal
      title={t('skills.settings')}
      open={isOpen}
      onCancel={onClose}
      footer={null}
      width={700}
      destroyOnClose
    >
      <div className={styles.section}>
        <div className={styles.labelArea}>
          <label className={styles.label}>{t('skills.skillsStoragePath')}</label>
        </div>
        <div className={styles.inputArea}>
          <div className={styles.pathRow}>
            <span className={styles.pathText}>{path}</span>
            <Button
              type="link"
              size="small"
              icon={<FolderOpenOutlined />}
              onClick={handleOpenFolder}
            />
          </div>
          <p className={styles.hint}>{t('skills.skillsStorageHint')}</p>
        </div>
      </div>

      <div className={styles.section}>
        <div className={styles.labelArea}>
          <label className={styles.label}>{t('skills.preferredTools')}</label>
        </div>
        <div className={styles.inputArea}>
          <div className={styles.toolList}>
            {allTools.map((tool) => (
              <div key={tool.key} className={styles.toolItem}>
                <Checkbox
                  checked={preferredTools.includes(tool.key)}
                  onChange={(e) => handleToolToggle(tool.key, e.target.checked)}
                >
                  {tool.label}
                </Checkbox>
                {!tool.installed && (
                  <Tag color="default">{t('skills.notInstalled')}</Tag>
                )}
              </div>
            ))}
          </div>
          <p className={styles.hint}>{t('skills.preferredToolsHint')}</p>
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
          <Button
            type="link"
            size="small"
            icon={<FolderOpenOutlined />}
            onClick={handleOpenCacheFolder}
          />
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
