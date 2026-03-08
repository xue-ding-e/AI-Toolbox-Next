import React from 'react';
import { Alert, Button, Form, Input, Modal } from 'antd';
import { useTranslation } from 'react-i18next';
import MarkdownEditor from '@/components/common/MarkdownEditor';
import type { OpenCodePromptConfig } from '@/types/openCodePrompt';
import styles from './OpenCodePromptSettings.module.less';

export interface OpenCodePromptConfigFormValues {
  name: string;
  content: string;
}

interface OpenCodePromptConfigModalProps {
  open: boolean;
  initialValues?: Partial<OpenCodePromptConfig>;
  onCancel: () => void;
  onSuccess: (values: OpenCodePromptConfigFormValues) => Promise<void> | void;
}

const OpenCodePromptConfigModal: React.FC<OpenCodePromptConfigModalProps> = ({
  open,
  initialValues,
  onCancel,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm<OpenCodePromptConfigFormValues>();
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
      title={initialValues?.id ? t('opencode.prompt.editConfig') : t('opencode.prompt.addConfig')}
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
            message={t('opencode.prompt.localConfigHint')}
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
            label={t('opencode.prompt.name')}
            name="name"
            rules={[
              { required: true, message: t('opencode.prompt.nameRequired') },
              { max: 100, message: t('opencode.prompt.nameTooLong') },
            ]}
          >
            <Input placeholder={t('opencode.prompt.namePlaceholder')} />
          </Form.Item>
          <Form.Item
            label={t('opencode.prompt.content')}
            name="content"
            rules={[{ required: true, message: t('opencode.prompt.contentRequired') }]}
          >
            <MarkdownEditor
              height={320}
              minHeight={220}
              maxHeight={520}
              resizable
              placeholder={t('opencode.prompt.contentPlaceholder')}
            />
          </Form.Item>
        </Form>
      </div>
    </Modal>
  );
};

export default OpenCodePromptConfigModal;
