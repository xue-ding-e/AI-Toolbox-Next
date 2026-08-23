import React from 'react';
import { Button, Space, message, Modal } from 'antd';
import { CheckOutlined, DownloadOutlined, FileSearchOutlined, FolderOpenOutlined, RobotOutlined } from '@ant-design/icons';
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';
import * as api from '../../services/mcpApi';
import type { McpGroupInventoryPreview } from '../../types';
import styles from './McpInventoryModal.module.less';

interface McpInventoryModalProps {
  open: boolean;
  onClose: () => void;
  onSuccess: () => void;
}

// Group inventory modal mirrors SkillInventoryModal: export the full
// group->servers mapping, let the user (or an AI assistant) curate the JSON,
// then preview and apply it back. Grouping only — configs never change.
export const McpInventoryModal: React.FC<McpInventoryModalProps> = ({ open, onClose, onSuccess }) => {
  const { t } = useTranslation();
  const [exportPath, setExportPath] = React.useState('');
  const [importPath, setImportPath] = React.useState('');
  const [preview, setPreview] = React.useState<McpGroupInventoryPreview | null>(null);
  const [loading, setLoading] = React.useState(false);

  React.useEffect(() => {
    if (!open) {
      setExportPath('');
      setImportPath('');
      setPreview(null);
    }
  }, [open]);

  const handleExportFile = async () => {
    const selectedPath = await saveDialog({
      title: t('mcp.inventory.exportDialogTitle'),
      defaultPath: 'mcp-group-inventory.json',
      filters: [{ name: 'JSON', extensions: ['json'] }],
    });
    if (typeof selectedPath !== 'string' || !selectedPath) {
      return null;
    }

    setLoading(true);
    try {
      await api.exportMcpGroupInventory(selectedPath);
      setExportPath(selectedPath);
      message.success(t('mcp.inventory.exportSuccess', { path: selectedPath }));
      return selectedPath;
    } catch (error) {
      message.error(String(error));
      return null;
    } finally {
      setLoading(false);
    }
  };

  const handleCopyPrompt = async () => {
    setLoading(true);
    try {
      let currentExportPath = exportPath;
      if (!currentExportPath.trim()) {
        const exported = await handleExportFile();
        if (!exported) {
          return;
        }
        currentExportPath = exported;
      }
      const prompt = t('mcp.inventory.agentPromptText', { path: currentExportPath });
      await navigator.clipboard.writeText(prompt);
      message.success(t('mcp.inventory.copyPromptSuccess'));
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoading(false);
    }
  };

  const handleSelectImportFile = async () => {
    try {
      const selected = await openDialog({
        title: t('mcp.inventory.importDialogTitle'),
        multiple: false,
        directory: false,
        filters: [
          {
            name: 'JSON',
            extensions: ['json'],
          },
        ],
      });
      if (typeof selected !== 'string') {
        return;
      }
      setImportPath(selected);
      setPreview(null);
    } catch (error) {
      message.error(String(error));
    }
  };

  const handlePreview = async () => {
    if (!importPath.trim()) return;
    setLoading(true);
    try {
      const result = await api.previewMcpGroupInventoryImport(importPath);
      setPreview(result);
      if (!result.valid) {
        message.error(t('mcp.inventory.previewInvalid'));
      }
    } catch (error) {
      message.error(String(error));
    } finally {
      setLoading(false);
    }
  };

  const handleApply = () => {
    if (!preview?.valid || !importPath.trim()) return;
    Modal.confirm({
      title: t('mcp.inventory.applyTitle'),
      content: t('mcp.inventory.applyContent', { count: preview.changed_count }),
      okText: t('mcp.inventory.apply'),
      cancelText: t('common.cancel'),
      onOk: async () => {
        setLoading(true);
        try {
          const result = await api.applyMcpGroupInventoryImport(importPath);
          if (!result.valid) {
            setPreview(result);
            return;
          }
          message.success(t('mcp.inventory.applySuccess'));
          onSuccess();
          onClose();
        } catch (error) {
          message.error(String(error));
        } finally {
          setLoading(false);
        }
      },
    });
  };

  const footer = (
    <div className={styles.footer}>
      <Space>
        <Button icon={<DownloadOutlined />} onClick={handleExportFile} loading={loading}>
          {t('mcp.inventory.exportFile')}
        </Button>
        <Button icon={<RobotOutlined />} onClick={handleCopyPrompt} loading={loading}>
          {t('mcp.inventory.copyAgentPrompt')}
        </Button>
      </Space>
      <Space>
        <Button onClick={onClose}>{t('common.cancel')}</Button>
        {!preview ? (
          <Button
            type="primary"
            icon={<FileSearchOutlined />}
            onClick={handlePreview}
            loading={loading}
            disabled={!importPath.trim()}
          >
            {t('mcp.inventory.preview')}
          </Button>
        ) : (
          <Button
            type="primary"
            icon={<CheckOutlined />}
            onClick={handleApply}
            loading={loading}
            disabled={!preview.valid}
          >
            {t('mcp.inventory.apply')}
          </Button>
        )}
      </Space>
    </div>
  );

  return (
    <Modal
      open={open}
      title={t('mcp.inventory.title')}
      onCancel={onClose}
      width={780}
      footer={footer}
      destroyOnHidden
      className={styles.modal}
    >
      <div className={styles.content}>
        <section className={styles.sectionCard}>
          <div className={styles.sectionHeader}>
            <div>
              <strong>{t('mcp.inventory.exportTitle')}</strong>
              <p>{t('mcp.inventory.exportHint')}</p>
            </div>
          </div>
          <div className={styles.pathRow}>
            <span>{t('mcp.inventory.exportPath')}</span>
            <code>{exportPath || t('mcp.inventory.defaultExportPath')}</code>
          </div>
        </section>
        <section className={styles.sectionCard}>
          <div className={styles.sectionHeader}>
            <div>
              <strong>{t('mcp.inventory.importTitle')}</strong>
              <p>{t('mcp.inventory.importHint')}</p>
            </div>
            <Button icon={<FolderOpenOutlined />} onClick={handleSelectImportFile} disabled={loading}>
              {t('mcp.inventory.selectFile')}
            </Button>
          </div>
          <div className={styles.pathRow}>
            <span>{t('mcp.inventory.importPath')}</span>
            <code>{importPath || t('mcp.inventory.noImportFile')}</code>
          </div>
        </section>
        {preview && (
          <section className={styles.previewCard}>
            <div className={styles.previewRow}>
              {t('mcp.inventory.previewGroups', { count: preview.group_count })}
            </div>
            <div className={styles.previewRow}>
              {t('mcp.inventory.previewMatched', { count: preview.matched_server_count })}
            </div>
            <div className={styles.previewRow}>
              {t('mcp.inventory.previewChanged', { count: preview.changed_count })}
            </div>
            {preview.errors.length > 0 && (
              <div className={styles.previewErrors}>
                {preview.errors.map((err, i) => <div key={i}>{err}</div>)}
              </div>
            )}
          </section>
        )}
      </div>
    </Modal>
  );
};

export default McpInventoryModal;
