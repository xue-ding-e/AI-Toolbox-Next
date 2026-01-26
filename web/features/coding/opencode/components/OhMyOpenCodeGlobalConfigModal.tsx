import React from 'react';
import { Modal, Form, Button, Typography, Select, Collapse, Input } from 'antd';
import { ReloadOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import JsonEditor from '@/components/common/JsonEditor';

const { Text } = Typography;

const DEFAULT_SCHEMA = 'https://raw.githubusercontent.com/code-yeongyu/oh-my-opencode/master/assets/oh-my-opencode.schema.json';

// Helper function to check if value is empty object
const isEmptyObject = (value: unknown): boolean => {
  return value !== null && typeof value === 'object' && !Array.isArray(value) && Object.keys(value as Record<string, unknown>).length === 0;
};

// Return undefined if value is empty object, otherwise return value
const emptyToUndefined = (value: unknown): unknown => {
  return isEmptyObject(value) ? undefined : value;
};

interface OhMyOpenCodeGlobalConfigModalProps {
  open: boolean;
  initialValues?: {
    schema?: string;
    sisyphusAgent?: Record<string, unknown> | null;
    disabledAgents?: string[];
    disabledMcps?: string[];
    disabledHooks?: string[];
    disabledSkills?: string[];
    lsp?: Record<string, unknown> | null;
    experimental?: Record<string, unknown> | null;
    backgroundTask?: Record<string, unknown> | null;
    browserAutomationEngine?: Record<string, unknown> | null;
    claudeCode?: Record<string, unknown> | null;
    otherFields?: Record<string, unknown>;
  };
  onCancel: () => void;
  onSuccess: (values: {
    schema: string;
    sisyphusAgent: Record<string, unknown> | null;
    disabledAgents: string[];
    disabledMcps: string[];
    disabledHooks: string[];
    disabledSkills: string[];
    lsp?: Record<string, unknown> | null;
    experimental?: Record<string, unknown> | null;
    backgroundTask?: Record<string, unknown> | null;
    browserAutomationEngine?: Record<string, unknown> | null;
    claudeCode?: Record<string, unknown> | null;
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
  const backgroundTaskValidRef = React.useRef(true);
  const browserAutomationEngineValidRef = React.useRef(true);
  const claudeCodeValidRef = React.useRef(true);
  const otherFieldsValidRef = React.useRef(true);

  const labelCol = 4;
  const wrapperCol = 20;

  // Initialize form values
  React.useEffect(() => {
    if (open) {
      if (initialValues) {
        form.setFieldsValue({
          schema: initialValues.schema || DEFAULT_SCHEMA,
          sisyphusAgent: isEmptyObject(initialValues.sisyphusAgent) ? undefined : initialValues.sisyphusAgent,
          disabledAgents: initialValues.disabledAgents || [],
          disabledMcps: initialValues.disabledMcps || [],
          disabledHooks: initialValues.disabledHooks || [],
          disabledSkills: initialValues.disabledSkills || [],
          lsp: isEmptyObject(initialValues.lsp) ? undefined : initialValues.lsp,
          experimental: isEmptyObject(initialValues.experimental) ? undefined : initialValues.experimental,
          backgroundTask: isEmptyObject(initialValues.backgroundTask) ? undefined : initialValues.backgroundTask,
          browserAutomationEngine: isEmptyObject(initialValues.browserAutomationEngine) ? undefined : initialValues.browserAutomationEngine,
          claudeCode: isEmptyObject(initialValues.claudeCode) ? undefined : initialValues.claudeCode,
          otherFields: isEmptyObject(initialValues.otherFields) ? undefined : initialValues.otherFields,
        });
      } else {
        form.resetFields();
        // Set default values
        form.setFieldsValue({
          schema: DEFAULT_SCHEMA,
          sisyphusAgent: undefined,
          disabledAgents: [],
          disabledMcps: [],
          disabledHooks: [],
          disabledSkills: [],
          lsp: undefined,
          experimental: undefined,
          backgroundTask: undefined,
          browserAutomationEngine: undefined,
          claudeCode: undefined,
          otherFields: undefined,
        });
      }
      sisyphusJsonValidRef.current = true;
      lspJsonValidRef.current = true;
      experimentalJsonValidRef.current = true;
      backgroundTaskValidRef.current = true;
      browserAutomationEngineValidRef.current = true;
      claudeCodeValidRef.current = true;
      otherFieldsValidRef.current = true;
    }
  }, [open, initialValues, form]);

  const handleSubmit = async () => {
    try {
      setLoading(true);

      // Validate JSON fields
      if (!sisyphusJsonValidRef.current || !lspJsonValidRef.current || !experimentalJsonValidRef.current || 
          !backgroundTaskValidRef.current || !browserAutomationEngineValidRef.current || !claudeCodeValidRef.current || 
          !otherFieldsValidRef.current) {
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
        disabledSkills: allValues.disabledSkills || [],
        lsp: allValues.lsp || null,
        experimental: allValues.experimental || null,
        backgroundTask: allValues.backgroundTask || null,
        browserAutomationEngine: allValues.browserAutomationEngine || null,
        claudeCode: allValues.claudeCode || null,
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
                      value={emptyToUndefined(form.getFieldValue('sisyphusAgent'))}
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
                      placeholder={`{
    "disabled": false,
    "default_builder_enabled": false,
    "planner_enabled": true,
    "replace_plan": true
}`}
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
                          { value: 'Prometheus (Planner)', label: 'Prometheus (Planner)' },
                          { value: 'Atlas', label: 'Atlas' },
                          { value: 'oracle', label: 'Oracle' },
                          { value: 'librarian', label: 'Librarian' },
                          { value: 'explore', label: 'Explore' },
                          { value: 'multimodal-looker', label: 'Multimodal Looker' },
                          { value: 'frontend-ui-ux-engineer', label: 'Frontend UI/UX Engineer' },
                          { value: 'document-writer', label: 'Document Writer' },
                          { value: 'Sisyphus-Junior', label: 'Sisyphus-Junior' },
                          { value: 'Metis (Plan Consultant)', label: 'Metis (Plan Consultant)' },
                          { value: 'Momus (Plan Reviewer)', label: 'Momus (Plan Reviewer)' },
                          { value: 'OpenCode-Builder', label: 'OpenCode-Builder' },
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
                        options={[
                          { value: 'todo-continuation-enforcer', label: 'todo-continuation-enforcer' },
                          { value: 'context-window-monitor', label: 'context-window-monitor' },
                          { value: 'session-recovery', label: 'session-recovery' },
                          { value: 'session-notification', label: 'session-notification' },
                          { value: 'comment-checker', label: 'comment-checker' },
                          { value: 'grep-output-truncator', label: 'grep-output-truncator' },
                          { value: 'tool-output-truncator', label: 'tool-output-truncator' },
                          { value: 'directory-agents-injector', label: 'directory-agents-injector' },
                          { value: 'directory-readme-injector', label: 'directory-readme-injector' },
                          { value: 'empty-task-response-detector', label: 'empty-task-response-detector' },
                          { value: 'think-mode', label: 'think-mode' },
                          { value: 'anthropic-context-window-limit-recovery', label: 'anthropic-context-window-limit-recovery' },
                          { value: 'rules-injector', label: 'rules-injector' },
                          { value: 'background-notification', label: 'background-notification' },
                          { value: 'auto-update-checker', label: 'auto-update-checker' },
                          { value: 'startup-toast', label: 'startup-toast' },
                          { value: 'keyword-detector', label: 'keyword-detector' },
                          { value: 'agent-usage-reminder', label: 'agent-usage-reminder' },
                          { value: 'non-interactive-env', label: 'non-interactive-env' },
                          { value: 'interactive-bash-session', label: 'interactive-bash-session' },
                          { value: 'compaction-context-injector', label: 'compaction-context-injector' },
                          { value: 'thinking-block-validator', label: 'thinking-block-validator' },
                          { value: 'claude-code-hooks', label: 'claude-code-hooks' },
                          { value: 'ralph-loop', label: 'ralph-loop' },
                          { value: 'preemptive-compaction', label: 'preemptive-compaction' },
                        ]}
                      />
                    </Form.Item>

                    <Form.Item
                      label={t('opencode.ohMyOpenCode.disabledSkills')}
                      name="disabledSkills"
                      style={{ marginBottom: 12 }}
                    >
                      <Select
                        mode="tags"
                        placeholder={t('opencode.ohMyOpenCode.disabledSkillsPlaceholder')}
                        options={[
                          { value: 'playwright', label: 'playwright' },
                          { value: 'agent-browser', label: 'agent-browser' },
                          { value: 'git-master', label: 'git-master' },
                        ]}
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
                      value={emptyToUndefined(form.getFieldValue('lsp'))}
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
                      placeholder={`{
    "lsp": {
        "typescript-language-server": {
            "command": ["typescript-language-server", "--stdio"],
            "extensions": [".ts", ".tsx"],
            "priority": 10
        },
        "pylsp": {
            "disabled": true
        }
    }
}`}
                    />
                  </Form.Item>
                ),
              },
              {
                key: 'claudeCode',
                label: <Text strong>{t('opencode.ohMyOpenCode.claudeCodeSettings') || 'Claude Code'}</Text>,
                children: (
                  <Form.Item
                    name="claudeCode"
                    help={t('opencode.ohMyOpenCode.claudeCodeHint') || 'Configure Claude Code integration features'}
                    labelCol={{ span: 24 }}
                    wrapperCol={{ span: 24 }}
                  >
                    <JsonEditor
                      value={emptyToUndefined(form.getFieldValue('claudeCode'))}
                      onChange={(value, isValid) => {
                        claudeCodeValidRef.current = isValid;
                        if (isValid && typeof value === 'object' && value !== null) {
                          form.setFieldValue('claudeCode', value);
                        }
                      }}
                      height={200}
                      minHeight={150}
                      maxHeight={350}
                      resizable
                      mode="text"
                      placeholder={`{
    "mcp": true,
    "commands": true,
    "skills": true,
    "agents": true,
    "hooks": true,
    "plugins": true
}`}
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
                      value={emptyToUndefined(form.getFieldValue('experimental'))}
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
                      placeholder={`{
    "experimental": {
        "truncate_all_tool_outputs": true,
        "aggressive_truncation": true,
        "auto_resume": true
    }
}`}
                    />
                  </Form.Item>
                ),
              },
              {
                key: 'backgroundTask',
                label: <Text strong>{t('opencode.ohMyOpenCode.backgroundTaskSettings') || 'Background Task'}</Text>,
                children: (
                  <Form.Item
                    name="backgroundTask"
                    help={t('opencode.ohMyOpenCode.backgroundTaskHint') || 'Configure background task concurrency settings'}
                    labelCol={{ span: 24 }}
                    wrapperCol={{ span: 24 }}
                  >
                    <JsonEditor
                      value={emptyToUndefined(form.getFieldValue('backgroundTask'))}
                      onChange={(value, isValid) => {
                        backgroundTaskValidRef.current = isValid;
                        if (isValid && typeof value === 'object' && value !== null) {
                          form.setFieldValue('backgroundTask', value);
                        }
                      }}
                      height={250}
                      minHeight={150}
                      maxHeight={400}
                      resizable
                      mode="text"
                      placeholder={`{
    "defaultConcurrency": 5,
    "providerConcurrency": { "anthropic": 3, "openai": 5, "google": 10 },
    "modelConcurrency": { "anthropic/claude-opus-4-5": 2, "google/gemini-3-flash": 10 }
}`}
                    />
                  </Form.Item>
                ),
              },
              {
                key: 'browserAutomation',
                label: <Text strong>{t('opencode.ohMyOpenCode.browserAutomationSettings') || 'Browser Automation'}</Text>,
                children: (
                  <Form.Item
                    name="browserAutomationEngine"
                    help={t('opencode.ohMyOpenCode.browserAutomationHint') || 'Configure browser automation engine'}
                    labelCol={{ span: 24 }}
                    wrapperCol={{ span: 24 }}
                  >
                    <JsonEditor
                      value={emptyToUndefined(form.getFieldValue('browserAutomationEngine'))}
                      onChange={(value, isValid) => {
                        browserAutomationEngineValidRef.current = isValid;
                        if (isValid && typeof value === 'object' && value !== null) {
                          form.setFieldValue('browserAutomationEngine', value);
                        }
                      }}
                      height={150}
                      minHeight={100}
                      maxHeight={300}
                      resizable
                      mode="text"
                      placeholder={`{ "provider": "playwright" }`}
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
                      value={emptyToUndefined(form.getFieldValue('otherFields'))}
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
                      placeholder={`{
    "background_task": {
        "defaultConcurrency": 5
    }
}`}
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
