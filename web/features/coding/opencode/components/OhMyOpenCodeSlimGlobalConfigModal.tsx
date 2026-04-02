import React from 'react';
import { Alert, Button, Collapse, Form, Modal, Select, Typography, message } from 'antd';
import { useTranslation } from 'react-i18next';
import JsonEditor from '@/components/common/JsonEditor';
import { SLIM_AGENT_TYPES, type OhMyOpenCodeSlimGlobalConfig, type OhMyOpenCodeSlimGlobalConfigInput } from '@/types/ohMyOpenCodeSlim';

const { Text } = Typography;

interface OhMyOpenCodeSlimGlobalConfigModalProps {
  open: boolean;
  initialConfig?: OhMyOpenCodeSlimGlobalConfig;
  isLocal?: boolean;
  onCancel: () => void;
  onSuccess: (values: OhMyOpenCodeSlimGlobalConfigInput) => void;
}

const DISABLED_MCP_OPTIONS = [
  { value: 'context7', label: 'context7' },
  { value: 'grep_app', label: 'grep_app' },
  { value: 'websearch', label: 'websearch' },
];

const emptyToUndefined = (value: unknown): unknown => {
  if (value === null || value === undefined) {
    return undefined;
  }

  if (typeof value === 'object' && !Array.isArray(value) && Object.keys(value as Record<string, unknown>).length === 0) {
    return undefined;
  }

  return value;
};

const asObject = (value: unknown): Record<string, unknown> | null => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return null;
  }

  return value as Record<string, unknown>;
};

const asStringArray = (value: unknown): string[] => {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.filter((item): item is string => typeof item === 'string' && item.trim() !== '');
};

const OhMyOpenCodeSlimGlobalConfigModal: React.FC<OhMyOpenCodeSlimGlobalConfigModalProps> = ({
  open,
  initialConfig,
  isLocal = false,
  onCancel,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  const [loading, setLoading] = React.useState(false);

  const sisyphusValidRef = React.useRef(true);
  const lspValidRef = React.useRef(true);
  const experimentalValidRef = React.useRef(true);
  const otherFieldsValidRef = React.useRef(true);

  React.useEffect(() => {
    if (!open) {
      form.resetFields();
      return;
    }

    form.setFieldsValue({
      sisyphusAgent: emptyToUndefined(initialConfig?.sisyphusAgent),
      disabledAgents: initialConfig?.disabledAgents ?? [],
      disabledMcps: initialConfig?.disabledMcps ?? [],
      disabledHooks: initialConfig?.disabledHooks ?? [],
      lsp: emptyToUndefined(initialConfig?.lsp),
      experimental: emptyToUndefined(initialConfig?.experimental),
      otherFields: emptyToUndefined(initialConfig?.otherFields),
    });

    sisyphusValidRef.current = true;
    lspValidRef.current = true;
    experimentalValidRef.current = true;
    otherFieldsValidRef.current = true;
  }, [open, initialConfig, form]);

  const handleSave = async () => {
    if (
      !sisyphusValidRef.current ||
      !lspValidRef.current ||
      !experimentalValidRef.current ||
      !otherFieldsValidRef.current
    ) {
      message.error(t('opencode.ohMyOpenCode.invalidJson'));
      return;
    }

    setLoading(true);
    try {
      await form.validateFields();
      const values = form.getFieldsValue(true) as Record<string, unknown>;

      const input: OhMyOpenCodeSlimGlobalConfigInput = {
        sisyphusAgent: asObject(values.sisyphusAgent),
        disabledAgents: asStringArray(values.disabledAgents),
        disabledMcps: asStringArray(values.disabledMcps),
        disabledHooks: asStringArray(values.disabledHooks),
        lsp: asObject(values.lsp),
        experimental: asObject(values.experimental),
        council: initialConfig?.council ?? null,
        otherFields: asObject(values.otherFields),
      };

      onSuccess(input);
    } catch (error) {
      console.error('Failed to save slim global config:', error);
      message.error(t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      title={t('opencode.ohMyOpenCode.globalConfigTitle')}
      open={open}
      onCancel={onCancel}
      width={960}
      footer={[
        <Button key="cancel" onClick={onCancel}>
          {t('common.cancel')}
        </Button>,
        <Button key="save" type="primary" loading={loading} onClick={handleSave}>
          {t('common.save')}
        </Button>,
      ]}
    >
      {isLocal && (
        <Alert
          message={t('opencode.ohMyOpenCode.localConfigHint')}
          type="warning"
          showIcon
          style={{ marginBottom: 16, marginTop: 16 }}
        />
      )}

      <Form
        form={form}
        layout="horizontal"
        labelCol={{ span: 6 }}
        wrapperCol={{ span: 18 }}
        style={{ marginTop: isLocal ? 0 : 24 }}
      >
        <div style={{ maxHeight: 640, overflowY: 'auto', paddingRight: 8 }}>
          <Collapse
            defaultActiveKey={['disabled']}
            bordered={false}
            style={{ background: 'transparent' }}
            items={[
              {
                key: 'disabled',
                label: <Text strong>{t('opencode.ohMyOpenCode.disabledItems')}</Text>,
                children: (
                  <>
                    <Form.Item label={t('opencode.ohMyOpenCode.disabledAgents')} name="disabledAgents">
                      <Select
                        mode="tags"
                        allowClear
                        options={SLIM_AGENT_TYPES.map((agent) => ({
                          value: agent,
                          label: t(`opencode.ohMyOpenCodeSlim.agents.${agent}.name`),
                        }))}
                        placeholder={t('opencode.ohMyOpenCode.disabledAgentsPlaceholder')}
                      />
                    </Form.Item>

                    <Form.Item label={t('opencode.ohMyOpenCode.disabledMcps')} name="disabledMcps">
                      <Select
                        mode="tags"
                        allowClear
                        options={DISABLED_MCP_OPTIONS}
                        placeholder={t('opencode.ohMyOpenCode.disabledMcpsPlaceholder')}
                      />
                    </Form.Item>

                    <Form.Item label={t('opencode.ohMyOpenCode.disabledHooks')} name="disabledHooks">
                      <Select
                        mode="tags"
                        allowClear
                        placeholder={t('opencode.ohMyOpenCode.disabledHooksPlaceholder')}
                      />
                    </Form.Item>
                  </>
                ),
              },
              {
                key: 'sisyphus',
                label: <Text strong>{t('opencode.ohMyOpenCode.sisyphusSettings')}</Text>,
                children: (
                  <Form.Item
                    name="sisyphusAgent"
                    labelCol={{ span: 24 }}
                    wrapperCol={{ span: 24 }}
                  >
                    <JsonEditor
                      value={emptyToUndefined(form.getFieldValue('sisyphusAgent'))}
                      onChange={(value, isValid) => {
                        sisyphusValidRef.current = isValid;
                        if (value === null || value === undefined) {
                          form.setFieldValue('sisyphusAgent', undefined);
                          return;
                        }
                        if (isValid && typeof value === 'object' && value !== null && !Array.isArray(value)) {
                          form.setFieldValue('sisyphusAgent', value);
                        }
                      }}
                      height={180}
                      minHeight={120}
                      maxHeight={260}
                      resizable
                      mode="text"
                      placeholder={`{
  "planner_enabled": true
}`}
                    />
                  </Form.Item>
                ),
              },
              {
                key: 'lsp',
                label: <Text strong>{t('opencode.ohMyOpenCode.lspSettings')}</Text>,
                children: (
                  <Form.Item
                    name="lsp"
                    labelCol={{ span: 24 }}
                    wrapperCol={{ span: 24 }}
                    help={t('opencode.ohMyOpenCode.lspConfigHint')}
                  >
                    <JsonEditor
                      value={emptyToUndefined(form.getFieldValue('lsp'))}
                      onChange={(value, isValid) => {
                        lspValidRef.current = isValid;
                        if (value === null || value === undefined) {
                          form.setFieldValue('lsp', undefined);
                          return;
                        }
                        if (isValid && typeof value === 'object' && value !== null && !Array.isArray(value)) {
                          form.setFieldValue('lsp', value);
                        }
                      }}
                      height={220}
                      minHeight={140}
                      maxHeight={320}
                      resizable
                      mode="text"
                      placeholder={`{
  "typescript-language-server": {
    "command": ["typescript-language-server", "--stdio"]
  }
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
                    labelCol={{ span: 24 }}
                    wrapperCol={{ span: 24 }}
                    help={t('opencode.ohMyOpenCode.experimentalConfigHint')}
                  >
                    <JsonEditor
                      value={emptyToUndefined(form.getFieldValue('experimental'))}
                      onChange={(value, isValid) => {
                        experimentalValidRef.current = isValid;
                        if (value === null || value === undefined) {
                          form.setFieldValue('experimental', undefined);
                          return;
                        }
                        if (isValid && typeof value === 'object' && value !== null && !Array.isArray(value)) {
                          form.setFieldValue('experimental', value);
                        }
                      }}
                      height={180}
                      minHeight={120}
                      maxHeight={260}
                      resizable
                      mode="text"
                      placeholder={`{
  "some_experimental_flag": true
}`}
                    />
                  </Form.Item>
                ),
              },
              {
                key: 'other',
                label: <Text strong>{t('opencode.ohMyOpenCodeSlim.otherFields')}</Text>,
                children: (
                  <Form.Item
                    name="otherFields"
                    labelCol={{ span: 24 }}
                    wrapperCol={{ span: 24 }}
                    help={t('opencode.ohMyOpenCodeSlim.otherFieldsHint')}
                  >
                    <JsonEditor
                      value={emptyToUndefined(form.getFieldValue('otherFields'))}
                      onChange={(value, isValid) => {
                        otherFieldsValidRef.current = isValid;
                        if (value === null || value === undefined) {
                          form.setFieldValue('otherFields', undefined);
                          return;
                        }
                        if (isValid && typeof value === 'object' && value !== null && !Array.isArray(value)) {
                          form.setFieldValue('otherFields', value);
                        }
                      }}
                      height={220}
                      minHeight={140}
                      maxHeight={320}
                      resizable
                      mode="text"
                      placeholder={`{
  "multiplexer": {
    "type": "tmux"
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

export default OhMyOpenCodeSlimGlobalConfigModal;
