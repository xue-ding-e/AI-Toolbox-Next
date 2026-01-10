import React from 'react';
import { Modal, Form, Input, Button, Typography, Select, Collapse, Space } from 'antd';
import { MoreOutlined } from '@ant-design/icons';
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
  
  // Track which agents have advanced settings expanded
  const [expandedAgents, setExpandedAgents] = React.useState<Record<string, boolean>>({});
  
  // Store advanced settings values in refs to avoid re-renders
  const advancedSettingsRef = React.useRef<Record<string, Record<string, unknown>>>({});
  const otherFieldsRef = React.useRef<Record<string, unknown>>({});
  
  // Use refs for validation state to avoid re-renders during editing
  const otherFieldsValidRef = React.useRef(true);
  const advancedSettingsValidRef = React.useRef<Record<string, boolean>>({});

  const labelCol = 4;
  const wrapperCol = 20;

  // Initialize form values
  React.useEffect(() => {
    if (open) {
      if (initialValues) {
        // Parse agent models and advanced settings from config
        const agentFields: Record<string, string | undefined> = {};
        const validityState: Record<string, boolean> = {};
        
        AGENT_TYPES.forEach((agentType) => {
          const agent = initialValues.agents[agentType];
          if (agent) {
            // Extract model
            if (agent.model) {
              agentFields[`agent_${agentType}`] = agent.model;
            }
            
            // Extract advanced fields (everything except model) and store in ref
            const advancedConfig: Record<string, unknown> = {};
            Object.keys(agent).forEach((key) => {
              if (key !== 'model' && agent[key as keyof OhMyOpenCodeAgentConfig] !== undefined) {
                advancedConfig[key] = agent[key as keyof OhMyOpenCodeAgentConfig];
              }
            });
            
            advancedSettingsRef.current[agentType] = advancedConfig;
          }
          
          // Initialize validity state
          validityState[agentType] = true;
        });

        form.setFieldsValue({
          id: initialValues.id,
          name: initialValues.name,
          ...agentFields,
          otherFields: initialValues.otherFields || {},
        });
        
        otherFieldsRef.current = initialValues.otherFields || {};
        advancedSettingsValidRef.current = validityState;
      } else {
        form.resetFields();
        form.setFieldsValue({
          otherFields: {},
        });
        
        // Reset validity state
        const validityState: Record<string, boolean> = {};
        AGENT_TYPES.forEach((agentType) => {
          validityState[agentType] = true;
          advancedSettingsRef.current[agentType] = {};
        });
        advancedSettingsValidRef.current = validityState;
        otherFieldsRef.current = {};
      }
      otherFieldsValidRef.current = true;
      setExpandedAgents({}); // Collapse all on open
    }
  }, [open, initialValues, form]);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setLoading(true);

      // Validate JSON fields
      if (!otherFieldsValidRef.current) {
        setLoading(false);
        return;
      }
      
      // Check all advanced settings are valid
      const hasInvalidAdvancedSettings = Object.values(advancedSettingsValidRef.current).some(valid => !valid);
      if (hasInvalidAdvancedSettings) {
        setLoading(false);
        return;
      }

      // Build agents object with merged advanced settings
      const agents: Record<string, OhMyOpenCodeAgentConfig | undefined> = {};
      AGENT_TYPES.forEach((agentType) => {
        const modelFieldName = `agent_${agentType}` as keyof typeof values;
        
        const modelValue = values[modelFieldName];
        const advancedValue = advancedSettingsRef.current[agentType];
        
        // Only create agent config if model is set OR advanced settings exist
        if (modelValue || (advancedValue && Object.keys(advancedValue).length > 0)) {
          agents[agentType] = {
            ...(modelValue ? { model: modelValue } : {}),
            ...(advancedValue || {}),
          } as OhMyOpenCodeAgentConfig;
        } else {
          agents[agentType] = undefined;
        }
      });

      const result: OhMyOpenCodeConfigFormValues = {
        name: values.name,
        agents,
        otherFields: otherFieldsRef.current && Object.keys(otherFieldsRef.current).length > 0 ? otherFieldsRef.current : undefined,
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
  
  const toggleAdvancedSettings = (agentType: string) => {
    setExpandedAgents(prev => ({
      ...prev,
      [agentType]: !prev[agentType],
    }));
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
          <Collapse
            defaultActiveKey={['agents']}
            ghost
            items={[
              {
                key: 'agents',
                label: (
                  <Text strong>{t('opencode.ohMyOpenCode.agentModels')}</Text>
                ),
                children: (
                  <>
                    <Text type="secondary" style={{ display: 'block', fontSize: 12, marginBottom: 12 }}>
                      {t('opencode.ohMyOpenCode.agentModelsHint')}
                    </Text>
                    {AGENT_TYPES.map((agentType) => (
                      <div key={agentType}>
                        <Form.Item
                          label={getAgentDisplayName(agentType).split(' ')[0]}
                          tooltip={getAgentDescription(agentType)}
                          style={{ marginBottom: expandedAgents[agentType] ? 8 : 12 }}
                        >
                          <Space.Compact style={{ width: '100%' }}>
                            <Form.Item name={`agent_${agentType}`} noStyle>
                              <Select
                                placeholder={t('opencode.ohMyOpenCode.selectModel')}
                                options={modelOptions}
                                allowClear
                                showSearch
                                optionFilterProp="label"
                                style={{ width: 'calc(100% - 32px)' }}
                              />
                            </Form.Item>
                            <Button
                              icon={<MoreOutlined />}
                              onClick={() => toggleAdvancedSettings(agentType)}
                              type={expandedAgents[agentType] ? 'primary' : 'default'}
                              title={t('opencode.ohMyOpenCode.advancedSettings')}
                            />
                          </Space.Compact>
                        </Form.Item>
                        
                        {expandedAgents[agentType] && (
                          <Form.Item
                            help={t('opencode.ohMyOpenCode.advancedSettingsHint')}
                            labelCol={{ span: 24 }}
                            wrapperCol={{ span: 24 }}
                            style={{ marginBottom: 16, marginLeft: labelCol * 4 + 8 }}
                          >
                            <JsonEditor
                              value={advancedSettingsRef.current[agentType] || {}}
                              onChange={(value, isValid) => {
                                advancedSettingsValidRef.current[agentType] = isValid;
                                if (isValid && typeof value === 'object' && value !== null) {
                                  advancedSettingsRef.current[agentType] = value as Record<string, unknown>;
                                }
                              }}
                              height={150}
                              minHeight={100}
                              maxHeight={300}
                              resizable
                              mode="text"
                            />
                          </Form.Item>
                        )}
                      </div>
                    ))}
                  </>
                ),
              },
            ]}
          />

          <Collapse
            defaultActiveKey={[]}
            style={{ marginTop: 8 }}
            ghost
            items={[
              {
                key: 'other',
                label: <Text strong>{t('opencode.ohMyOpenCode.otherFields')}</Text>,
                children: (
                  <Form.Item
                    help={t('opencode.ohMyOpenCode.otherFieldsHint')}
                    labelCol={{ span: 24 }}
                    wrapperCol={{ span: 24 }}
                  >
                    <JsonEditor
                      value={otherFieldsRef.current || {}}
                      onChange={(value, isValid) => {
                        otherFieldsValidRef.current = isValid;
                        if (isValid && typeof value === 'object' && value !== null) {
                          otherFieldsRef.current = value as Record<string, unknown>;
                        }
                      }}
                      height={200}
                      minHeight={150}
                      maxHeight={400}
                      resizable
                      mode="text"
                    />
                  </Form.Item>
                ),
              },
            ]}
          />
        </div>
      </Form>
    </Modal>
  );
};

export default OhMyOpenCodeConfigModal;
