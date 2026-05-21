import React from 'react';
import { Button, Form, Input, Modal, message } from 'antd';
import { Plus, Save } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { upsertModelPricing, type ModelPricing } from '@/services';
import styles from './ModelPricingModal.module.less';

interface ModelPricingEditModalProps {
  open: boolean;
  pricing: ModelPricing | null;
  isNew: boolean;
  onClose: () => void;
  onSaved: (pricing: ModelPricing) => void;
}

interface ModelPricingFormValues {
  model_id: string;
  display_name: string;
  input_cost_per_million: string;
  output_cost_per_million: string;
  cache_read_cost_per_million: string;
  cache_creation_cost_per_million: string;
}

const costPattern = /^\d+(?:\.\d+)?$/;

const defaultPricing: ModelPricing = {
  model_id: '',
  display_name: '',
  input_cost_per_million: '0',
  output_cost_per_million: '0',
  cache_read_cost_per_million: '0',
  cache_creation_cost_per_million: '0',
};

const toFormValues = (pricing: ModelPricing): ModelPricingFormValues => ({
  model_id: pricing.model_id,
  display_name: pricing.display_name,
  input_cost_per_million: pricing.input_cost_per_million,
  output_cost_per_million: pricing.output_cost_per_million,
  cache_read_cost_per_million: pricing.cache_read_cost_per_million,
  cache_creation_cost_per_million: pricing.cache_creation_cost_per_million,
});

const trimFormValues = (values: ModelPricingFormValues): ModelPricing => ({
  model_id: values.model_id.trim(),
  display_name: values.display_name.trim(),
  input_cost_per_million: values.input_cost_per_million.trim(),
  output_cost_per_million: values.output_cost_per_million.trim(),
  cache_read_cost_per_million: values.cache_read_cost_per_million.trim(),
  cache_creation_cost_per_million: values.cache_creation_cost_per_million.trim(),
});

const ModelPricingEditModal: React.FC<ModelPricingEditModalProps> = ({
  open,
  pricing,
  isNew,
  onClose,
  onSaved,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm<ModelPricingFormValues>();
  const [saving, setSaving] = React.useState(false);

  React.useEffect(() => {
    if (!open) {
      return;
    }
    form.setFieldsValue(toFormValues(pricing ?? defaultPricing));
  }, [form, open, pricing]);

  const validateCost = React.useCallback(
    (_: unknown, value?: string) => {
      const trimmedValue = typeof value === 'string' ? value.trim() : '';
      if (!trimmedValue || !costPattern.test(trimmedValue)) {
        return Promise.reject(new Error(t('gateway.page.pricing.invalidCost')));
      }
      return Promise.resolve();
    },
    [t],
  );

  const handleSubmit = React.useCallback(async () => {
    const values = await form.validateFields();
    const nextPricing = trimFormValues(values);
    setSaving(true);
    try {
      const savedPricing = await upsertModelPricing({
        ...nextPricing,
        model_id: isNew ? nextPricing.model_id : pricing?.model_id ?? nextPricing.model_id,
      });
      message.success(
        isNew
          ? t('gateway.page.pricing.pricingAdded')
          : t('gateway.page.pricing.pricingUpdated'),
      );
      onSaved(savedPricing);
      onClose();
    } catch (error) {
      message.error(
        t('gateway.page.pricing.saveModelFailed', {
          error: error instanceof Error ? error.message : String(error),
        }),
      );
    } finally {
      setSaving(false);
    }
  }, [form, isNew, onClose, onSaved, pricing?.model_id, t]);

  return (
    <Modal
      open={open}
      title={
        isNew
          ? t('gateway.page.pricing.addPricing')
          : t('gateway.page.pricing.editPricing')
      }
      width={560}
      className={styles.editModal}
      onCancel={onClose}
      footer={[
        <Button key="cancel" onClick={onClose}>
          {t('common.cancel')}
        </Button>,
        <Button
          key="save"
          type="primary"
          icon={isNew ? <Plus size={14} /> : <Save size={14} />}
          loading={saving}
          onClick={() => void handleSubmit()}
        >
          {isNew ? t('common.add') : t('common.save')}
        </Button>,
      ]}
    >
      <Form
        form={form}
        layout="horizontal"
        labelCol={{ span: 8 }}
        wrapperCol={{ span: 16 }}
        className={styles.editForm}
      >
        <Form.Item
          name="model_id"
          label={t('gateway.page.pricing.modelId')}
          rules={[{ required: true, message: t('gateway.page.pricing.modelIdRequired') }]}
        >
          <Input
            disabled={!isNew}
            placeholder={t('gateway.page.pricing.modelIdPlaceholder')}
          />
        </Form.Item>
        <Form.Item
          name="display_name"
          label={t('gateway.page.pricing.displayName')}
          rules={[{ required: true, message: t('gateway.page.pricing.displayNameRequired') }]}
        >
          <Input placeholder={t('gateway.page.pricing.displayNamePlaceholder')} />
        </Form.Item>
        <Form.Item
          name="input_cost_per_million"
          label={t('gateway.page.pricing.inputCost')}
          rules={[{ validator: validateCost }]}
        >
          <Input inputMode="decimal" />
        </Form.Item>
        <Form.Item
          name="output_cost_per_million"
          label={t('gateway.page.pricing.outputCost')}
          rules={[{ validator: validateCost }]}
        >
          <Input inputMode="decimal" />
        </Form.Item>
        <Form.Item
          name="cache_read_cost_per_million"
          label={t('gateway.page.pricing.cacheReadCost')}
          rules={[{ validator: validateCost }]}
        >
          <Input inputMode="decimal" />
        </Form.Item>
        <Form.Item
          name="cache_creation_cost_per_million"
          label={t('gateway.page.pricing.cacheCreationCost')}
          rules={[{ validator: validateCost }]}
        >
          <Input inputMode="decimal" />
        </Form.Item>
      </Form>
    </Modal>
  );
};

export default ModelPricingEditModal;
