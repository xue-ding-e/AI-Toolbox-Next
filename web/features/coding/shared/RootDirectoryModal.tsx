import React from 'react';
import { Button, Form, Input, Modal, Space, Typography, message } from 'antd';
import { ExclamationCircleOutlined, FolderOpenOutlined, InfoCircleOutlined, ReloadOutlined } from '@ant-design/icons';
import { open } from '@tauri-apps/plugin-dialog';

const { Text } = Typography;

export interface RootPathInfo {
  path: string;
  source: 'custom' | 'env' | 'shell' | 'default';
}

interface RootDirectoryModalProps {
  open: boolean;
  title: string;
  currentPathInfo: RootPathInfo | null;
  currentSourceText: string;
  currentSourceLabel: string;
  description: string;
  envWarningTitle: string;
  envWarningDesc: string;
  envHint?: string | null;
  customPathLabel: string;
  placeholder: string;
  selectFolderText: string;
  folderOnlyTitle: string;
  folderOnlyDesc: string;
  saveSuccessText: string;
  resetSuccessText: string;
  resetText: string;
  cancelText: string;
  saveText: string;
  errorText: string;
  onCancel: () => void;
  onSubmit: (rootDir: string | null) => Promise<void>;
  onReset: () => Promise<void>;
}

const RootDirectoryModal: React.FC<RootDirectoryModalProps> = ({
  open: modalOpen,
  title,
  currentPathInfo,
  currentSourceText,
  currentSourceLabel,
  description,
  envWarningTitle,
  envWarningDesc,
  envHint,
  customPathLabel,
  placeholder,
  selectFolderText,
  folderOnlyTitle,
  folderOnlyDesc,
  saveSuccessText,
  resetSuccessText,
  resetText,
  cancelText,
  saveText,
  errorText,
  onCancel,
  onSubmit,
  onReset,
}) => {
  const [form] = Form.useForm();
  const [loading, setLoading] = React.useState(false);

  React.useEffect(() => {
    if (modalOpen) {
      form.setFieldsValue({
        customPath: currentPathInfo?.source === 'custom' ? currentPathInfo.path : '',
      });
    }
  }, [currentPathInfo, form, modalOpen]);

  const handleSelectFolder = async () => {
    try {
      const selected = await open({
        title: selectFolderText,
        multiple: false,
        directory: true,
      });

      if (selected && typeof selected === 'string') {
        form.setFieldsValue({ customPath: selected });
      }
    } catch (error) {
      console.error('Failed to select folder:', error);
      message.error(errorText);
    }
  };

  const handleReset = async () => {
    try {
      setLoading(true);
      await onReset();
      message.success(resetSuccessText);
    } catch (error) {
      console.error('Failed to reset root directory:', error);
      message.error(errorText);
    } finally {
      setLoading(false);
    }
  };

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setLoading(true);
      const rootDir = values.customPath?.trim() ? values.customPath.trim() : null;
      await onSubmit(rootDir);
      message.success(saveSuccessText);
    } catch (error) {
      console.error('Failed to save root directory:', error);
      message.error(errorText);
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      title={title}
      open={modalOpen}
      onCancel={onCancel}
      footer={[
        <Button key="reset" icon={<ReloadOutlined />} onClick={handleReset} loading={loading}>
          {resetText}
        </Button>,
        <Button key="cancel" onClick={onCancel}>
          {cancelText}
        </Button>,
        <Button key="submit" type="primary" onClick={handleSubmit} loading={loading}>
          {saveText}
        </Button>,
      ]}
      width={640}
    >
      <Space direction="vertical" style={{ width: '100%' }} size="middle">
        <div style={{ display: 'flex', alignItems: 'flex-start', gap: 8 }}>
          <InfoCircleOutlined style={{ marginTop: 3, color: 'var(--color-text-secondary)' }} />
          <div>
            <Text style={{ fontSize: 12 }}>
              <Text strong style={{ fontSize: 12 }}>
                {folderOnlyTitle}:
              </Text>{' '}
              <Text type="secondary" style={{ fontSize: 12 }}>
                {folderOnlyDesc}
              </Text>
            </Text>
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'flex-start', gap: 8 }}>
          <ExclamationCircleOutlined style={{ marginTop: 3, color: 'var(--color-text-secondary)' }} />
          <div>
            <Text style={{ fontSize: 12 }}>
              <Text strong style={{ fontSize: 12 }}>
                {envWarningTitle}:
              </Text>{' '}
              <Text type="secondary" style={{ fontSize: 12 }}>
                {envWarningDesc}
              </Text>
            </Text>
          </div>
        </div>

        <div>
          <Text type="secondary">{currentSourceText}: </Text>
          <Text strong>{currentSourceLabel}</Text>
        </div>

        <Text type="secondary">{description}</Text>

        {envHint ? (
          <div style={{ display: 'flex', alignItems: 'flex-start', gap: 8 }}>
            <InfoCircleOutlined style={{ marginTop: 3, color: 'var(--color-text-secondary)' }} />
            <Text type="secondary">{envHint}</Text>
          </div>
        ) : null}

        <Form form={form} layout="vertical">
          <Form.Item name="customPath" label={customPathLabel}>
            <Input
              placeholder={placeholder}
              addonAfter={
                <Button
                  type="text"
                  size="small"
                  icon={<FolderOpenOutlined />}
                  onClick={handleSelectFolder}
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

export default RootDirectoryModal;
