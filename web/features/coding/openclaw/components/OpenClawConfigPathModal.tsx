import React from 'react';
import { Modal, Form, Input, Button, Space, Typography, message } from 'antd';
import { FolderOpenOutlined, ReloadOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { open } from '@tauri-apps/plugin-dialog';
import {
  saveOpenClawCommonConfig,
} from '@/services/openclawApi';
import type { OpenClawConfigPathInfo } from '@/types/openclaw';

const { Text } = Typography;

interface Props {
  open: boolean;
  currentPathInfo: OpenClawConfigPathInfo | null;
  onCancel: () => void;
  onSuccess: () => void;
}

const OpenClawConfigPathModal: React.FC<Props> = ({
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
      form.setFieldsValue({
        customPath: currentPathInfo.source === 'custom' ? currentPathInfo.path : '',
      });
    }
  }, [modalOpen, currentPathInfo, form]);

  const handleSelectFile = async () => {
    try {
      const selected = await open({
        title: t('openclaw.configPath_modal.title'),
        multiple: false,
        directory: false,
        filters: [{ name: 'JSON', extensions: ['json', 'json5'] }],
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
      await saveOpenClawCommonConfig({
        configPath: null,
        updatedAt: new Date().toISOString(),
      });
      message.success(t('common.success'));
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
      await saveOpenClawCommonConfig({
        configPath: values.customPath || null,
        updatedAt: new Date().toISOString(),
      });
      message.success(t('common.success'));
      onSuccess();
    } catch (error) {
      console.error('Failed to save path:', error);
      message.error(t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      title={t('openclaw.configPath_modal.title')}
      open={modalOpen}
      onCancel={onCancel}
      footer={[
        <Button key="reset" icon={<ReloadOutlined />} onClick={handleReset} loading={loading}>
          {t('openclaw.configPath_modal.reset')}
        </Button>,
        <Button key="cancel" onClick={onCancel}>
          {t('common.cancel')}
        </Button>,
        <Button key="submit" type="primary" onClick={handleSubmit} loading={loading}>
          {t('openclaw.configPath_modal.save')}
        </Button>,
      ]}
      width={560}
    >
      <Space direction="vertical" style={{ width: '100%' }} size="middle">
        <div>
          <Text type="secondary">{t('openclaw.configPath_modal.description')}</Text>
        </div>
        <Form form={form} layout="vertical">
          <Form.Item name="customPath" label={t('openclaw.customConfigPath')}>
            <Input
              placeholder={t('openclaw.configPath_modal.pathPlaceholder')}
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

export default OpenClawConfigPathModal;
