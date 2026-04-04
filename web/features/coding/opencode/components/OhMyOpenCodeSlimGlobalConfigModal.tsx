import React from 'react';
import { Alert, Button, Collapse, Form, Modal, Select, message } from 'antd';
import { useTranslation } from 'react-i18next';
import JsonEditor from '@/components/common/JsonEditor';
import { SLIM_AGENT_TYPES, type OhMyOpenCodeSlimGlobalConfig, type OhMyOpenCodeSlimGlobalConfigInput } from '@/types/ohMyOpenCodeSlim';
import OhMyOpenCodeSlimCouncilForm, {
  buildSlimCouncilConfig,
  parseSlimCouncilFormValues,
  type SlimCouncilModelOption,
} from './OhMyOpenCodeSlimCouncilForm';
import styles from './OhMyOpenCodeSlimGlobalConfigModal.module.less';

interface OhMyOpenCodeSlimGlobalConfigModalProps {
  open: boolean;
  initialConfig?: OhMyOpenCodeSlimGlobalConfig;
  isLocal?: boolean;
  modelOptions: SlimCouncilModelOption[];
  modelVariantsMap?: Record<string, string[]>;
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
  modelOptions,
  modelVariantsMap = {},
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
  const councilOtherFieldsValidRef = React.useRef(true);

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
      ...parseSlimCouncilFormValues(initialConfig?.council ?? null),
    });

    sisyphusValidRef.current = true;
    lspValidRef.current = true;
    experimentalValidRef.current = true;
    otherFieldsValidRef.current = true;
    councilOtherFieldsValidRef.current = true;
  }, [open, initialConfig, form]);

  const handleSave = async () => {
    if (
      !sisyphusValidRef.current ||
      !lspValidRef.current ||
      !experimentalValidRef.current ||
      !otherFieldsValidRef.current ||
      !councilOtherFieldsValidRef.current
    ) {
      message.error(t('opencode.ohMyOpenCode.invalidJson'));
      return;
    }

    setLoading(true);
    try {
      await form.validateFields();
      const values = form.getFieldsValue(true) as Record<string, unknown>;
      const councilBuildResult = buildSlimCouncilConfig(values, t);
      if (councilBuildResult.errorMessage) {
        message.error(councilBuildResult.errorMessage);
        return;
      }

      const input: OhMyOpenCodeSlimGlobalConfigInput = {
        sisyphusAgent: asObject(values.sisyphusAgent),
        disabledAgents: asStringArray(values.disabledAgents),
        disabledMcps: asStringArray(values.disabledMcps),
        disabledHooks: asStringArray(values.disabledHooks),
        lsp: asObject(values.lsp),
        experimental: asObject(values.experimental),
        council: councilBuildResult.council,
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

  const buildSectionLabel = (title: string, hint?: string) => (
    <div className={styles.sectionLabel}>
      <div className={styles.sectionLabelMain}>
        <span className={styles.sectionTitle}>{title}</span>
      </div>
      {hint ? <span className={styles.sectionHint}>{hint}</span> : null}
    </div>
  );

  return (
    <Modal
      className={styles.modal}
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
      <div className={styles.content}>
        {isLocal && (
          <Alert
            className={styles.alert}
            message={t('opencode.ohMyOpenCode.localConfigHint')}
            type="warning"
            showIcon
          />
        )}

        <Form
          className={styles.form}
          form={form}
          layout="horizontal"
          labelCol={{ span: 6 }}
          wrapperCol={{ span: 18 }}
        >
          <div className={styles.scrollArea}>
            <Collapse
              className={styles.sectionCollapse}
              defaultActiveKey={['disabled']}
              bordered={false}
              items={[
                {
                  key: 'disabled',
                  label: buildSectionLabel(t('opencode.ohMyOpenCode.disabledItems'), t('opencode.ohMyOpenCode.disabledAgentsPlaceholder')),
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
              ]}
            />

            <Collapse
              className={styles.sectionCollapse}
              bordered={false}
              items={[
                {
                  key: 'sisyphus',
                  label: buildSectionLabel(t('opencode.ohMyOpenCode.sisyphusSettings')),
                  children: (
                    <Form.Item
                      className={styles.editorItem}
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
              ]}
            />

            <OhMyOpenCodeSlimCouncilForm
              form={form}
              modelOptions={modelOptions}
              modelVariantsMap={modelVariantsMap}
              councilOtherFieldsValidRef={councilOtherFieldsValidRef}
            />

            <Collapse
              className={styles.sectionCollapse}
              bordered={false}
              items={[
                {
                  key: 'lsp',
                  label: buildSectionLabel(t('opencode.ohMyOpenCode.lspSettings'), t('opencode.ohMyOpenCode.lspConfigHint')),
                  children: (
                    <Form.Item
                      className={styles.editorItem}
                      name="lsp"
                      labelCol={{ span: 24 }}
                      wrapperCol={{ span: 24 }}
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
              ]}
            />

            <Collapse
              className={styles.sectionCollapse}
              bordered={false}
              items={[
                {
                  key: 'experimental',
                  label: buildSectionLabel(t('opencode.ohMyOpenCode.experimentalSettings'), t('opencode.ohMyOpenCode.experimentalConfigHint')),
                  children: (
                    <Form.Item
                      className={styles.editorItem}
                      name="experimental"
                      labelCol={{ span: 24 }}
                      wrapperCol={{ span: 24 }}
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
              ]}
            />

            <Collapse
              className={styles.sectionCollapse}
              bordered={false}
              items={[
                {
                  key: 'other',
                  label: buildSectionLabel(t('opencode.ohMyOpenCodeSlim.otherFields'), t('opencode.ohMyOpenCodeSlim.otherFieldsHint')),
                  children: (
                    <Form.Item
                      className={styles.editorItem}
                      name="otherFields"
                      labelCol={{ span: 24 }}
                      wrapperCol={{ span: 24 }}
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
      </div>
    </Modal>
  );
};

export default OhMyOpenCodeSlimGlobalConfigModal;
