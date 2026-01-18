/**
 * File Mapping Edit Modal
 *
 * Modal for adding/editing file mappings
 */

import React, { useEffect } from 'react';
import { Modal, Form, Input, Select, Switch, Space, Typography, Divider } from 'antd';
import { useTranslation } from 'react-i18next';
import { wslAddFileMapping, wslUpdateFileMapping } from '@/services/wslSyncApi';
import type { FileMapping } from '@/types/wslsync';

const { Text } = Typography;

interface FileMappingModalProps {
  open: boolean;
  onClose: () => void;
  mapping: FileMapping | null;
}

export const FileMappingModal: React.FC<FileMappingModalProps> = ({ open, onClose, mapping }) => {
  const { t } = useTranslation();
  const [form] = Form.useForm();

  const isEdit = mapping !== null;

  useEffect(() => {
    if (open) {
      if (mapping && mapping.id) {
        form.setFieldsValue(mapping);
      } else {
        form.resetFields();
        form.setFieldsValue({
          module: mapping?.module || 'opencode',
          enabled: true,
          isPattern: false,
        });
      }
    }
  }, [open, mapping, form]);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();

      // Generate ID if new
      const id = mapping?.id || `custom-${Date.now()}`;

      const newMapping: FileMapping = {
        ...values,
        id,
      };

      // Save to database (will trigger wsl-config-changed event to refresh UI)
      if (isEdit && mapping?.id) {
        await wslUpdateFileMapping(newMapping);
      } else {
        await wslAddFileMapping(newMapping);
      }

      onClose();
    } catch (error) {
      console.error('Failed to save mapping:', error);
    }
  };

  return (
    <Modal
      title={isEdit && mapping?.id ? t('settings.wsl.editMapping') : t('settings.wsl.addMapping')}
      open={open}
      onOk={handleSubmit}
      onCancel={onClose}
      width={600}
      okText={t('common.save')}
      cancelText={t('common.cancel')}
    >
      <Form form={form} layout="horizontal" labelCol={{ span: 6 }} wrapperCol={{ span: 18 }}>
        <Form.Item
          name="name"
          label={t('settings.wsl.mappingName')}
          rules={[{ required: true, message: t('settings.wsl.mappingNameRequired') }]}
        >
          <Input placeholder={t('settings.wsl.mappingNamePlaceholder')} />
        </Form.Item>

        <Form.Item
          name="module"
          label={t('settings.wsl.module')}
          rules={[{ required: true }]}
        >
          <Select>
            <Select.Option value="opencode">OpenCode</Select.Option>
            <Select.Option value="claude">Claude Code</Select.Option>
            <Select.Option value="codex">Codex</Select.Option>
          </Select>
        </Form.Item>

        <Divider />

        <Form.Item
          name="windowsPath"
          label={
            <Space>
              <Text>Windows</Text>
              <Text type="secondary" style={{ fontSize: 12 }}>路径</Text>
            </Space>
          }
          rules={[{ required: true, message: t('settings.wsl.windowsPathRequired') }]}
          extra={t('settings.wsl.windowsPathHint')}
        >
          <Input placeholder="%USERPROFILE%\.config\opencode\config.json" />
        </Form.Item>

        <Form.Item
          name="wslPath"
          label={
            <Space>
              <Text>WSL</Text>
              <Text type="secondary" style={{ fontSize: 12 }}>路径</Text>
            </Space>
          }
          rules={[{ required: true, message: t('settings.wsl.wslPathRequired') }]}
          extra={t('settings.wsl.wslPathHint')}
        >
          <Input placeholder="~/.config/opencode/config.json" />
        </Form.Item>

        <Divider />

        <Form.Item
          name="enabled"
          label={
            <Space>
              <Text>启用</Text>
              <Text type="secondary" style={{ fontSize: 12 }}>同步</Text>
            </Space>
          }
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>

        <Form.Item
          name="isPattern"
          label={
            <Space>
              <Text>模式</Text>
              <Text type="secondary" style={{ fontSize: 12 }}>通配符</Text>
            </Space>
          }
          valuePropName="checked"
          extra={t('settings.wsl.patternModeHint')}
        >
          <Switch />
        </Form.Item>
      </Form>
    </Modal>
  );
};
