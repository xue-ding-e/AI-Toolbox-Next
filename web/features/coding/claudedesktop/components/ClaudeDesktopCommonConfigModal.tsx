import React from 'react';
import { Modal, Button, Alert, message } from 'antd';
import { useTranslation } from 'react-i18next';
import {
  getClaudeDesktopCommonConfig,
  saveClaudeDesktopCommonConfig,
} from '@/services/claudeDesktopApi';
import JsonEditor from '@/components/common/JsonEditor';
import { isJsonObject } from '@/utils/json';
import styles from '../../claudecode/components/CommonConfigModal.module.less';

interface ClaudeDesktopCommonConfigModalProps {
  open: boolean;
  onCancel: () => void;
  onSuccess: () => void;
}

/**
 * Claude Desktop common (base) config editor. Reuses the visual structure of
 * the Claude Code common-config modal (editorSection + quick option row) but is
 * scoped to the desktop base JSON (e.g. `mcpServers`), with no CLI root dir.
 */
const ClaudeDesktopCommonConfigModal: React.FC<ClaudeDesktopCommonConfigModalProps> = ({
  open,
  onCancel,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [configValue, setConfigValue] = React.useState<unknown>({});
  const [isValid, setIsValid] = React.useState(true);

  const loadConfig = React.useCallback(async () => {
    setLoading(true);
    try {
      const config = await getClaudeDesktopCommonConfig();
      if (config?.config) {
        try {
          const parsed = JSON.parse(config.config) as unknown;
          if (!isJsonObject(parsed)) {
            throw new Error('Expected JSON object');
          }
          setConfigValue(parsed);
          setIsValid(true);
        } catch {
          setConfigValue(config.config);
          setIsValid(false);
        }
      } else {
        setConfigValue({});
        setIsValid(true);
      }
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      message.error(errorMsg || t('common.error'));
    } finally {
      setLoading(false);
    }
  }, [t]);

  React.useEffect(() => {
    if (open) {
      void loadConfig();
    }
  }, [open, loadConfig]);

  const handleEditorChange = (value: unknown, valid: boolean) => {
    setConfigValue(value);
    setIsValid(valid);
  };

  const handleSave = async () => {
    if (!isValid) {
      message.error(t('claudecode.commonConfig.invalidJson'));
      return;
    }

    let configString: string;
    if (typeof configValue === 'string') {
      if (configValue.trim() === '') {
        configString = '{}';
      } else {
        try {
          JSON.parse(configValue);
          configString = configValue;
        } catch {
          message.error(t('claudecode.commonConfig.invalidJson'));
          return;
        }
      }
    } else {
      configString = JSON.stringify(configValue ?? {}, null, 2);
    }

    setLoading(true);
    try {
      await saveClaudeDesktopCommonConfig({ config: configString });
      message.success(t('common.success'));
      onSuccess();
      onCancel();
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      message.error(errorMsg || t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      title="Claude Desktop 通用配置"
      open={open}
      onCancel={onCancel}
      footer={[
        <Button key="cancel" onClick={onCancel} disabled={loading}>
          {t('common.cancel')}
        </Button>,
        <Button key="save" type="primary" onClick={handleSave} loading={loading}>
          {t('common.save')}
        </Button>,
      ]}
      width={800}
    >
      <div className={styles.content}>
        <div className={styles.editorSection}>
          <JsonEditor
            value={configValue}
            onChange={handleEditorChange}
            mode="text"
            height={400}
            minHeight={200}
            maxHeight={600}
            resizable
            placeholder={`{\n  "mcpServers": {}\n}`}
          />
        </div>
        <Alert
          message="此为基础配置文件（如 mcpServers 等），保存后会自动重新应用当前已应用的供应商配置。"
          type="info"
          showIcon
        />
      </div>
    </Modal>
  );
};

export default ClaudeDesktopCommonConfigModal;