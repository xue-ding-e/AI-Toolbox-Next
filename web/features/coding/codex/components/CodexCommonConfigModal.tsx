import React from 'react';
import { Modal, Alert, message } from 'antd';
import { useTranslation } from 'react-i18next';
import { getCodexCommonConfig, saveCodexCommonConfig } from '@/services/codexApi';
import JsonEditor from '@/components/common/JsonEditor';

interface CodexCommonConfigModalProps {
  open: boolean;
  onCancel: () => void;
  onSuccess: () => void;
}

const CodexCommonConfigModal: React.FC<CodexCommonConfigModalProps> = ({
  open,
  onCancel,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [configValue, setConfigValue] = React.useState<string>('');

  // Load existing config
  React.useEffect(() => {
    if (open) {
      loadConfig();
    }
  }, [open]);

  const loadConfig = async () => {
    try {
      const config = await getCodexCommonConfig();
      if (config && config.config) {
        setConfigValue(config.config);
      } else {
        setConfigValue('');
      }
    } catch (error) {
      console.error('Failed to load common config:', error);
      message.error(t('common.error'));
    }
  };

  const handleSave = async () => {
    setLoading(true);
    try {
      await saveCodexCommonConfig(configValue);
      message.success(t('common.success'));
      onSuccess();
      onCancel();
    } catch (error) {
      console.error('Failed to save common config:', error);
      message.error(t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const handleEditorChange = (value: unknown) => {
    if (typeof value === 'string') {
      setConfigValue(value);
    } else if (typeof value === 'object' && value !== null) {
      setConfigValue(JSON.stringify(value, null, 2));
    }
  };

  return (
    <Modal
      title={t('codex.commonConfig.title')}
      open={open}
      onCancel={onCancel}
      onOk={handleSave}
      confirmLoading={loading}
      width={700}
      okText={t('common.save')}
      cancelText={t('common.cancel')}
    >
      <div style={{ marginBottom: 16 }}>
        <Alert
          title={t('codex.commonConfig.description')}
          type="info"
          showIcon
          style={{ marginBottom: 12 }}
        />
      </div>

      <JsonEditor
        value={configValue}
        onChange={handleEditorChange}
        mode="text"
        height={400}
        minHeight={200}
        maxHeight={600}
        resizable
      />

      <div style={{ marginTop: 12 }}>
        <Alert
          title={t('codex.commonConfig.hint')}
          type="info"
          showIcon
          closable
        />
      </div>
    </Modal>
  );
};

export default CodexCommonConfigModal;
