import React from 'react';
import { Modal, Form, Input, Button, Space, Alert, Typography, message } from 'antd';
import { FolderOpenOutlined, ReloadOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { open } from '@tauri-apps/plugin-dialog';
import { 
  getOpenCodeCommonConfig, 
  saveOpenCodeCommonConfig, 
  type ConfigPathInfo 
} from '@/services/opencodeApi';

const { Text } = Typography;

interface ConfigPathModalProps {
  open: boolean;
  currentPathInfo: ConfigPathInfo | null;
  onCancel: () => void;
  onSuccess: () => void;
}

const ConfigPathModal: React.FC<ConfigPathModalProps> = ({
  open: modalOpen,
  currentPathInfo,
  onCancel,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  const [loading, setLoading] = React.useState(false);

  React.useEffect(() => {
    if (modalOpen && currentPathInfo) {
      // Only set custom path if source is 'custom'
      form.setFieldsValue({
        customPath: currentPathInfo.source === 'custom' ? currentPathInfo.path : '',
      });
    }
  }, [modalOpen, currentPathInfo, form]);

  const handleSelectFile = async () => {
    try {
      const selected = await open({
        title: t('opencode.configPathSource.modal.selectFile'),
        multiple: false,
        directory: false,
        filters: [
          {
            name: 'JSON',
            extensions: ['json', 'jsonc'],
          },
        ],
      });

      if (selected && typeof selected === 'string') {
        form.setFieldsValue({ customPath: selected });
      }
    } catch (error) {
      console.error('Failed to select file:', error);
      message.error(t('common.error'));
    }
  };

  const handleReset = async () => {
    try {
      setLoading(true);
      const currentConfig = await getOpenCodeCommonConfig();
      await saveOpenCodeCommonConfig({
        configPath: null,
        showPluginsInTray: currentConfig?.showPluginsInTray ?? false,
        updatedAt: currentConfig?.updatedAt || new Date().toISOString(),
      });
      message.success(t('opencode.configPathSource.modal.resetSuccess'));
      onSuccess();
    } catch (error) {
      console.error('Failed to reset path:', error);
      message.error(t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setLoading(true);

      const currentConfig = await getOpenCodeCommonConfig();
      await saveOpenCodeCommonConfig({
        configPath: values.customPath || null,
        showPluginsInTray: currentConfig?.showPluginsInTray ?? false,
        updatedAt: new Date().toISOString(),
      });

      message.success(t('opencode.configPathSource.modal.saveSuccess'));
      onSuccess();
    } catch (error) {
      console.error('Failed to save path:', error);
      message.error(t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const getSourceLabel = () => {
    if (!currentPathInfo) return '';
    switch (currentPathInfo.source) {
      case 'custom':
        return t('opencode.configPathSource.modal.sourceCustom');
      case 'env':
        return t('opencode.configPathSource.modal.sourceEnv');
      case 'default':
        return t('opencode.configPathSource.modal.sourceDefault');
      default:
        return '';
    }
  };

  return (
    <Modal
      title={t('opencode.configPathSource.modal.title')}
      open={modalOpen}
      onCancel={onCancel}
      footer={[
        <Button key="reset" icon={<ReloadOutlined />} onClick={handleReset} loading={loading}>
          {t('opencode.configPathSource.modal.reset')}
        </Button>,
        <Button key="cancel" onClick={onCancel}>
          {t('common.cancel')}
        </Button>,
        <Button key="submit" type="primary" onClick={handleSubmit} loading={loading}>
          {t('common.save')}
        </Button>,
      ]}
      width={600}
    >
      <Space orientation="vertical" style={{ width: '100%' }} size="middle">
        <Alert
          message={t('opencode.configPathSource.modal.envWarningTitle')}
          description={t('opencode.configPathSource.modal.envWarningDesc')}
          type="warning"
          showIcon
        />

        <div>
          <Text type="secondary">{t('opencode.configPathSource.modal.currentSource')}: </Text>
          <Text strong>{getSourceLabel()}</Text>
        </div>

        {currentPathInfo?.source === 'env' && (
          <Alert
            message={t('opencode.configPathSource.modal.envHint', { path: currentPathInfo.path })}
            type="warning"
            showIcon
          />
        )}

        <Form form={form} layout="vertical">
          <Form.Item
            name="customPath"
            label={t('opencode.configPathSource.modal.customPath')}
          >
            <Input
              placeholder={t('opencode.configPathSource.modal.placeholder')}
              addonAfter={
                <Button
                  type="text"
                  size="small"
                  icon={<FolderOpenOutlined />}
                  onClick={handleSelectFile}
                  style={{ margin: -7 }}
                />
              }
            />
          </Form.Item>
        </Form>
      </Space>
    </Modal>
  );
};

export default ConfigPathModal;

