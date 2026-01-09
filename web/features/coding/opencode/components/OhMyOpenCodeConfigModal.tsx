import React from 'react';
import { Modal, Form, Input, Button, Typography, Switch, Select } from 'antd';
import { RightOutlined, DownOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { OhMyOpenCodeConfig, OhMyOpenCodeAgentConfig, OhMyOpenCodeSisyphusConfig, OhMyOpenCodeAgentType } from '@/types/ohMyOpenCode';
import { getAgentDisplayName, getAgentDescription } from '@/services/ohMyOpenCodeApi';

const { Text } = Typography;

interface OhMyOpenCodeConfigModalProps {
  open: boolean;
  isEdit: boolean;
  initialValues?: OhMyOpenCodeConfig;
  existingIds?: string[];
  modelOptions: { label: string; value: string }[];
  onCancel: () => void;
  onSuccess: (values: OhMyOpenCodeConfigFormValues) => void;
  onDuplicateId?: (id: string) => void;
}

export interface OhMyOpenCodeConfigFormValues {
  id: string;
  name: string;
  agents: Record<string, OhMyOpenCodeAgentConfig | undefined>;
  sisyphusAgent: OhMyOpenCodeSisyphusConfig;
  disabledAgents: string[];
  disabledMcps: string[];
  disabledHooks: string[];
  disabledSkills: string[];
  disabledCommands: string[];
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
  existingIds = [],
  modelOptions,
  onCancel,
  onSuccess,
  onDuplicateId,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  const [loading, setLoading] = React.useState(false);
  const [advancedExpanded, setAdvancedExpanded] = React.useState(false);

  const labelCol = 6;
  const wrapperCol = 18;

  // Initialize form values
  React.useEffect(() => {
    if (open) {
      if (initialValues) {
        form.setFieldsValue({
          id: initialValues.id,
          name: initialValues.name,
          sisyphusAgent: initialValues.sisyphusAgent,
        });
      } else {
        form.resetFields();
        // Set default sisyphus agent values
        form.setFieldsValue({
          sisyphusAgent: {
            disabled: false,
            default_builder_enabled: false,
            planner_enabled: true,
            replace_plan: true,
          },
        });
      }
    }
  }, [open, initialValues, form]);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setLoading(true);

      // Check for duplicate ID when creating
      if (!isEdit && existingIds.includes(values.id)) {
        if (onDuplicateId) {
          onDuplicateId(values.id);
        }
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
        id: values.id,
        name: values.name,
        agents,
        sisyphusAgent: values.sisyphusAgent || {},
        disabledAgents: values.disabledAgents || [],
        disabledMcps: values.disabledMcps || [],
        disabledHooks: values.disabledHooks || [],
        disabledSkills: values.disabledSkills || [],
        disabledCommands: values.disabledCommands || [],
      };

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
      width={700}
    >
      <Form
        form={form}
        layout="horizontal"
        labelCol={{ span: labelCol }}
        wrapperCol={{ span: wrapperCol }}
        style={{ marginTop: 24 }}
      >
        <Form.Item
          label={t('opencode.ohMyOpenCode.configName')}
          name="name"
          rules={[{ required: true, message: t('opencode.ohMyOpenCode.configNamePlaceholder') }]}
        >
          <Input 
            placeholder={t('opencode.ohMyOpenCode.configNamePlaceholder')}
            disabled={isEdit}
          />
        </Form.Item>

        {!isEdit && (
          <Form.Item
            label={t('opencode.ohMyOpenCode.configId')}
            name="id"
            rules={[{ required: true, message: t('opencode.ohMyOpenCode.configIdPlaceholder') }]}
            extra={t('opencode.ohMyOpenCode.configIdHint')}
          >
            <Input placeholder="omo_config_xxx" />
          </Form.Item>
        )}

        <Text strong style={{ display: 'block', marginBottom: 16 }}>
          {t('opencode.ohMyOpenCode.agentModels')}
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

        <div style={{ marginBottom: advancedExpanded ? 16 : 0 }}>
          <Button
            type="link"
            onClick={() => setAdvancedExpanded(!advancedExpanded)}
            style={{ padding: 0, height: 'auto' }}
          >
            {advancedExpanded ? <DownOutlined /> : <RightOutlined />}
            <span style={{ marginLeft: 4 }}>{t('common.advancedSettings')}</span>
          </Button>
        </div>

        {advancedExpanded && (
          <>
            <Text strong style={{ display: 'block', marginBottom: 16 }}>
              {t('opencode.ohMyOpenCode.sisyphusSettings')}
            </Text>

            <Form.Item
              label={t('opencode.ohMyOpenCode.sisyphusDisabled')}
              name={['sisyphusAgent', 'disabled']}
              valuePropName="checked"
            >
              <Switch />
            </Form.Item>

            <Form.Item
              label={t('opencode.ohMyOpenCode.defaultBuilderEnabled')}
              name={['sisyphusAgent', 'default_builder_enabled']}
              valuePropName="checked"
            >
              <Switch />
            </Form.Item>

            <Form.Item
              label={t('opencode.ohMyOpenCode.plannerEnabled')}
              name={['sisyphusAgent', 'planner_enabled']}
              valuePropName="checked"
            >
              <Switch />
            </Form.Item>

            <Form.Item
              label={t('opencode.ohMyOpenCode.replacePlan')}
              name={['sisyphusAgent', 'replace_plan']}
              valuePropName="checked"
            >
              <Switch />
            </Form.Item>

            <Text strong style={{ display: 'block', marginBottom: 16, marginTop: 24 }}>
              {t('opencode.ohMyOpenCode.disabledItems')}
            </Text>

            <Form.Item
              label={t('opencode.ohMyOpenCode.disabledAgents')}
              name="disabledAgents"
            >
              <Select
                mode="tags"
                placeholder={t('opencode.ohMyOpenCode.disabledAgentsPlaceholder')}
                options={[
                  { value: 'oracle', label: 'Oracle' },
                  { value: 'librarian', label: 'Librarian' },
                  { value: 'explore', label: 'Explore' },
                  { value: 'frontend-ui-ux-engineer', label: 'Frontend UI/UX Engineer' },
                  { value: 'document-writer', label: 'Document Writer' },
                  { value: 'multimodal-looker', label: 'Multimodal Looker' },
                ]}
              />
            </Form.Item>

            <Form.Item
              label={t('opencode.ohMyOpenCode.disabledMcps')}
              name="disabledMcps"
            >
              <Input.TextArea 
                placeholder={t('opencode.ohMyOpenCode.disabledMcpsPlaceholder')}
                rows={2}
              />
            </Form.Item>

            <Form.Item
              label={t('opencode.ohMyOpenCode.disabledHooks')}
              name="disabledHooks"
            >
              <Input.TextArea 
                placeholder={t('opencode.ohMyOpenCode.disabledHooksPlaceholder')}
                rows={2}
              />
            </Form.Item>
          </>
        )}
      </Form>
    </Modal>
  );
};

export default OhMyOpenCodeConfigModal;
