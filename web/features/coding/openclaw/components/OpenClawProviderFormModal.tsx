import React from 'react';
import { Modal, Form, Input, Select, Button, Divider } from 'antd';
import { ImportOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { OpenClawProviderConfig } from '@/types/openclaw';

const API_PROTOCOLS = [
  { value: 'openai-completions', label: 'OpenAI Completions' },
  { value: 'openai-responses', label: 'OpenAI Responses' },
  { value: 'anthropic-messages', label: 'Anthropic Messages' },
  { value: 'google-generative-ai', label: 'Google Generative AI' },
  { value: 'bedrock-converse-stream', label: 'Bedrock Converse Stream' },
];

export interface ProviderFormValues {
  providerId: string;
  baseUrl?: string;
  apiKey?: string;
  api?: string;
}

interface Props {
  open: boolean;
  editingProvider?: { id: string; config: OpenClawProviderConfig } | null;
  existingIds: string[];
  onCancel: () => void;
  onSubmit: (values: ProviderFormValues) => void;
  onOpenImport?: () => void;
}

const OpenClawProviderFormModal: React.FC<Props> = ({
  open: modalOpen,
  editingProvider,
  existingIds,
  onCancel,
  onSubmit,
  onOpenImport,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  const isEdit = !!editingProvider;

  React.useEffect(() => {
    if (modalOpen) {
      if (editingProvider) {
        form.setFieldsValue({
          providerId: editingProvider.id,
          baseUrl: editingProvider.config.baseUrl || '',
          apiKey: editingProvider.config.apiKey || '',
          api: editingProvider.config.api || 'openai-completions',
        });
      } else {
        form.resetFields();
        form.setFieldsValue({ api: 'openai-completions' });
      }
    }
  }, [modalOpen, editingProvider, form]);

  const handleOk = async () => {
    try {
      const values = await form.validateFields();
      onSubmit(values);
    } catch {
      // validation error
    }
  };

  return (
    <Modal
      title={isEdit ? t('openclaw.providers.editProvider') : t('openclaw.providers.addProvider')}
      open={modalOpen}
      onOk={handleOk}
      onCancel={onCancel}
      okText={t('common.save')}
      cancelText={t('common.cancel')}
      destroyOnClose
    >
      <Form form={form} layout="vertical" autoComplete="off">
        <Form.Item
          name="providerId"
          label={t('openclaw.providers.providerId')}
          rules={[
            { required: true, message: t('common.required') },
            {
              validator: (_, value) => {
                if (!isEdit && value && existingIds.includes(value)) {
                  return Promise.reject(new Error('Provider ID already exists'));
                }
                return Promise.resolve();
              },
            },
          ]}
        >
          <Input
            placeholder={t('openclaw.providers.providerIdPlaceholder')}
            disabled={isEdit}
          />
        </Form.Item>

        <Form.Item name="baseUrl" label={t('openclaw.providers.baseUrl')}>
          <Input placeholder={t('openclaw.providers.baseUrlPlaceholder')} />
        </Form.Item>

        <Form.Item name="apiKey" label={t('openclaw.providers.apiKey')}>
          <Input.Password placeholder={t('openclaw.providers.apiKeyPlaceholder')} />
        </Form.Item>

        <Form.Item name="api" label={t('openclaw.providers.apiProtocol')}>
          <Select options={API_PROTOCOLS} />
        </Form.Item>
      </Form>

      {/* Import from OpenCode button — only in add mode */}
      {!isEdit && onOpenImport && (
        <>
          <Divider style={{ margin: '8px 0 12px' }} />
          <div style={{ textAlign: 'center' }}>
            <Button
              type="dashed"
              icon={<ImportOutlined />}
              onClick={onOpenImport}
            >
              {t('openclaw.providers.importFromOpenCode')}
            </Button>
          </div>
        </>
      )}
    </Modal>
  );
};

export default OpenClawProviderFormModal;
