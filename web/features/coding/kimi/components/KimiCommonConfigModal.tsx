import React, { useEffect } from 'react';
import { Form, Modal, message } from 'antd';
import { useTranslation } from 'react-i18next';
import TomlEditor from '@/components/common/TomlEditor';
import type { KimiCommonConfig, KimiCommonConfigInput } from '@/types/kimi';
import { buildKimiCommonConfigSubmitValues } from '../utils/commonConfigForm';

interface KimiCommonConfigModalProps {
  open: boolean;
  config: KimiCommonConfig | null;
  onCancel: () => void;
  onSubmit: (config: KimiCommonConfigInput) => Promise<void>;
}

const KimiCommonConfigModal: React.FC<KimiCommonConfigModalProps> = ({
  open,
  config,
  onCancel,
  onSubmit,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm<KimiCommonConfigInput>();
  const [submitting, setSubmitting] = React.useState(false);

  useEffect(() => {
    if (!open) {
      return;
    }
    form.setFieldsValue({ config: config?.config ?? '' });
  }, [config, form, open]);

  const handleOk = async () => {
    try {
      const values = await form.validateFields();
      setSubmitting(true);
      // Only the TOML payload is submitted here; the root directory keeps its
      // single editing entry in RootDirectoryModal (see commonConfigForm.ts).
      await onSubmit(buildKimiCommonConfigSubmitValues(values.config));
    } catch (error) {
      // Tauri invoke rejections are plain strings, not Error instances.
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Modal
      open={open}
      title={t('kimi.commonConfig.title')}
      onOk={() => void handleOk()}
      onCancel={onCancel}
      confirmLoading={submitting}
      width={800}
      destroyOnHidden
    >
      <Form form={form} layout="vertical">
        <Form.Item name="config">
          <TomlEditorFormItem placeholder={t('kimi.commonConfig.description')} />
        </Form.Item>
      </Form>
    </Modal>
  );
};

// TomlEditor 与 antd Form.Item 集成的包装组件（TomlEditor 的 value 是必填 prop，
// Form.Item 在运行时注入 value/onChange，但 TS 静态类型无法感知，需要桥接）。
const TomlEditorFormItem: React.FC<{
  value?: string;
  onChange?: (value: string) => void;
  placeholder?: string;
}> = ({ value = '', onChange, placeholder }) => (
  <TomlEditor
    value={value}
    onChange={onChange}
    height={300}
    placeholder={placeholder}
  />
);

export default KimiCommonConfigModal;
