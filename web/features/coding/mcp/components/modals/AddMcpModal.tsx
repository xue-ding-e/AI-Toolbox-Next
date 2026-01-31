import React, { useState } from 'react';
import { Modal, Form, Input, Select, Button, Space } from 'antd';
import { PlusOutlined, MinusCircleOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { CreateMcpServerInput, McpTool, StdioConfig, HttpConfig } from '../../types';
import styles from './AddMcpModal.module.less';

interface AddMcpModalProps {
  open: boolean;
  tools: McpTool[];
  onClose: () => void;
  onSubmit: (input: CreateMcpServerInput) => Promise<void>;
}

export const AddMcpModal: React.FC<AddMcpModalProps> = ({
  open,
  tools,
  onClose,
  onSubmit,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  const [loading, setLoading] = useState(false);
  const [serverType, setServerType] = useState<'stdio' | 'http' | 'sse'>('stdio');

  const installedTools = tools.filter((t) => t.installed);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setLoading(true);

      let serverConfig: StdioConfig | HttpConfig;
      if (serverType === 'stdio') {
        serverConfig = {
          command: values.command,
          args: values.args?.filter((a: string) => a) || [],
          env: values.env ? JSON.parse(values.env) : undefined,
        };
      } else {
        serverConfig = {
          url: values.url,
          headers: values.headers ? JSON.parse(values.headers) : undefined,
        };
      }

      await onSubmit({
        name: values.name,
        server_type: serverType,
        server_config: serverConfig,
        enabled_tools: values.enabled_tools || [],
        description: values.description,
        tags: values.tags?.filter((t: string) => t) || [],
      });

      form.resetFields();
      onClose();
    } catch (error) {
      console.error('Form validation failed:', error);
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      title={t('mcp.addServer')}
      open={open}
      onCancel={onClose}
      footer={[
        <Button key="cancel" onClick={onClose}>
          {t('common.cancel')}
        </Button>,
        <Button key="submit" type="primary" loading={loading} onClick={handleSubmit}>
          {t('common.save')}
        </Button>,
      ]}
      width={600}
    >
      <Form
        form={form}
        layout="horizontal"
        labelCol={{ span: 6 }}
        wrapperCol={{ span: 18 }}
        initialValues={{ server_type: 'stdio' }}
      >
        <Form.Item
          label={t('mcp.name')}
          name="name"
          rules={[{ required: true, message: t('mcp.nameRequired') }]}
        >
          <Input placeholder={t('mcp.namePlaceholder')} />
        </Form.Item>

        <Form.Item label={t('mcp.type')} name="server_type">
          <Select
            value={serverType}
            onChange={(v) => setServerType(v)}
            options={[
              { label: 'Stdio', value: 'stdio' },
              { label: 'HTTP', value: 'http' },
              { label: 'SSE', value: 'sse' },
            ]}
          />
        </Form.Item>

        {serverType === 'stdio' ? (
          <>
            <Form.Item
              label={t('mcp.command')}
              name="command"
              rules={[{ required: true, message: t('mcp.commandRequired') }]}
            >
              <Input placeholder="npx -y @modelcontextprotocol/server-xxx" />
            </Form.Item>

            <Form.Item label={t('mcp.args')}>
              <Form.List name="args">
                {(fields, { add, remove }) => (
                  <>
                    {fields.map((field, index) => (
                      <Space key={field.key} className={styles.argRow}>
                        <Form.Item {...field} noStyle>
                          <Input placeholder={`${t('mcp.arg')} ${index + 1}`} />
                        </Form.Item>
                        <MinusCircleOutlined onClick={() => remove(field.name)} />
                      </Space>
                    ))}
                    <Button type="dashed" onClick={() => add()} block icon={<PlusOutlined />}>
                      {t('mcp.addArg')}
                    </Button>
                  </>
                )}
              </Form.List>
            </Form.Item>

            <Form.Item label={t('mcp.env')} name="env">
              <Input.TextArea
                placeholder='{"VAR": "value"}'
                rows={2}
              />
            </Form.Item>
          </>
        ) : (
          <>
            <Form.Item
              label={t('mcp.url')}
              name="url"
              rules={[{ required: true, message: t('mcp.urlRequired') }]}
            >
              <Input placeholder="https://example.com/mcp" />
            </Form.Item>

            <Form.Item label={t('mcp.headers')} name="headers">
              <Input.TextArea
                placeholder='{"Authorization": "Bearer xxx"}'
                rows={2}
              />
            </Form.Item>
          </>
        )}

        <Form.Item label={t('mcp.enabledTools')} name="enabled_tools">
          <Select
            mode="multiple"
            placeholder={t('mcp.selectTools')}
            options={installedTools.map((t) => ({
              label: t.display_name,
              value: t.key,
            }))}
          />
        </Form.Item>

        <Form.Item label={t('mcp.description')} name="description">
          <Input.TextArea rows={2} placeholder={t('mcp.descriptionPlaceholder')} />
        </Form.Item>
      </Form>
    </Modal>
  );
};

export default AddMcpModal;
