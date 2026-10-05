import React from 'react';
import { Alert, Form, Input, Modal, Tabs, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import JsonEditor from '@/components/common/JsonEditor';
import {
  OMO_NATIVE_AGENTS,
  OMO_NATIVE_CATEGORIES,
  type OmoNativeAgentsConfig,
} from '@/types/omoNative';

const { Text } = Typography;

export interface OmoNativeConfigFormValues {
  name: string;
  agents: Record<string, unknown> | null;
  categories: Record<string, unknown> | null;
  modelProfiles: Record<string, unknown> | null;
  modelProfile: string;
  task: Record<string, unknown> | null;
  otherFields: Record<string, unknown> | null;
}

interface OmoNativeConfigModalProps {
  open: boolean;
  isEdit: boolean;
  isLocal?: boolean;
  initialValues?: OmoNativeAgentsConfig | null;
  onCancel: () => void;
  onSuccess: (values: OmoNativeConfigFormValues) => Promise<void> | void;
}

/** 空对象视为「未设置」，写回时用 null 表示不接管该键。 */
const orNull = (value: unknown): Record<string, unknown> | null => {
  if (value === undefined || value === null) return null;
  if (typeof value !== 'object' || Array.isArray(value)) return null;
  return Object.keys(value as Record<string, unknown>).length === 0
    ? null
    : (value as Record<string, unknown>);
};

const OmoNativeConfigModal: React.FC<OmoNativeConfigModalProps> = ({
  open,
  isEdit,
  isLocal = false,
  initialValues,
  onCancel,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm<OmoNativeConfigFormValues>();
  const [saving, setSaving] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (!open) return;
    setError(null);
    form.setFieldsValue({
      name: initialValues?.name ?? '',
      agents: (initialValues?.agents ?? null) as Record<string, unknown> | null,
      categories: (initialValues?.categories ?? null) as Record<string, unknown> | null,
      modelProfiles: (initialValues?.modelProfiles ?? null) as Record<string, unknown> | null,
      modelProfile: initialValues?.modelProfile ?? '',
      task: (initialValues?.task ?? null) as Record<string, unknown> | null,
      otherFields: (initialValues?.otherFields ?? null) as Record<string, unknown> | null,
    });
  }, [open, initialValues, form]);

  const handleOk = async () => {
    let values: OmoNativeConfigFormValues;
    try {
      values = await form.validateFields();
    } catch {
      return;
    }

    setSaving(true);
    setError(null);
    try {
      await onSuccess({
        name: values.name,
        agents: orNull(values.agents),
        categories: orNull(values.categories),
        modelProfiles: orNull(values.modelProfiles),
        modelProfile: values.modelProfile ?? '',
        task: orNull(values.task),
        otherFields: orNull(values.otherFields),
      });
    } catch (submitError) {
      setError((submitError as Error).message);
    } finally {
      setSaving(false);
    }
  };

  const jsonField = (
    name: keyof OmoNativeConfigFormValues,
    label: string,
    hint: string,
  ) => (
    <div>
      <Form.Item label={label} name={name} style={{ marginBottom: 4 }}>
        <JsonEditor height={220} />
      </Form.Item>
      <Text type="secondary" style={{ fontSize: 12, display: 'block', marginBottom: 12 }}>
        {hint}
      </Text>
    </div>
  );

  return (
    <Modal
      open={open}
      title={isEdit ? t('omoNative.modal.editTitle') : t('omoNative.modal.addTitle')}
      width={900}
      onCancel={onCancel}
      onOk={handleOk}
      confirmLoading={saving}
      destroyOnHidden
    >
      {isLocal && (
        <Alert
          type="info"
          showIcon
          style={{ marginBottom: 16 }}
          message={t('omoNative.modal.localNotice')}
        />
      )}
      {error && <Alert type="error" showIcon style={{ marginBottom: 16 }} message={error} />}
      <Form form={form} layout="vertical">
        <Form.Item
          label={t('omoNative.modal.name')}
          name="name"
          rules={[{ required: true, message: t('omoNative.modal.nameRequired') }]}
        >
          <Input placeholder={t('omoNative.modal.namePlaceholder')} />
        </Form.Item>

        <Tabs
          items={[
            {
              key: 'agents',
              label: t('omoNative.modal.tabAgents'),
              children: jsonField(
                'agents',
                t('omoNative.modal.agents'),
                t('omoNative.modal.agentsHint', { names: OMO_NATIVE_AGENTS.join(', ') }),
              ),
            },
            {
              key: 'categories',
              label: t('omoNative.modal.tabCategories'),
              children: jsonField(
                'categories',
                t('omoNative.modal.categories'),
                t('omoNative.modal.categoriesHint', { names: OMO_NATIVE_CATEGORIES.join(', ') }),
              ),
            },
            {
              key: 'modelProfiles',
              label: t('omoNative.modal.tabModelProfiles'),
              children: (
                <div>
                  <Form.Item label={t('omoNative.modal.modelProfile')} name="modelProfile">
                    <Input
                      placeholder={t('omoNative.modal.modelProfilePlaceholder')}
                      allowClear
                    />
                  </Form.Item>
                  <Text
                    type="secondary"
                    style={{ fontSize: 12, display: 'block', marginBottom: 12 }}
                  >
                    {t('omoNative.modal.modelProfileHint')}
                  </Text>
                  {jsonField(
                    'modelProfiles',
                    t('omoNative.modal.modelProfiles'),
                    t('omoNative.modal.modelProfilesHint'),
                  )}
                </div>
              ),
            },
            {
              key: 'task',
              label: t('omoNative.modal.tabTask'),
              children: jsonField(
                'task',
                t('omoNative.modal.task'),
                t('omoNative.modal.taskHint'),
              ),
            },
            {
              key: 'other',
              label: t('omoNative.modal.tabOther'),
              children: jsonField(
                'otherFields',
                t('omoNative.modal.otherFields'),
                t('omoNative.modal.otherFieldsHint'),
              ),
            },
          ]}
        />
      </Form>
    </Modal>
  );
};

export default OmoNativeConfigModal;
