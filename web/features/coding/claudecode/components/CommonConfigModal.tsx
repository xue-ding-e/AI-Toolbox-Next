import React from 'react';
import { Modal, Alert, Button, message } from 'antd';
import { useTranslation } from 'react-i18next';
import {
  extractClaudeCommonConfigFromCurrentFile,
  getClaudeCommonConfig,
  saveClaudeCommonConfig,
  saveClaudeLocalConfig,
} from '@/services/claudeCodeApi';
import JsonEditor from '@/components/common/JsonEditor';

interface CommonConfigModalProps {
  open: boolean;
  onCancel: () => void;
  onSuccess: () => void;
  isLocalProvider?: boolean;
}

const CommonConfigModal: React.FC<CommonConfigModalProps> = ({
  open,
  onCancel,
  onSuccess,
  isLocalProvider = false,
}) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [configValue, setConfigValue] = React.useState<unknown>({});
  const [rootDir, setRootDir] = React.useState<string | null>(null);

  // Use ref for validation state to avoid re-renders during editing
  const isValidRef = React.useRef(true);

  // 加载现有配置
  React.useEffect(() => {
    if (open) {
      loadConfig();
    }
  }, [open]);

  const loadConfig = async () => {
    setLoading(true);
    try {
      const config = await getClaudeCommonConfig();
      if (config?.config) {
        try {
          const configObj = JSON.parse(config.config);
          setConfigValue(configObj);
          setRootDir(config.rootDir ?? null);
          isValidRef.current = true;
        } catch (error) {
          console.error('Failed to parse config JSON:', error);
          setConfigValue(config.config);
          setRootDir(config.rootDir ?? null);
          isValidRef.current = false;
        }
      } else {
        // 空配置时设置为空字符串，让 JSON 编辑器显示 placeholder
        setConfigValue("");
        setRootDir(null);
        isValidRef.current = true;
      }
    } catch (error) {
      console.error('Failed to load common config:', error);
      const errorMsg = error instanceof Error ? error.message : String(error);
      message.error(errorMsg || t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const handleExtractFromCurrentConfig = async () => {
    setLoading(true);
    try {
      const extractedConfig = await extractClaudeCommonConfigFromCurrentFile();
      const extractedValue = extractedConfig.config ? JSON.parse(extractedConfig.config) : "";
      setConfigValue(extractedValue);
      setRootDir(extractedConfig.rootDir ?? null);
      isValidRef.current = true;
      message.success(t('claudecode.commonConfig.extractSuccess'));
    } catch (error) {
      console.error('Failed to extract common config from current file:', error);
      const errorMsg = error instanceof Error ? error.message : String(error);
      message.error(errorMsg || t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    if (!isValidRef.current) {
      message.error(t('claudecode.commonConfig.invalidJson'));
      return;
    }

    setLoading(true);
    try {
      const configString = JSON.stringify(configValue, null, 2);
      if (isLocalProvider) {
        await saveClaudeLocalConfig({ commonConfig: configString, rootDir });
      } else {
        await saveClaudeCommonConfig({ config: configString, rootDir });
      }
      message.success(t('common.success'));
      onSuccess();
      onCancel();
    } catch (error) {
      console.error('Failed to save common config:', error);
      const errorMsg = error instanceof Error ? error.message : String(error);
      message.error(errorMsg || t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const handleEditorChange = (value: unknown, valid: boolean) => {
    setConfigValue(value);
    isValidRef.current = valid;
  };

  return (
    <Modal
      title={t('claudecode.commonConfig.title')}
      open={open}
      onCancel={onCancel}
      onOk={handleSave}
      confirmLoading={loading}
      width={800}
      okText={t('common.save')}
      cancelText={t('common.cancel')}
      footer={[
        <Button
          key="extract"
          onClick={handleExtractFromCurrentConfig}
          loading={loading}
        >
          {t('claudecode.commonConfig.extractFromCurrent')}
        </Button>,
        <Button key="cancel" onClick={onCancel} disabled={loading}>
          {t('common.cancel')}
        </Button>,
        <Button key="save" type="primary" onClick={handleSave} loading={loading}>
          {t('common.save')}
        </Button>,
      ]}
    >
      {isLocalProvider && (
        <Alert
          message={t('claudecode.localConfigHint')}
          type="warning"
          showIcon
          style={{ marginBottom: 16 }}
        />
      )}
      <JsonEditor
        value={configValue}
        onChange={handleEditorChange}
        mode="text"
        height={400}
        minHeight={200}
        maxHeight={600}
        resizable
        placeholder={`{
    "skipWebFetchPreflight": true
}`}
      />

      <div style={{ marginTop: 12 }}>
        <Alert
          message={t('claudecode.commonConfig.combinedHint')}
          type="info"
          showIcon
        />
      </div>
    </Modal>
  );
};

export default CommonConfigModal;
