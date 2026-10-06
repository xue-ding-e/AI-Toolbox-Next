import React from 'react';
import { Modal, Alert, Button, message, Space } from 'antd';
import { useTranslation } from 'react-i18next';
import JsonEditor from '@/components/common/JsonEditor';
import { readZcodeCliConfig, saveZcodeCliConfig } from '@/services/zcodeApi';

interface ZcodeCommonConfigModalProps {
  open: boolean;
  onCancel: () => void;
  onSuccess: () => void;
}

/**
 * Editor for `~/.zcode/cli/config.json`.
 *
 * That file holds everything the provider UI does not cover: MCP servers,
 * hooks, plugins and permission switches. It is independent of
 * `provider_config.json`, so it is edited verbatim rather than projected —
 * whatever the user types is what lands on disk.
 */
const ZcodeCommonConfigModal: React.FC<ZcodeCommonConfigModalProps> = ({
  open,
  onCancel,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [configValue, setConfigValue] = React.useState('');
  const [isJsonValid, setIsJsonValid] = React.useState(true);

  const loadConfig = React.useCallback(async () => {
    setLoading(true);
    try {
      const config = await readZcodeCliConfig();
      setConfigValue(config);
      setIsJsonValid(true);
    } catch (error) {
      console.error('Failed to load ZCode CLI config:', error);
      const detail = error instanceof Error ? error.message : String(error);
      message.error(detail || t('common.error'));
    } finally {
      setLoading(false);
    }
  }, [t]);

  React.useEffect(() => {
    if (open) {
      void loadConfig();
    }
  }, [loadConfig, open]);

  // `onChange` reports validity but hands back a parsed object; the raw text is
  // what we must persist, so both callbacks feed the same state.
  const handleEditorChange = React.useCallback((_value: unknown, valid: boolean) => {
    setIsJsonValid(valid);
  }, []);

  const handleRawChange = React.useCallback((raw: string) => {
    setConfigValue(raw);
  }, []);

  const handleSave = async () => {
    if (!isJsonValid) {
      message.error(t('zcode.commonConfig.invalidJson'));
      return;
    }
    setLoading(true);
    try {
      await saveZcodeCliConfig(configValue);
      message.success(t('common.success'));
      onSuccess();
      onCancel();
    } catch (error) {
      console.error('Failed to save ZCode CLI config:', error);
      const detail = error instanceof Error ? error.message : String(error);
      message.error(detail || t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      title={t('zcode.commonConfig.title')}
      open={open}
      onCancel={onCancel}
      width={800}
      footer={[
        <Button key="reload" onClick={() => void loadConfig()} loading={loading}>
          {t('zcode.commonConfig.reload')}
        </Button>,
        <Button key="cancel" onClick={onCancel} disabled={loading}>
          {t('common.cancel')}
        </Button>,
        <Button key="save" type="primary" onClick={handleSave} loading={loading}>
          {t('common.save')}
        </Button>,
      ]}
    >
      <Space orientation="vertical" size={12} style={{ width: '100%' }}>
        <Alert message={t('zcode.commonConfig.description')} type="info" showIcon />
        {!isJsonValid && (
          <Alert message={t('zcode.commonConfig.invalidJson')} type="error" showIcon />
        )}
        <JsonEditor value={configValue} onChange={handleEditorChange} onRawChange={handleRawChange} height={420} />
      </Space>
    </Modal>
  );
};

export default ZcodeCommonConfigModal;
