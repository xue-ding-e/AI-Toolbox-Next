import React from 'react';
import { Modal, Form, Button, Typography, Select, Collapse, Input } from 'antd';
import { ReloadOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import JsonEditor from '@/components/common/JsonEditor';

const { Text } = Typography;

const DEFAULT_SCHEMA = 'https://raw.githubusercontent.com/code-yeongyu/oh-my-opencode/master/assets/oh-my-opencode.schema.json';
const DEFAULT_SISYPHUS_AGENT = {
  disabled: false,
  default_builder_enabled: false,
  planner_enabled: true,
  replace_plan: true,
};

interface OhMyOpenCodeGlobalConfigModalProps {
  open: boolean;
  initialValues?: {
    schema?: string;
    sisyphusAgent?: Record<string, unknown> | null;
    disabledAgents?: string[];
    disabledMcps?: string[];
    disabledHooks?: string[];
    lsp?: Record<string, unknown> | null;
    experimental?: Record<string, unknown> | null;
    otherFields?: Record<string, unknown>;
  };
  onCancel: () => void;
  onSuccess: (values: {
    schema: string;
    sisyphusAgent: Record<string, unknown> | null;
    disabledAgents: string[];
    disabledMcps: string[];
    disabledHooks: string[];
    lsp?: Record<string, unknown> | null;
    experimental?: Record<string, unknown> | null;
    otherFields?: Record<string, unknown>;
  }) => void;
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

  // Use refs for validation state to avoid re-renders during editing
  const sisyphusJsonValidRef = React.useRef(true);
  const lspJsonValidRef = React.useRef(true);
  const experimentalJsonValidRef = React.useRef(true);
  const otherFieldsValidRef = React.useRef(true);

  const labelCol = 4;
  const wrapperCol = 20;

  // Initialize form values
  React.useEffect(() => {
    if (open) {
      if (initialValues) {
        form.setFieldsValue({
          schema: initialValues.schema || DEFAULT_SCHEMA,
          sisyphusAgent: initialValues.sisyphusAgent || DEFAULT_SISYPHUS_AGENT,
          disabledAgents: initialValues.disabledAgents || [],
          disabledMcps: initialValues.disabledMcps || [],
          disabledHooks: initialValues.disabledHooks || [],
          lsp: initialValues.lsp || {},
          experimental: initialValues.experimental || {},
          otherFields: initialValues.otherFields || {},
        });
      } else {
        form.resetFields();
        // Set default values
        form.setFieldsValue({
          schema: DEFAULT_SCHEMA,
          sisyphusAgent: DEFAULT_SISYPHUS_AGENT,
          disabledAgents: [],
          disabledMcps: [],
          disabledHooks: [],
          lsp: {},
          experimental: {},
          otherFields: {},
        });
      }
      sisyphusJsonValidRef.current = true;
      lspJsonValidRef.current = true;
      experimentalJsonValidRef.current = true;
      otherFieldsValidRef.current = true;
    }
  }, [open, initialValues, form]);

  const handleSubmit = async () => {
    try {
      setLoading(true);

      // Validate JSON fields
      if (!sisyphusJsonValidRef.current || !lspJsonValidRef.current || !experimentalJsonValidRef.current || !otherFieldsValidRef.current) {
        setLoading(false);
        return;
      }

      const allValues = form.getFieldsValue(true) || {};

      const result = {
        schema: allValues.schema || DEFAULT_SCHEMA,
        sisyphusAgent: allValues.sisyphusAgent || null,
        disabledAgents: allValues.disabledAgents || [],
        disabledMcps: allValues.disabledMcps || [],
        disabledHooks: allValues.disabledHooks || [],
        lsp: allValues.lsp || null,
        experimental: allValues.experimental || null,
        otherFields: allValues.otherFields || null,
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
          {/* Schema 设置 */}
          <Form.Item
            label="$schema"
            name="schema"
            style={{ marginBottom: 16 }}
          >
            <Input
              placeholder="https://raw.githubusercontent.com/..."
              addonAfter={
                <Button
                  type="text"
                  size="small"
                  icon={<ReloadOutlined />}
                  onClick={() => form.setFieldValue('schema', DEFAULT_SCHEMA)}
                  style={{ margin: -4, padding: '0 4px' }}
                />
              }
            />
          </Form.Item>

          <Collapse
            defaultActiveKey={['disabled']}
            bordered={false}
            style={{ background: 'transparent' }}
            items={[
              {
                key: 'sisyphus',
                label: <Text strong>{t('opencode.ohMyOpenCode.sisyphusSettings')}</Text>,
                children: (
                  <Form.Item
                    name="sisyphusAgent"
                    help="Sisyphus agent configuration in JSON format"
                    labelCol={{ span: 24 }}
                    wrapperCol={{ span: 24 }}
                  >
                    <JsonEditor
                      value={form.getFieldValue('sisyphusAgent') || {}}
                      onChange={(value, isValid) => {
                        sisyphusJsonValidRef.current = isValid;
                        if (isValid && typeof value === 'object' && value !== null) {
                          form.setFieldValue('sisyphusAgent', value);
                        }
                      }}
                      height={200}
                      minHeight={120}
                      maxHeight={300}
                      resizable
                      mode="text"
                    />
                  </Form.Item>
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
                      style={{ marginBottom: 12 }}
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
                      style={{ marginBottom: 12 }}
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
                      style={{ marginBottom: 12 }}
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
                    name="lsp"
                    help={t('opencode.ohMyOpenCode.lspConfigHint')}
                    labelCol={{ span: 24 }}
                    wrapperCol={{ span: 24 }}
                  >
                    <JsonEditor
                      value={form.getFieldValue('lsp') || {}}
                      onChange={(value, isValid) => {
                        lspJsonValidRef.current = isValid;
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
                    name="experimental"
                    help={t('opencode.ohMyOpenCode.experimentalConfigHint')}
                    labelCol={{ span: 24 }}
                    wrapperCol={{ span: 24 }}
                  >
                    <JsonEditor
                      value={form.getFieldValue('experimental') || {}}
                      onChange={(value, isValid) => {
                        experimentalJsonValidRef.current = isValid;
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
                    name="otherFields"
                    help={t('opencode.ohMyOpenCode.otherFieldsGlobalHint')}
                    labelCol={{ span: 24 }}
                    wrapperCol={{ span: 24 }}
                  >
                    <JsonEditor
                      value={form.getFieldValue('otherFields') || {}}
                      onChange={(value, isValid) => {
                        otherFieldsValidRef.current = isValid;
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
