import React from 'react';
import { Modal, Form, Button, Typography, Switch, Select, Collapse } from 'antd';
import { useTranslation } from 'react-i18next';
import type { OhMyOpenCodeGlobalConfig, OhMyOpenCodeSisyphusConfig, OhMyOpenCodeLspServer, OhMyOpenCodeExperimental } from '@/types/ohMyOpenCode';
import JsonEditor from '@/components/common/JsonEditor';

const { Text } = Typography;

interface OhMyOpenCodeGlobalConfigModalProps {
  open: boolean;
  initialValues?: OhMyOpenCodeGlobalConfig;
  onCancel: () => void;
  onSuccess: (values: OhMyOpenCodeGlobalConfigFormValues) => void;
}

export interface OhMyOpenCodeGlobalConfigFormValues {
  sisyphusAgent?: OhMyOpenCodeSisyphusConfig;
  disabledAgents?: string[];
  disabledMcps?: string[];
  disabledHooks?: string[];
  lsp?: Record<string, OhMyOpenCodeLspServer>;
  experimental?: OhMyOpenCodeExperimental;
  otherFields?: Record<string, unknown>;
}

const OhMyOpenCodeGlobalConfigModal: React.FC<OhMyOpenCodeGlobalConfigModalProps> = ({
  open,
  initialValues,
  onCancel,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  const [loading, setLoading] = React.useState(false);
  const [lspJsonValid, setLspJsonValid] = React.useState(true);
  const [experimentalJsonValid, setExperimentalJsonValid] = React.useState(true);
  const [otherFieldsValid, setOtherFieldsValid] = React.useState(true);

  const labelCol = 5;
  const wrapperCol = 19;

  // Initialize form values
  React.useEffect(() => {
    if (open) {
      if (initialValues) {
        form.setFieldsValue({
          sisyphusAgent: initialValues.sisyphusAgent || {
            disabled: false,
            default_builder_enabled: false,
            planner_enabled: true,
            replace_plan: true,
          },
          disabledAgents: initialValues.disabledAgents || [],
          disabledMcps: initialValues.disabledMcps || [],
          disabledHooks: initialValues.disabledHooks || [],
          lsp: initialValues.lsp || {},
          experimental: initialValues.experimental || {},
          otherFields: initialValues.otherFields || {},
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
          lsp: {},
          experimental: {},
          otherFields: {},
        });
      }
      setLspJsonValid(true);
      setExperimentalJsonValid(true);
      setOtherFieldsValid(true);
    }
  }, [open, initialValues, form]);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      setLoading(true);

      // Validate JSON fields
      if (!lspJsonValid || !experimentalJsonValid || !otherFieldsValid) {
        setLoading(false);
        return;
      }

      const result: OhMyOpenCodeGlobalConfigFormValues = {
        sisyphusAgent: values.sisyphusAgent || {},
        disabledAgents: values.disabledAgents || [],
        disabledMcps: values.disabledMcps || [],
        disabledHooks: values.disabledHooks || [],
        lsp: values.lsp && Object.keys(values.lsp).length > 0 ? values.lsp : undefined,
        experimental: values.experimental && Object.keys(values.experimental).length > 0 ? values.experimental : undefined,
        otherFields: values.otherFields && Object.keys(values.otherFields).length > 0 ? values.otherFields : undefined,
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
      title={t('opencode.ohMyOpenCode.globalConfigTitle')}
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
      width={900}
    >
      <Form
        form={form}
        layout="horizontal"
        labelCol={{ span: labelCol }}
        wrapperCol={{ span: wrapperCol }}
        style={{ marginTop: 24 }}
      >
        <div style={{ maxHeight: 600, overflowY: 'auto', paddingRight: 8 }}>
          <Collapse
            defaultActiveKey={['sisyphus', 'disabled', 'other']}
            bordered={false}
            style={{ background: 'transparent' }}
            items={[
              {
                key: 'sisyphus',
                label: <Text strong>{t('opencode.ohMyOpenCode.sisyphusSettings')}</Text>,
                children: (
                  <>
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
                  </>
                ),
              },
              {
                key: 'disabled',
                label: <Text strong>{t('opencode.ohMyOpenCode.disabledItems')}</Text>,
                children: (
                  <>
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
                      <Select
                        mode="tags"
                        placeholder={t('opencode.ohMyOpenCode.disabledMcpsPlaceholder')}
                        options={[
                          { value: 'context7', label: 'context7' },
                          { value: 'grep_app', label: 'grep_app' },
                          { value: 'websearch', label: 'websearch' },
                        ]}
                      />
                    </Form.Item>

                    <Form.Item
                      label={t('opencode.ohMyOpenCode.disabledHooks')}
                      name="disabledHooks"
                    >
                      <Select
                        mode="tags"
                        placeholder={t('opencode.ohMyOpenCode.disabledHooksPlaceholder')}
                      />
                    </Form.Item>
                  </>
                ),
              },
              {
                key: 'lsp',
                label: <Text strong>{t('opencode.ohMyOpenCode.lspSettings')}</Text>,
                children: (
                  <Form.Item
                    label={t('opencode.ohMyOpenCode.lspConfig')}
                    name="lsp"
                    validateStatus={!lspJsonValid ? 'error' : undefined}
                    help={!lspJsonValid ? t('opencode.ohMyOpenCode.invalidJson') : t('opencode.ohMyOpenCode.lspConfigHint')}
                    labelCol={{ span: 24 }}
                    wrapperCol={{ span: 24 }}
                  >
                    <JsonEditor
                      value={form.getFieldValue('lsp') || {}}
                      onChange={(value, isValid) => {
                        setLspJsonValid(isValid);
                        if (isValid && typeof value === 'object' && value !== null) {
                          form.setFieldValue('lsp', value);
                        }
                      }}
                      height={250}
                      minHeight={150}
                      maxHeight={400}
                      resizable
                      mode="text"
                    />
                  </Form.Item>
                ),
              },
              {
                key: 'experimental',
                label: <Text strong>{t('opencode.ohMyOpenCode.experimentalSettings')}</Text>,
                children: (
                  <Form.Item
                    label={t('opencode.ohMyOpenCode.experimentalConfig')}
                    name="experimental"
                    validateStatus={!experimentalJsonValid ? 'error' : undefined}
                    help={!experimentalJsonValid ? t('opencode.ohMyOpenCode.invalidJson') : t('opencode.ohMyOpenCode.experimentalConfigHint')}
                    labelCol={{ span: 24 }}
                    wrapperCol={{ span: 24 }}
                  >
                    <JsonEditor
                      value={form.getFieldValue('experimental') || {}}
                      onChange={(value, isValid) => {
                        setExperimentalJsonValid(isValid);
                        if (isValid && typeof value === 'object' && value !== null) {
                          form.setFieldValue('experimental', value);
                        }
                      }}
                      height={250}
                      minHeight={150}
                      maxHeight={400}
                      resizable
                      mode="text"
                    />
                  </Form.Item>
                ),
              },
              {
                key: 'other',
                label: <Text strong>{t('opencode.ohMyOpenCode.otherFields')}</Text>,
                children: (
                  <Form.Item
                    label={t('opencode.ohMyOpenCode.otherFields')}
                    name="otherFields"
                    validateStatus={!otherFieldsValid ? 'error' : undefined}
                    help={!otherFieldsValid ? t('opencode.ohMyOpenCode.invalidJson') : t('opencode.ohMyOpenCode.otherFieldsGlobalHint')}
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
                      height={250}
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

export default OhMyOpenCodeGlobalConfigModal;

