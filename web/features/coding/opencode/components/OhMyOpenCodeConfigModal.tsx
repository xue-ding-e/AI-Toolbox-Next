import React from 'react';
import { Modal, Form, Input, Button, Typography, Select, Divider } from 'antd';
import { useTranslation } from 'react-i18next';
import type { OhMyOpenCodeConfig, OhMyOpenCodeAgentConfig, OhMyOpenCodeAgentType } from '@/types/ohMyOpenCode';
import { getAgentDisplayName, getAgentDescription } from '@/services/ohMyOpenCodeApi';
import JsonEditor from '@/components/common/JsonEditor';

const { Text } = Typography;

interface OhMyOpenCodeConfigModalProps {
  open: boolean;
  isEdit: boolean;
  initialValues?: OhMyOpenCodeConfig;
  modelOptions: { label: string; value: string }[];
  onCancel: () => void;
  onSuccess: (values: OhMyOpenCodeConfigFormValues) => void;
}

export interface OhMyOpenCodeConfigFormValues {
  id?: string; // Optional - only present when editing
  name: string;
  agents: Record<string, OhMyOpenCodeAgentConfig | undefined>;
  otherFields?: Record<string, unknown>;
}

// Default agent types
const AGENT_TYPES: OhMyOpenCodeAgentType[] = [
  'Sisyphus',
  'oracle',
  'librarian',
  'explore',
  'frontend-ui-ux-engineer',
  'document-writer',
  'multimodal-looker',
];

const OhMyOpenCodeConfigModal: React.FC<OhMyOpenCodeConfigModalProps> = ({
  open,
  isEdit,
  initialValues,
  modelOptions,
  onCancel,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  const [loading, setLoading] = React.useState(false);
  const [otherFieldsValid, setOtherFieldsValid] = React.useState(true);

  const labelCol = 4;
  const wrapperCol = 20;

  // Initialize form values
  React.useEffect(() => {
    if (open) {
      if (initialValues) {
        // Parse agent models from config
        const agentFields: Record<string, string | undefined> = {};
        AGENT_TYPES.forEach((agentType) => {
          const agent = initialValues.agents[agentType];
          if (agent?.model) {
            agentFields[`agent_${agentType}`] = agent.model;
          }
        });

        form.setFieldsValue({
          id: initialValues.id,
          name: initialValues.name,
          ...agentFields,
          otherFields: initialValues.otherFields || {},
        });
      } else {
        form.resetFields();
        form.setFieldsValue({
          otherFields: {},
        });
      }
      setOtherFieldsValid(true);
    }
  }, [open, initialValues, form]);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setLoading(true);

      // Validate JSON fields
      if (!otherFieldsValid) {
        setLoading(false);
        return;
      }

      // Build agents object
      const agents: Record<string, OhMyOpenCodeAgentConfig | undefined> = {};
      AGENT_TYPES.forEach((agentType) => {
        const modelFieldName = `agent_${agentType}` as keyof typeof values;
        const modelValue = values[modelFieldName];
        agents[agentType] = modelValue ? { model: modelValue } : undefined;
      });

      const result: OhMyOpenCodeConfigFormValues = {
        name: values.name,
        agents,
        otherFields: values.otherFields && Object.keys(values.otherFields).length > 0 ? values.otherFields : undefined,
      };

      // Include id when editing (read from form values which were set from initialValues)
      if (isEdit && values.id) {
        result.id = values.id;
      }

      onSuccess(result);
      form.resetFields();
    } catch (error) {
      console.error('Form validation error:', error);
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      title={isEdit 
        ? t('opencode.ohMyOpenCode.editConfig') 
        : t('opencode.ohMyOpenCode.addConfig')}
      open={open}
      onCancel={onCancel}
      footer={[
        <Button key="cancel" onClick={onCancel}>
          {t('common.cancel')}
        </Button>,
        <Button key="submit" type="primary" loading={loading} onClick={handleSubmit}>
          {t('common.save')}
        </Button>,
      ]}
      width={800}
    >
      <Form
        form={form}
        layout="horizontal"
        labelCol={{ span: labelCol }}
        wrapperCol={{ span: wrapperCol }}
        style={{ marginTop: 24 }}
      >
        {/* Hidden ID field for editing */}
        <Form.Item name="id" hidden>
          <Input />
        </Form.Item>

        <Form.Item
          label={t('opencode.ohMyOpenCode.configName')}
          name="name"
          rules={[{ required: true, message: t('opencode.ohMyOpenCode.configNamePlaceholder') }]}
        >
          <Input 
            placeholder={t('opencode.ohMyOpenCode.configNamePlaceholder')}
          />
        </Form.Item>

        <div style={{ maxHeight: 400, overflowY: 'auto', paddingRight: 8, marginTop: 16 }}>
          <Text type="secondary" style={{ display: 'block', marginBottom: 16 }}>
            {t('opencode.ohMyOpenCode.agentModelsHint')}
          </Text>
          {AGENT_TYPES.map((agentType) => (
            <Form.Item
              key={agentType}
              label={getAgentDisplayName(agentType).split(' ')[0]}
              name={`agent_${agentType}`}
              extra={
                <Text type="secondary" style={{ fontSize: 11 }}>
                  {getAgentDescription(agentType)}
                </Text>
              }
            >
              <Select
                placeholder={t('opencode.ohMyOpenCode.selectModel')}
                options={modelOptions}
                allowClear
                showSearch
                optionFilterProp="label"
                style={{ width: '100%' }}
              />
            </Form.Item>
          ))}

          <Divider style={{ marginTop: 32, marginBottom: 24 }} />

          <Text strong style={{ display: 'block', marginBottom: 16 }}>
            {t('opencode.ohMyOpenCode.otherFields')}
          </Text>
          
          <Form.Item
            name="otherFields"
            validateStatus={!otherFieldsValid ? 'error' : undefined}
            help={!otherFieldsValid ? t('opencode.ohMyOpenCode.invalidJson') : t('opencode.ohMyOpenCode.otherFieldsHint')}
            labelCol={{ span: 24 }}
            wrapperCol={{ span: 24 }}
          >
            <JsonEditor
              value={form.getFieldValue('otherFields') || {}}
              onChange={(value, isValid) => {
                setOtherFieldsValid(isValid);
                if (isValid && typeof value === 'object' && value !== null) {
                  form.setFieldValue('otherFields', value);
                }
              }}
              height={200}
              minHeight={150}
              maxHeight={400}
              resizable
              mode="text"
            />
          </Form.Item>
        </div>
      </Form>
    </Modal>
  );
};

export default OhMyOpenCodeConfigModal;
