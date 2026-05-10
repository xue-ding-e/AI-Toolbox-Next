import React from 'react';
import { Alert, Button, Modal, message } from 'antd';
import { useTranslation } from 'react-i18next';
import JsonEditor from '@/components/common/JsonEditor';
import {
  extractGeminiCliCommonConfigFromCurrentFile,
  getGeminiCliCommonConfig,
  saveGeminiCliCommonConfig,
  saveGeminiCliLocalConfig,
} from '@/services/geminiCliApi';

interface GeminiCliCommonConfigModalProps {
  open: boolean;
  onCancel: () => void;
  onSuccess: () => void;
  isLocalProvider?: boolean;
}

const parseJsonConfig = (rawConfig?: string): unknown => {
  if (!rawConfig?.trim()) {
    return {};
  }

  try {
    return JSON.parse(rawConfig);
  } catch {
    return {};
  }
};

const GeminiCliCommonConfigModal: React.FC<GeminiCliCommonConfigModalProps> = ({
  open,
  onCancel,
  onSuccess,
  isLocalProvider = false,
}) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [configValue, setConfigValue] = React.useState<unknown>({});
  const [configValid, setConfigValid] = React.useState(true);
  const [rootDir, setRootDir] = React.useState<string | null>(null);

  const loadConfig = React.useCallback(async () => {
    setLoading(true);
    try {
      const config = await getGeminiCliCommonConfig();
      setConfigValue(parseJsonConfig(config?.config));
      setRootDir(config?.rootDir ?? null);
      setConfigValid(true);
    } catch (error) {
      console.error('Failed to load Gemini CLI common config:', error);
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
  }, [loadConfig, open]);

  const handleSave = async () => {
    if (!configValid) {
      message.error(t('geminicli.commonConfig.configInvalid'));
      return;
    }

    setLoading(true);
    try {
      const config = JSON.stringify(configValue || {}, null, 2);
      if (isLocalProvider) {
        await saveGeminiCliLocalConfig({ commonConfig: config, rootDir });
      } else {
        await saveGeminiCliCommonConfig({ config, rootDir });
      }
      message.success(t('common.success'));
      onSuccess();
      onCancel();
    } catch (error) {
      console.error('Failed to save Gemini CLI common config:', error);
      const errorMsg = error instanceof Error ? error.message : String(error);
      message.error(errorMsg || t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const handleExtractFromCurrentConfig = async () => {
    setLoading(true);
    try {
      const extractedConfig = await extractGeminiCliCommonConfigFromCurrentFile();
      setConfigValue(parseJsonConfig(extractedConfig.config));
      setRootDir(extractedConfig.rootDir ?? null);
      setConfigValid(true);
      message.success(t('geminicli.commonConfig.extractSuccess'));
    } catch (error) {
      console.error('Failed to extract Gemini CLI common config:', error);
      const errorMsg = error instanceof Error ? error.message : String(error);
      message.error(errorMsg || t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      title={t('geminicli.commonConfig.title')}
      open={open}
      onCancel={onCancel}
      onOk={() => {
        void handleSave();
      }}
      confirmLoading={loading}
      width={800}
      okButtonProps={{ disabled: !configValid }}
      okText={t('common.save')}
      cancelText={t('common.cancel')}
      footer={[
        <Button key="extract" onClick={handleExtractFromCurrentConfig} loading={loading}>
          {t('geminicli.commonConfig.extractFromCurrent')}
        </Button>,
        <Button key="cancel" onClick={onCancel} disabled={loading}>
          {t('common.cancel')}
        </Button>,
        <Button key="save" type="primary" onClick={() => void handleSave()} loading={loading} disabled={!configValid}>
          {t('common.save')}
        </Button>,
      ]}
    >
      {isLocalProvider && (
        <Alert
          message={t('geminicli.localConfigHint')}
          type="warning"
          showIcon
          style={{ marginBottom: 16 }}
        />
      )}

      <JsonEditor
        value={configValue}
        onChange={(value, isValid) => {
          setConfigValue(value);
          setConfigValid(isValid);
        }}
        height={400}
      />

      <Alert
        message={t('geminicli.commonConfig.description')}
        type="info"
        showIcon
        style={{ marginTop: 12 }}
      />
    </Modal>
  );
};

export default GeminiCliCommonConfigModal;
