import React from 'react';
import { Modal, Form, Input, InputNumber, Switch } from 'antd';
import { useTranslation } from 'react-i18next';
import type { OpenClawModel } from '@/types/openclaw';

export interface ModelFormValues {
  id: string;
  name?: string;
  contextWindow?: number;
  maxTokens?: number;
  reasoning?: boolean;
  costInput?: number;
  costOutput?: number;
  costCacheRead?: number;
  costCacheWrite?: number;
}

interface Props {
  open: boolean;
  editingModel?: OpenClawModel | null;
  existingIds: string[];
  onCancel: () => void;
  onSubmit: (values: ModelFormValues) => void;
}

const formItemLayout = {
  labelCol: { span: 8 },
  wrapperCol: { span: 16 },
};

const OpenClawModelFormModal: React.FC<Props> = ({
  open: modalOpen,
  editingModel,
  existingIds,
  onCancel,
  onSubmit,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  const isEdit = !!editingModel;

  React.useEffect(() => {
    if (modalOpen) {
      if (editingModel) {
        form.setFieldsValue({
          id: editingModel.id,
          name: editingModel.name || '',
          contextWindow: editingModel.contextWindow,
          maxTokens: editingModel.maxTokens,
          reasoning: editingModel.reasoning || false,
          costInput: editingModel.cost?.input,
          costOutput: editingModel.cost?.output,
          costCacheRead: editingModel.cost?.cacheRead,
          costCacheWrite: editingModel.cost?.cacheWrite,
        });
      } else {
        form.resetFields();
      }
    }
  }, [modalOpen, editingModel, form]);

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
      title={isEdit ? t('openclaw.providers.editModel') : t('openclaw.providers.addModel')}
      open={modalOpen}
      onOk={handleOk}
      onCancel={onCancel}
      okText={t('common.save')}
      cancelText={t('common.cancel')}
      destroyOnClose
      width={520}
    >
      <Form form={form} layout="horizontal" autoComplete="off" {...formItemLayout}>
        <Form.Item
          name="id"
          label={t('openclaw.providers.modelId')}
          rules={[
            { required: true, message: t('common.required') },
            {
              validator: (_, value) => {
                if (!isEdit && value && existingIds.includes(value)) {
                  return Promise.reject(new Error('Model ID already exists'));
                }
                return Promise.resolve();
              },
            },
          ]}
        >
          <Input placeholder={t('openclaw.providers.modelIdPlaceholder')} disabled={isEdit} />
        </Form.Item>

        <Form.Item name="name" label={t('openclaw.providers.modelName')}>
          <Input />
        </Form.Item>

        <Form.Item name="contextWindow" label={t('openclaw.providers.contextLimit')}>
          <InputNumber min={0} style={{ width: '100%' }} />
        </Form.Item>

        <Form.Item name="maxTokens" label={t('openclaw.providers.outputLimit')}>
          <InputNumber min={0} style={{ width: '100%' }} />
        </Form.Item>

        <Form.Item name="reasoning" label={t('openclaw.providers.reasoning')} valuePropName="checked">
          <Switch />
        </Form.Item>

        <Form.Item name="costInput" label={t('openclaw.providers.costInput')}>
          <InputNumber min={0} step={0.01} style={{ width: '100%' }} />
        </Form.Item>

        <Form.Item name="costOutput" label={t('openclaw.providers.costOutput')}>
          <InputNumber min={0} step={0.01} style={{ width: '100%' }} />
        </Form.Item>

        <Form.Item name="costCacheRead" label={t('openclaw.providers.costCacheRead')}>
          <InputNumber min={0} step={0.01} style={{ width: '100%' }} />
        </Form.Item>

        <Form.Item name="costCacheWrite" label={t('openclaw.providers.costCacheWrite')}>
          <InputNumber min={0} step={0.01} style={{ width: '100%' }} />
        </Form.Item>
      </Form>
    </Modal>
  );
};

export default OpenClawModelFormModal;
