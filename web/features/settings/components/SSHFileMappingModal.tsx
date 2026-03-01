/**
 * SSH File Mapping Edit Modal
 *
 * Modal for adding/editing SSH file mappings
 */

import React, { useEffect } from 'react';
import { Modal, Form, Input, Select, Switch, Space, Typography, Divider } from 'antd';
import { useTranslation } from 'react-i18next';
import { sshAddFileMapping, sshUpdateFileMapping } from '@/services/sshSyncApi';
import type { SSHFileMapping } from '@/types/sshsync';

const { Text } = Typography;

interface SSHFileMappingModalProps {
  open: boolean;
  onClose: () => void;
  mapping: SSHFileMapping | null;
}

export const SSHFileMappingModal: React.FC<SSHFileMappingModalProps> = ({ open, onClose, mapping }) => {
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
          isDirectory: false,
        });
      }
    }
  }, [open, mapping, form]);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      const id = mapping?.id || `custom-${Date.now()}`;

      const newMapping: SSHFileMapping = {
        ...values,
        id,
      };

      if (isEdit && mapping?.id) {
        await sshUpdateFileMapping(newMapping);
      } else {
        await sshAddFileMapping(newMapping);
      }

      onClose();
    } catch (error) {
      console.error('Failed to save SSH mapping:', error);
    }
  };

  return (
    <Modal
      title={isEdit && mapping?.id ? t('settings.ssh.editMapping') : t('settings.ssh.addMapping')}
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
          label={t('settings.ssh.mappingName')}
          rules={[{ required: true, message: t('settings.ssh.mappingNameRequired') }]}
        >
          <Input placeholder={t('settings.ssh.mappingNamePlaceholder')} />
        </Form.Item>

        <Form.Item
          name="module"
          label={t('settings.ssh.module')}
          rules={[{ required: true }]}
        >
          <Select>
            <Select.Option value="opencode">OpenCode</Select.Option>
            <Select.Option value="claude">Claude Code</Select.Option>
            <Select.Option value="codex">Codex</Select.Option>
            <Select.Option value="openclaw">OpenClaw</Select.Option>
          </Select>
        </Form.Item>

        <Divider />

        <Form.Item
          name="localPath"
          label={t('settings.ssh.localPath')}
          rules={[{ required: true, message: t('settings.ssh.localPathRequired') }]}
          extra={t('settings.ssh.localPathHint')}
        >
          <Input placeholder="~/.config/opencode/config.json" />
        </Form.Item>

        <Form.Item
          name="remotePath"
          label={t('settings.ssh.remotePath')}
          rules={[{ required: true, message: t('settings.ssh.remotePathRequired') }]}
          extra={t('settings.ssh.remotePathHint')}
        >
          <Input placeholder="~/.config/opencode/config.json" />
        </Form.Item>

        <Divider />

        <Form.Item
          name="enabled"
          label={
            <Space>
              <Text>{t('settings.ssh.enableMapping')}</Text>
            </Space>
          }
          valuePropName="checked"
        >
          <Switch />
        </Form.Item>

        <Form.Item
          name="isPattern"
          label={t('settings.ssh.patternMode')}
          valuePropName="checked"
          extra={t('settings.ssh.patternModeHint')}
        >
          <Switch />
        </Form.Item>

        <Form.Item
          name="isDirectory"
          label={t('settings.ssh.directoryMode')}
          valuePropName="checked"
          extra={t('settings.ssh.directoryModeHint')}
        >
          <Switch />
        </Form.Item>
      </Form>
    </Modal>
  );
};
