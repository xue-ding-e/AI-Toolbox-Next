import React from 'react';
import { Modal, Tabs, Form, Input, Select, Space, Button, Alert, message, Typography } from 'antd';
import { EyeInvisibleOutlined, EyeOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '@/stores';
import type { CodexProvider, CodexProviderFormValues, CodexSettingsConfig } from '@/types/codex';
import { readOpenCodeConfig } from '@/services/opencodeApi';
import type { OpenCodeModel } from '@/types/opencode';

const { Text } = Typography;
const { TextArea } = Input;

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

  // When Modal opens, set activeTab based on defaultTab
  React.useEffect(() => {
    if (open) {
      setActiveTab(defaultTab);
    }
  }, [open, defaultTab]);

  // Load OpenCode providers list when import tab is active
  React.useEffect(() => {
    if (open && activeTab === 'import') {
      loadOpenCodeProviders();
    }
  }, [open, activeTab]);

  // Initialize form
  React.useEffect(() => {
    if (open && provider) {
      let settingsConfig: CodexSettingsConfig = {};
      try {
        settingsConfig = JSON.parse(provider.settingsConfig);
      } catch (error) {
        console.error('Failed to parse settingsConfig:', error);
      }

      // Extract base_url from config.toml if present
      let baseUrl = '';
      const configContent = settingsConfig.config || '';
      const baseUrlMatch = configContent.match(/base_url\s*=\s*["']([^"']+)["']/);
      if (baseUrlMatch) {
        baseUrl = baseUrlMatch[1];
      }

      form.setFieldsValue({
        name: provider.name,
        apiKey: settingsConfig.auth?.OPENAI_API_KEY,
        baseUrl: baseUrl,
        model: '',
        configToml: configContent,
        notes: provider.notes,
      });
    } else if (open && !provider) {
      form.resetFields();
    }
  }, [open, provider, form]);

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
      message.error(t('common.error'));
    } finally {
      setLoadingProviders(false);
    }
  };

  const handleProviderSelect = (providerId: string) => {
    const providerData = openCodeProviders.find((p) => p.id === providerId);
    if (!providerData) return;

    setSelectedProvider(providerData);
    setAvailableModels(providerData.models);

    // Process baseUrl: remove trailing /v1 and /
    let processedUrl = providerData.baseUrl || '';
    if (processedUrl.endsWith('/v1')) {
      processedUrl = processedUrl.slice(0, -3);
    }
    if (processedUrl.endsWith('/')) {
      processedUrl = processedUrl.slice(0, -1);
    }
    setProcessedBaseUrl(processedUrl);

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
        ? ['sourceProvider', 'name', 'apiKey', 'configToml', 'notes']
        : ['name', 'apiKey', 'configToml', 'notes'];

      const values = await form.validateFields(fieldsToValidate);

      setLoading(true);

      const formValues: CodexProviderFormValues = {
        name: values.name,
        category: 'custom',
        apiKey: values.apiKey,
        baseUrl: values.baseUrl,
        model: values.model,
        configToml: values.configToml,
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
      >
        <Input placeholder={t('codex.provider.baseUrlPlaceholder')} />
      </Form.Item>

      <Form.Item
        name="model"
        label={t('codex.provider.modelName')}
        help={<Text type="secondary" style={{ fontSize: 12 }}>{t('codex.provider.modelNameHelp')}</Text>}
      >
        <Input placeholder={t('codex.provider.modelNamePlaceholder')} />
      </Form.Item>

      <Form.Item name="configToml" label={t('codex.provider.configToml')}>
        <TextArea
          rows={4}
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

        <Form.Item name="configToml" label={t('codex.provider.configToml')}>
          <TextArea
            rows={4}
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
      width={600}
      okText={t('common.save')}
      cancelText={t('common.cancel')}
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
