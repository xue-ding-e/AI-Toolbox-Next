import React from 'react';
import { Modal, Input, Alert, message } from 'antd';
import { useTranslation } from 'react-i18next';
import { getClaudeCommonConfig, saveClaudeCommonConfig } from '@/services/claudeCodeApi';

const { TextArea } = Input;

interface CommonConfigModalProps {
  open: boolean;
  onCancel: () => void;
  onSuccess: () => void;
}

const CommonConfigModal: React.FC<CommonConfigModalProps> = ({
  open,
  onCancel,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [configText, setConfigText] = React.useState('{}');
  const [jsonError, setJsonError] = React.useState<string>('');

  // 加载现有配置
  React.useEffect(() => {
    if (open) {
      loadConfig();
    }
  }, [open]);

  const loadConfig = async () => {
    try {
      const config = await getClaudeCommonConfig();
      if (config && config.config) {
        try {
          const configObj = JSON.parse(config.config);
          setConfigText(JSON.stringify(configObj, null, 2));
        } catch (error) {
          console.error('Failed to parse config JSON:', error);
          setConfigText(config.config);
        }
      } else {
        setConfigText('{}');
      }
      setJsonError('');
    } catch (error) {
      console.error('Failed to load common config:', error);
      message.error(t('common.error'));
    }
  };

  const handleConfigChange = (value: string) => {
    setConfigText(value);
    // 验证 JSON 格式
    try {
      JSON.parse(value);
      setJsonError('');
    } catch (error) {
      setJsonError(t('claudecode.commonConfig.invalidJson'));
    }
  };

  const handleSave = async () => {
    // 验证 JSON
    try {
      JSON.parse(configText);
    } catch (error) {
      message.error(t('claudecode.commonConfig.invalidJson'));
      return;
    }

    setLoading(true);
    try {
      await saveClaudeCommonConfig(configText);
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

  return (
    <Modal
      title={t('claudecode.commonConfig.title')}
      open={open}
      onCancel={onCancel}
      onOk={handleSave}
      confirmLoading={loading}
      width={600}
      okText={t('common.save')}
      cancelText={t('common.cancel')}
    >
      <div style={{ marginBottom: 16 }}>
        <Alert
          message={t('claudecode.commonConfig.description')}
          type="info"
          showIcon
          style={{ marginBottom: 12 }}
        />
      </div>

      <TextArea
        value={configText}
        onChange={(e) => handleConfigChange(e.target.value)}
        placeholder={t('claudecode.commonConfig.placeholder')}
        rows={12}
        style={{
          fontFamily: 'monospace',
          fontSize: 12,
        }}
      />

      {jsonError && (
        <div style={{ marginTop: 8 }}>
          <Alert message={jsonError} type="error" showIcon />
        </div>
      )}

      <div style={{ marginTop: 12 }}>
        <Alert
          message={t('claudecode.commonConfig.hint')}
          type="info"
          showIcon
          closable
        />
      </div>
    </Modal>
  );
};

export default CommonConfigModal;
