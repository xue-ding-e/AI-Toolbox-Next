import React from 'react';
import { Modal, Tabs, Form, Input, Select, Space, Button, Alert, message, Typography } from 'antd';
import { EyeInvisibleOutlined, EyeOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '@/stores';
import type { CodexProvider, CodexProviderFormValues } from '@/types/codex';
import { readOpenCodeConfig } from '@/services/opencodeApi';
import type { OpenCodeModel } from '@/types/opencode';
import TomlEditor from '@/components/common/TomlEditor';
import JsonEditor from '@/components/common/JsonEditor';
import { parse as parseToml } from 'smol-toml';
import { useCodexConfigState } from '../hooks/useCodexConfigState';

const { Text } = Typography;
const { TextArea } = Input;

// JsonEditor 与 antd Form.Item 集成的包装组件
interface JsonEditorFormItemProps {
  value?: Record<string, unknown>;
  onChange?: (value: Record<string, unknown>) => void;
}

// 用于追踪 JSON 是否有效（在提交时验证）
const jsonValidityRef = { current: true };

const JsonEditorFormItem: React.FC<JsonEditorFormItemProps> = ({
  value,
  onChange,
}) => {
  return (
    <JsonEditor
      value={value}
      onChange={(newValue, isValid) => {
        // 记录当前的有效性状态
        jsonValidityRef.current = isValid;

        // 只有当 JSON 有效时才更新表单值
        if (isValid && onChange && typeof newValue === 'object' && newValue !== null) {
          onChange(newValue as Record<string, unknown>);
        }
        // JSON 无效时不调用 onChange，保持编辑器内容不变
      }}
      height={120}
      minHeight={80}
      maxHeight={200}
      resizable={false}
      placeholder={`{
  "OPENAI_API_KEY": "sk-your-api-key-here"
}`}
    />
  );
};

// 验证 JSON 有效性的规则（仅在提交时验证）
const validateJsonRule = (message: string) => ({
  validator: () => {
    if (!jsonValidityRef.current) {
      return Promise.reject(new Error(message));
    }
    return Promise.resolve();
  },
});

// TomlEditor 与 antd Form.Item 集成的包装组件
interface TomlEditorFormItemProps {
  value?: string;
  onChange?: (value: string) => void;
  placeholder?: string;
}

// 用于追踪 TOML 是否有效（在提交时验证）
const tomlValidityRef = { current: true };

const TomlEditorFormItem: React.FC<TomlEditorFormItemProps> = ({
  value = '',
  onChange,
  placeholder,
}) => {
  return (
    <TomlEditor
      value={value}
      onChange={(newValue) => {
        // 验证 TOML 有效性
        try {
          if (newValue.trim()) {
            parseToml(newValue);
          }
          tomlValidityRef.current = true;
        } catch {
          tomlValidityRef.current = false;
        }
        
        // 始终调用 onChange，保持编辑器内容
        if (onChange) {
          onChange(newValue);
        }
      }}
      height={150}
      placeholder={placeholder}
    />
  );
};

// 验证 TOML 有效性的规则（仅在提交时验证）
const validateTomlRule = (message: string) => ({
  validator: () => {
    if (!tomlValidityRef.current) {
      return Promise.reject(new Error(message));
    }
    return Promise.resolve();
  },
});

// OpenCode provider display type
interface OpenCodeProviderDisplay {
  id: string;
  name: string;
  baseUrl: string | undefined;
  apiKey?: string;
  models: { id: string; name: string }[];
}

interface CodexProviderFormModalProps {
  open: boolean;
  provider?: CodexProvider | null;
  isCopy?: boolean;
  defaultTab?: 'manual' | 'import';
  onCancel: () => void;
  onSubmit: (values: CodexProviderFormValues) => Promise<void>;
}

const CodexProviderFormModal: React.FC<CodexProviderFormModalProps> = ({
  open,
  provider,
  isCopy = false,
  defaultTab = 'manual',
  onCancel,
  onSubmit,
}) => {
  const { t } = useTranslation();
  const language = useAppStore((state) => state.language);
  const [form] = Form.useForm();
  const [loading, setLoading] = React.useState(false);
  const [showApiKey, setShowApiKey] = React.useState(false);
  const [activeTab, setActiveTab] = React.useState<'manual' | 'import'>(defaultTab);

  const labelCol = { span: language === 'zh-CN' ? 4 : 6 };
  const wrapperCol = { span: 20 };

  // OpenCode import related state
  const [openCodeProviders, setOpenCodeProviders] = React.useState<OpenCodeProviderDisplay[]>([]);
  const [selectedProvider, setSelectedProvider] = React.useState<OpenCodeProviderDisplay | null>(null);
  const [availableModels, setAvailableModels] = React.useState<{ id: string; name: string }[]>([]);
  const [loadingProviders, setLoadingProviders] = React.useState(false);
  const [processedBaseUrl, setProcessedBaseUrl] = React.useState<string>('');

  const isEdit = !!provider && !isCopy;

  // 用于跟踪 Modal 的打开状态变化
  const prevOpenRef = React.useRef(false);
  const formInitializedRef = React.useRef(false);

  // 使用新的配置状态管理 Hook
  const {
    codexApiKey,
    codexAuth,
    codexBaseUrl,
    codexModel,
    codexConfig,
    isUpdatingApiKeyRef,
    handleApiKeyChange,
    handleAuthChange,
    handleBaseUrlChange,
    handleModelChange,
    handleConfigChange,
    getFinalSettingsConfig,
  } = useCodexConfigState({
    initialData: provider ? { settingsConfig: provider.settingsConfig } : undefined,
    open,
  });

  // When Modal opens, set activeTab and reset form init flag
  React.useEffect(() => {
    if (open && !prevOpenRef.current) {
      // Modal 从关闭变为打开
      setActiveTab(defaultTab);
      formInitializedRef.current = false;
    }
    prevOpenRef.current = open;
  }, [open, defaultTab]);

  // Load OpenCode providers list when import tab is active
  React.useEffect(() => {
    if (open && activeTab === 'import') {
      loadOpenCodeProviders();
    }
  }, [open, activeTab]);

  // Initialize form with Hook state after Hook has been initialized
  // 只在 Modal 刚打开且表单未初始化时执行
  React.useEffect(() => {
    if (!open || formInitializedRef.current) {
      return;
    }

    if (provider) {
      // 编辑模式：等待 Hook 初始化完成后设置表单
      // Hook 初始化完成的标志：apiKey 有值（因为 settingsConfig 肯定有 apiKey）
      // 或者 provider 的 settingsConfig 本身就是空的
      try {
        const parsed = JSON.parse(provider.settingsConfig || '{}');
        const providerHasApiKey = !!parsed.auth?.OPENAI_API_KEY;

        // 如果 provider 有 apiKey 但 Hook 还没初始化，等待
        if (providerHasApiKey && !codexApiKey) {
          return;
        }
      } catch {
        // 解析失败，直接初始化
      }

      form.setFieldsValue({
        name: provider.name,
        apiKey: codexApiKey,
        authJson: codexAuth,
        baseUrl: codexBaseUrl,
        model: codexModel,
        configToml: codexConfig,
        notes: provider.notes || '',
      });
      formInitializedRef.current = true;
    } else {
      // 新建模式：重置表单
      form.resetFields();
      formInitializedRef.current = true;
    }
  }, [open, provider, codexApiKey, codexAuth, codexBaseUrl, codexModel, codexConfig, form]);

  // 同步 Hook 的 codexConfig 到 Form 的 configToml 字段
  // 当用户在 baseUrl 或 model 输入框输入时，需要实时更新 TOML 编辑器
  const prevCodexConfigRef = React.useRef(codexConfig);
  React.useEffect(() => {
    // 只在表单已初始化且 codexConfig 变化时同步
    if (!formInitializedRef.current) return;
    if (prevCodexConfigRef.current === codexConfig) return;
    
    prevCodexConfigRef.current = codexConfig;
    
    // 获取当前表单的 configToml 值
    const currentFormConfig = form.getFieldValue('configToml') || '';
    
    // 只有当 Hook 的值与 Form 的值不同时才更新，避免不必要的更新
    if (currentFormConfig !== codexConfig) {
      form.setFieldsValue({ configToml: codexConfig });
    }
  }, [codexConfig, form]);

  // 同步 Hook 的 codexAuth 到 Form 的 authJson 字段
  // 只在 API Key 输入框变化时同步，避免 JsonEditor 自己的输入导致光标重置
  const prevCodexAuthRef = React.useRef(codexAuth);
  React.useEffect(() => {
    // 只在表单已初始化且 codexAuth 变化时同步
    if (!formInitializedRef.current) return;
    if (JSON.stringify(prevCodexAuthRef.current) === JSON.stringify(codexAuth)) return;
    
    prevCodexAuthRef.current = codexAuth;
    
    // 只有当是 API Key 输入框导致的变化时才同步到 JsonEditor
    // 避免 JsonEditor 自己的输入导致光标重置
    if (isUpdatingApiKeyRef.current) {
      form.setFieldsValue({ authJson: codexAuth });
    }
  }, [codexAuth, form, isUpdatingApiKeyRef]);

  const loadOpenCodeProviders = async () => {
    setLoadingProviders(true);
    try {
      const config = await readOpenCodeConfig();
      if (!config) {
        setOpenCodeProviders([]);
        return;
      }

      // Filter for @ai-sdk/openai providers
      const openaiProviders: OpenCodeProviderDisplay[] = [];
      for (const [id, providerData] of Object.entries(config.provider)) {
        if (providerData.npm === '@ai-sdk/openai') {
          const models = Object.entries(providerData.models || {}).map(([modelId, model]) => ({
            id: modelId,
            name: (model as OpenCodeModel).name || modelId,
          }));

          openaiProviders.push({
            id,
            name: providerData.name || id,
            baseUrl: providerData.options?.baseURL,
            apiKey: providerData.options?.apiKey,
            models,
          });
        }
      }

      setOpenCodeProviders(openaiProviders);
    } catch (error) {
      console.error('Failed to load OpenCode providers:', error);
      const errorMsg = error instanceof Error ? error.message : String(error);
      message.error(errorMsg || t('common.error'));
    } finally {
      setLoadingProviders(false);
    }
  };

  const handleProviderSelect = (providerId: string) => {
    const providerData = openCodeProviders.find((p) => p.id === providerId);
    if (!providerData) return;

    setSelectedProvider(providerData);
    setAvailableModels(providerData.models);

    // Process baseUrl: only remove trailing /
    let processedUrl = providerData.baseUrl || '';
    if (processedUrl.endsWith('/')) {
      processedUrl = processedUrl.slice(0, -1);
    }
    setProcessedBaseUrl(processedUrl);

    // Update Hook state
    handleApiKeyChange(providerData.apiKey || '');
    handleBaseUrlChange(processedUrl);

    // Auto-fill form
    form.setFieldsValue({
      name: providerData.name,
      baseUrl: processedUrl,
      apiKey: providerData.apiKey || '',
    });
  };

  const handleSubmit = async () => {
    try {
      const fieldsToValidate = activeTab === 'import'
        ? ['sourceProvider', 'name', 'apiKey', 'authJson', 'configToml', 'notes']
        : ['name', 'apiKey', 'authJson', 'configToml', 'notes'];

      // 强制触发一次同步，确保所有字段都已同步到 auth.json 和 config.toml
      const currentValues = form.getFieldsValue();
      if (currentValues.apiKey !== undefined) {
        handleApiKeyChange(currentValues.apiKey || '');
      }
      if (currentValues.baseUrl !== undefined) {
        handleBaseUrlChange(currentValues.baseUrl || '');
      }
      if (currentValues.model !== undefined) {
        handleModelChange(currentValues.model || '');
      }

      const values = await form.validateFields(fieldsToValidate);

      setLoading(true);

      // 从表单获取最新的 config.toml 值（同步后表单中的值是最新的）
      const latestConfigToml = (form.getFieldValue('configToml') as string) || '';
      // 使用 Hook 提供的最终配置（已合并字段），但 config 使用表单最新值
      const settingsConfig = getFinalSettingsConfig(latestConfigToml);

      const formValues: CodexProviderFormValues = {
        name: values.name,
        category: 'custom',
        settingsConfig,
        notes: values.notes,
        sourceProviderId: activeTab === 'import' ? selectedProvider?.id : undefined,
      };

      await onSubmit(formValues);
      form.resetFields();
      setSelectedProvider(null);
      setAvailableModels([]);
      onCancel();
    } catch (error) {
      console.error('Form validation failed:', error);
    } finally {
      setLoading(false);
    }
  };

  const modelSelectOptions = availableModels.map((model) => ({
    label: `${model.name} (${model.id})`,
    value: model.id,
  }));

  const renderManualTab = () => (
    <Form
      form={form}
      layout="horizontal"
      labelCol={labelCol}
      wrapperCol={wrapperCol}
      onValuesChange={(changedValues) => {
        // 当表单值变化时，同步到 Hook 状态
        if ('apiKey' in changedValues) {
          handleApiKeyChange(changedValues.apiKey || '');
        }
        if ('authJson' in changedValues) {
          handleAuthChange(changedValues.authJson || {});
        }
        if ('baseUrl' in changedValues) {
          handleBaseUrlChange(changedValues.baseUrl || '');
        }
        if ('model' in changedValues) {
          handleModelChange(changedValues.model || '');
        }
        if ('configToml' in changedValues) {
          handleConfigChange(changedValues.configToml || '');
        }
      }}
    >
      <Form.Item
        name="name"
        label={t('codex.provider.name')}
        rules={[{ required: true, message: t('common.error') }]}
      >
        <Input placeholder={t('codex.provider.namePlaceholder')} />
      </Form.Item>

      <Form.Item
        name="apiKey"
        label={t('codex.provider.apiKey')}
        rules={[{ required: true, message: t('common.error') }]}
      >
        <Input
          type={showApiKey ? 'text' : 'password'}
          placeholder={t('codex.provider.apiKeyPlaceholder')}
          addonAfter={
            <Button
              type="text"
              size="small"
              icon={showApiKey ? <EyeInvisibleOutlined /> : <EyeOutlined />}
              onClick={() => setShowApiKey(!showApiKey)}
            >
              {showApiKey ? t('codex.provider.hideApiKey') : t('codex.provider.showApiKey')}
            </Button>
          }
        />
      </Form.Item>

      <Form.Item
        name="baseUrl"
        label={t('codex.provider.baseUrl')}
        rules={[{ required: true, message: t('common.error') }]}
        help={<Text type="secondary" style={{ fontSize: 12 }}>{t('codex.provider.baseUrlHelp')}</Text>}
      >
        <Input 
          placeholder="https://your-api-endpoint.com/v1"
        />
      </Form.Item>

      <Form.Item
        name="model"
        label={t('codex.provider.modelName')}
        help={<Text type="secondary" style={{ fontSize: 12 }}>{t('codex.provider.modelNameHelp')}</Text>}
      >
        <Input 
          placeholder={t('codex.provider.modelNamePlaceholder')}
        />
      </Form.Item>

      <Form.Item 
        name="authJson" 
        label="auth.json"
        extra={<Text type="secondary" style={{ fontSize: 12 }}>{t('codex.provider.authJsonHelp')}</Text>}
        rules={[validateJsonRule(t('codex.provider.authJsonInvalid'))]}
      >
        <JsonEditorFormItem />
      </Form.Item>

      <Form.Item 
        name="configToml" 
        label="config.toml"
        extra={<Text type="secondary" style={{ fontSize: 12 }}>{t('codex.provider.configTomlHelp')}</Text>}
        rules={[validateTomlRule(t('codex.provider.configTomlInvalid'))]}
      >
        <TomlEditorFormItem 
          placeholder={t('codex.provider.configTomlPlaceholder')}
        />
      </Form.Item>

      <Form.Item name="notes" label={t('codex.provider.notes')}>
        <TextArea
          rows={2}
          placeholder={t('codex.provider.notesPlaceholder')}
        />
      </Form.Item>
    </Form>
  );

  const renderImportTab = () => (
    <div>
      <Form
        form={form}
        layout="horizontal"
        labelCol={labelCol}
        wrapperCol={wrapperCol}
        onValuesChange={(changedValues) => {
          // 当表单值变化时，同步到 Hook 状态
          if ('apiKey' in changedValues) {
            handleApiKeyChange(changedValues.apiKey || '');
          }
          if ('authJson' in changedValues) {
            handleAuthChange(changedValues.authJson || {});
          }
          if ('baseUrl' in changedValues) {
            handleBaseUrlChange(changedValues.baseUrl || '');
          }
          if ('model' in changedValues) {
            handleModelChange(changedValues.model || '');
          }
          if ('configToml' in changedValues) {
            handleConfigChange(changedValues.configToml || '');
          }
        }}
      >
        <Form.Item
          name="sourceProvider"
          label={t('codex.import.selectProvider')}
          rules={[{ required: true, message: t('common.error') }]}
        >
          <Select
            placeholder={t('codex.import.selectProviderPlaceholder')}
            loading={loadingProviders}
            onChange={handleProviderSelect}
            options={openCodeProviders.map((p) => ({
              label: `${p.name} (${p.baseUrl || ''})`,
              value: p.id,
            }))}
          />
        </Form.Item>

        {selectedProvider && (
          <Alert
            message={t('codex.import.importInfo')}
            description={
              <Space direction="vertical" size={4}>
                <div>{t('codex.import.providerName')}: {selectedProvider.name}</div>
                <div>{t('codex.import.baseUrl')}: {processedBaseUrl}</div>
                <div>{t('codex.import.availableModels')}: {availableModels.length > 0 ? t('codex.import.modelsCount', { count: availableModels.length }) : '-'}</div>
              </Space>
            }
            type="success"
            showIcon
            style={{ marginBottom: 16 }}
          />
        )}

        <Form.Item name="name" label={t('codex.provider.name')}>
          <Input placeholder={t('codex.provider.namePlaceholder')} disabled />
        </Form.Item>

        <Form.Item name="apiKey" label={t('codex.provider.apiKey')}>
          <Input type="password" disabled />
        </Form.Item>

        {availableModels.length > 0 && (
          <>
            <Alert
              message={t('codex.model.selectFromProvider')}
              type="info"
              showIcon
              style={{ marginBottom: 16 }}
            />

            <Form.Item name="model" label={t('codex.import.selectDefaultModel')}>
              <Select
                placeholder={t('codex.model.defaultModelPlaceholder')}
                options={modelSelectOptions}
                allowClear
                showSearch
              />
            </Form.Item>
          </>
        )}

        <Form.Item 
          name="authJson" 
          label="auth.json"
          extra={<Text type="secondary" style={{ fontSize: 12 }}>{t('codex.provider.authJsonHelp')}</Text>}
          rules={[validateJsonRule(t('codex.provider.authJsonInvalid'))]}
        >
          <JsonEditorFormItem />
        </Form.Item>

        <Form.Item 
          name="configToml" 
          label="config.toml"
          extra={<Text type="secondary" style={{ fontSize: 12 }}>{t('codex.provider.configTomlHelp')}</Text>}
          rules={[validateTomlRule(t('codex.provider.configTomlInvalid'))]}
        >
          <TomlEditorFormItem 
            placeholder={t('codex.provider.configTomlPlaceholder')}
          />
        </Form.Item>

        <Form.Item name="notes" label={t('codex.provider.notes')}>
          <TextArea
            rows={2}
            placeholder={t('codex.provider.notesPlaceholder')}
          />
        </Form.Item>
      </Form>
    </div>
  );

  return (
    <Modal
      title={isEdit ? t('codex.provider.editProvider') : t('codex.provider.addProvider')}
      open={open}
      onCancel={onCancel}
      onOk={handleSubmit}
      confirmLoading={loading}
      width={800}
      okText={t('common.save')}
      cancelText={t('common.cancel')}
      destroyOnClose
    >
      {!isEdit && (
        <Tabs
          activeKey={activeTab}
          onChange={(key) => setActiveTab(key as 'manual' | 'import')}
          items={[
            {
              key: 'manual',
              label: t('codex.form.tabManual'),
              children: renderManualTab(),
            },
            {
              key: 'import',
              label: t('codex.form.tabImport'),
              children: renderImportTab(),
            },
          ]}
        />
      )}
      {isEdit && renderManualTab()}
    </Modal>
  );
};

export default CodexProviderFormModal;
