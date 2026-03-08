import React from 'react';
import { Alert, Button, Form, Input, Modal } from 'antd';
import { useTranslation } from 'react-i18next';
import MarkdownEditor from '@/components/common/MarkdownEditor';
import type { GlobalPromptConfig } from '@/types/globalPrompt';
import styles from './GlobalPromptSettings.module.less';

export interface GlobalPromptConfigFormValues {
  name: string;
  content: string;
}

interface GlobalPromptConfigModalProps {
  open: boolean;
  translationKeyPrefix: string;
  initialValues?: Partial<GlobalPromptConfig>;
  onCancel: () => void;
  onSuccess: (values: GlobalPromptConfigFormValues) => Promise<void> | void;
}

const GlobalPromptConfigModal: React.FC<GlobalPromptConfigModalProps> = ({
  open,
  translationKeyPrefix,
  initialValues,
  onCancel,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm<GlobalPromptConfigFormValues>();
  const [saving, setSaving] = React.useState(false);

  React.useEffect(() => {
    if (!open) {
      return;
    }

    form.setFieldsValue({
      name: initialValues?.name || '',
      content: initialValues?.content || '',
    });
  }, [form, initialValues, open]);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setSaving(true);
      await onSuccess(values);
      form.resetFields();
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      title={initialValues?.id ? t(`${translationKeyPrefix}.editConfig`) : t(`${translationKeyPrefix}.addConfig`)}
      open={open}
      onCancel={onCancel}
      width={920}
      footer={[
        <Button key="cancel" onClick={onCancel}>
          {t('common.cancel')}
        </Button>,
        <Button key="submit" type="primary" loading={saving} onClick={handleSubmit}>
          {t('common.save')}
        </Button>,
      ]}
    >
      <div className={styles.modalBody}>
        {initialValues?.id === '__local__' && (
          <Alert
            message={t(`${translationKeyPrefix}.localConfigHint`)}
            type="warning"
            showIcon
            style={{ marginBottom: 16 }}
          />
        )}
        <Form
          form={form}
          layout="horizontal"
          labelCol={{ span: 2 }}
          wrapperCol={{ span: 22 }}
        >
          <Form.Item
            label={t(`${translationKeyPrefix}.name`)}
            name="name"
            rules={[
              { required: true, message: t(`${translationKeyPrefix}.nameRequired`) },
              { max: 100, message: t(`${translationKeyPrefix}.nameTooLong`) },
            ]}
          >
            <Input placeholder={t(`${translationKeyPrefix}.namePlaceholder`)} />
          </Form.Item>
          <Form.Item
            label={t(`${translationKeyPrefix}.content`)}
            name="content"
            rules={[{ required: true, message: t(`${translationKeyPrefix}.contentRequired`) }]}
          >
            <MarkdownEditor
              height={320}
              minHeight={220}
              maxHeight={520}
              resizable
              placeholder={t(`${translationKeyPrefix}.contentPlaceholder`)}
            />
          </Form.Item>
        </Form>
      </div>
    </Modal>
  );
};

export default GlobalPromptConfigModal;
