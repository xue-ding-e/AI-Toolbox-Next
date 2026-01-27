import React from 'react';
import { Modal, Form, Input, Select, Button, Typography, message, InputNumber, Switch, Space, Checkbox } from 'antd';
import { EyeOutlined, EyeInvisibleOutlined, RightOutlined, DownOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '@/stores';
import { PROVIDER_TYPES } from '@/constants/providerTypes';
import HeadersEditor from '@/components/common/HeadersEditor';
import JsonEditor from '@/components/common/JsonEditor';
import type { I18nPrefix } from '@/components/common/ProviderCard/types';

const { Text } = Typography;

/**
 * Form values for provider form
 */
export interface ProviderFormValues {
  id: string;
  name: string;
  sdkType: string;
  baseUrl: string;
  apiKey?: string;
  headers?: string | Record<string, string>;
  timeout?: number;
  disableTimeout?: boolean;
  setCacheKey?: boolean;
  extraOptions?: Record<string, unknown>;
}

interface ProviderFormModalProps {
  open: boolean;
  
  /** Whether this is an edit operation */
  isEdit?: boolean;
  /** Initial form values */
  initialValues?: Partial<ProviderFormValues>;
  
  /** Existing provider IDs for duplicate check (only used when !isEdit) */
  existingIds?: string[];
  /** Whether API key is required (settings page: true, OpenCode: false) */
  apiKeyRequired?: boolean;
  
  /** Callbacks */
  onCancel: () => void;
  onSuccess: (values: ProviderFormValues) => void;
  /** Custom duplicate ID error handler */
  onDuplicateId?: (id: string) => void;
  
  /** i18n prefix for translations */
  i18nPrefix?: I18nPrefix;
  
  /** Headers output format */
  headersOutputFormat?: 'string' | 'object';
  
  /** Whether to show OpenCode advanced options (timeout, setCacheKey) */
  showOpenCodeAdvanced?: boolean;
}

/**
 * A reusable provider form modal component
 */
const ProviderFormModal: React.FC<ProviderFormModalProps> = ({
  open,
  isEdit = false,
  initialValues,
  existingIds = [],
  apiKeyRequired = true,
  onCancel,
  onSuccess,
  onDuplicateId,
  i18nPrefix = 'settings',
  headersOutputFormat = 'string',
  showOpenCodeAdvanced = false,
}) => {
  const { t } = useTranslation();
  const language = useAppStore((state) => state.language);
  const [form] = Form.useForm();
  const [loading, setLoading] = React.useState(false);
  const [showApiKey, setShowApiKey] = React.useState(false);
  const [headersValid, setHeadersValid] = React.useState(true);
  const [extraOptionsValid, setExtraOptionsValid] = React.useState(true);
  const [advancedExpanded, setAdvancedExpanded] = React.useState(false);

  const labelCol = { span: language === 'zh-CN' ? 4 : 6 };
  const wrapperCol = { span: 20 };

  // Check if headers or OpenCode advanced options have content
  const hasAdvancedContent = React.useMemo(() => {
    const headers = form.getFieldValue('headers');
    const timeout = form.getFieldValue('timeout');
    const disableTimeout = form.getFieldValue('disableTimeout');
    const setCacheKey = form.getFieldValue('setCacheKey');
    const extraOptions = form.getFieldValue('extraOptions');
    
    // Check headers
    let hasHeaders = false;
    if (headers) {
      if (typeof headers === 'string') {
        try {
          const parsed = JSON.parse(headers);
          hasHeaders = Object.keys(parsed).length > 0;
        } catch {
          hasHeaders = headers.trim().length > 0;
        }
      } else if (typeof headers === 'object') {
        hasHeaders = Object.keys(headers).length > 0;
      }
    }
    
    // Check extraOptions
    let hasExtraOptions = false;
    if (extraOptions && typeof extraOptions === 'object') {
      hasExtraOptions = Object.keys(extraOptions).length > 0;
    }
    
    // Check OpenCode advanced options
    const hasOpenCodeAdvanced = showOpenCodeAdvanced && (
      disableTimeout === true || 
      timeout !== undefined || 
      setCacheKey === true ||
      hasExtraOptions
    );
    
    return hasHeaders || hasOpenCodeAdvanced;
  }, [form, showOpenCodeAdvanced]);

  React.useEffect(() => {
    if (open) {
      if (initialValues) {
        form.setFieldsValue(initialValues);
        // Auto expand if headers or OpenCode advanced options have content
        const headers = initialValues.headers;
        const timeout = initialValues.timeout;
        const disableTimeout = initialValues.disableTimeout;
        const setCacheKey = initialValues.setCacheKey;
        const extraOptions = initialValues.extraOptions;
        
        let shouldExpand = false;
        
        // Check headers
        if (headers) {
          if (typeof headers === 'string') {
            try {
              const parsed = JSON.parse(headers);
              shouldExpand = Object.keys(parsed).length > 0;
            } catch {
              shouldExpand = headers.trim().length > 0;
            }
          } else if (typeof headers === 'object') {
            shouldExpand = Object.keys(headers).length > 0;
          }
        }
        
        // Check OpenCode advanced options
        if (!shouldExpand && showOpenCodeAdvanced) {
          shouldExpand = disableTimeout === true || 
            timeout !== undefined || 
            setCacheKey === true;
        }
        
        // Check extraOptions
        if (!shouldExpand && showOpenCodeAdvanced && extraOptions) {
          shouldExpand = typeof extraOptions === 'object' && Object.keys(extraOptions).length > 0;
        }
        
        setAdvancedExpanded(shouldExpand);
      } else {
        form.resetFields();
        setAdvancedExpanded(false);
      }
      setShowApiKey(false);
      setHeadersValid(true);
      setExtraOptionsValid(true);
    }
  }, [open, initialValues, form, showOpenCodeAdvanced]);

  // Provider types that need /v1 suffix check
  const PROVIDERS_NEED_V1 = ['@ai-sdk/anthropic', '@ai-sdk/openai-compatible'];
  const PROVIDERS_NEED_V1_OR_V1BETA = ['@ai-sdk/google'];

  const doSubmit = (values: ProviderFormValues) => {
    onSuccess(values);
    form.resetFields();
    setLoading(false);
  };

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      
      // Validate headers JSON
      if (!headersValid) {
        message.error(t('settings.provider.invalidHeaders'));
        return;
      }
      
      // Validate extraOptions JSON
      if (!extraOptionsValid) {
        message.error(t('opencode.provider.invalidExtraOptions'));
        return;
      }
      
      setLoading(true);

      // Check for duplicate ID when creating
      if (!isEdit && existingIds.includes(values.id)) {
        if (onDuplicateId) {
          onDuplicateId(values.id);
        }
        setLoading(false);
        return;
      }

      // Remove trailing slash from baseUrl (only if baseUrl is provided)
      let baseUrl = values.baseUrl;
      if (baseUrl && baseUrl.endsWith('/')) {
        baseUrl = baseUrl.slice(0, -1);
        values.baseUrl = baseUrl;
      }

      // Check baseURL suffix for OpenCode only (only if baseUrl is provided)
      if (i18nPrefix === 'opencode' && baseUrl) {
        const sdkType = values.sdkType;
        let needConfirm = false;
        let confirmMessageKey = '';

        if (PROVIDERS_NEED_V1.includes(sdkType) && !baseUrl.endsWith('/v1')) {
          needConfirm = true;
          confirmMessageKey = 'opencode.provider.baseUrlConfirmV1';
        } else if (PROVIDERS_NEED_V1_OR_V1BETA.includes(sdkType) &&
                   !baseUrl.endsWith('/v1') && !baseUrl.endsWith('/v1beta')) {
          needConfirm = true;
          confirmMessageKey = 'opencode.provider.baseUrlConfirmV1Beta';
        }

        if (needConfirm) {
          Modal.confirm({
            title: t('common.confirm'),
            content: t(confirmMessageKey),
            okText: t('common.confirm'),
            cancelText: t('common.cancel'),
            onOk: () => {
              doSubmit(values as ProviderFormValues);
            },
          });
          setLoading(false);
          return;
        }
      }

      doSubmit(values as ProviderFormValues);
    } catch (error: unknown) {
      console.error('Provider form validation error:', error);
      // Form validation errors are already shown by Form
    } finally {
      setLoading(false);
    }
  };

  // Build i18n keys based on prefix
  const getKey = (key: string) => `${i18nPrefix}.provider.${key}`;

  return (
    <Modal
      title={isEdit ? t(`${i18nPrefix}.editProvider`) : t(`${i18nPrefix}.addProvider`)}
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
        labelCol={labelCol}
        wrapperCol={wrapperCol}
        style={{ marginTop: 24 }}
      >
        <Form.Item
          label={t(getKey('id'))}
          name="id"
          rules={[{ required: true, message: t(getKey('idPlaceholder')) }]}
        >
          <Input
            placeholder={t(getKey('idPlaceholder'))}
            disabled={isEdit}
          />
        </Form.Item>

        <Form.Item
          label={t(getKey('name'))}
          name="name"
          rules={[{ required: true, message: t(getKey('namePlaceholder')) }]}
        >
          <Input placeholder={t(getKey('namePlaceholder'))} />
        </Form.Item>

        <Form.Item
          label={i18nPrefix === 'settings' ? t('settings.provider.providerType') : t('opencode.provider.npm')}
          name="sdkType"
          rules={[{ required: true }]}
          initialValue="@ai-sdk/openai-compatible"
        >
          <Select
            placeholder={i18nPrefix === 'settings' ? t('settings.provider.providerType') : t('opencode.provider.npmPlaceholder')}
            showSearch
            optionFilterProp="label"
            optionRender={(option) => (
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span>{option.label}</span>
                <Text type="secondary" style={{ fontSize: 12 }}>{option.value}</Text>
              </div>
            )}
            options={PROVIDER_TYPES}
          />
        </Form.Item>

        <Form.Item
          label={i18nPrefix === 'settings' ? t('settings.provider.baseUrl') : t('opencode.provider.baseURL')}
          name="baseUrl"
          rules={i18nPrefix === 'settings' ? [{ required: true, message: t('settings.provider.baseUrlPlaceholder') }] : undefined}
          extra={<Text type="secondary" style={{ fontSize: 12 }}>{t(`${i18nPrefix}.provider.baseUrlHint`)}</Text>}
        >
          <Input placeholder={i18nPrefix === 'settings' ? t('settings.provider.baseUrlPlaceholder') : t('opencode.provider.baseURLPlaceholder')} />
        </Form.Item>

        <Form.Item
          label={t(getKey('apiKey'))}
          name="apiKey"
          rules={apiKeyRequired ? [{ required: true, message: t(getKey('apiKeyPlaceholder')) }] : undefined}
        >
          <Input
            type={showApiKey ? 'text' : 'password'}
            placeholder={t(getKey('apiKeyPlaceholder'))}
            suffix={
              <Button
                type="text"
                size="small"
                icon={showApiKey ? <EyeInvisibleOutlined /> : <EyeOutlined />}
                onClick={() => setShowApiKey(!showApiKey)}
                style={{ marginRight: -8 }}
              />
            }
          />
        </Form.Item>

        <div style={{ marginBottom: advancedExpanded ? 16 : 0 }}>
          <Button
            type="link"
            onClick={() => setAdvancedExpanded(!advancedExpanded)}
            style={{ padding: 0, height: 'auto' }}
          >
            {advancedExpanded ? <DownOutlined /> : <RightOutlined />}
            <span style={{ marginLeft: 4 }}>
              {t('common.advancedSettings')}
              {hasAdvancedContent && !advancedExpanded && (
                <span style={{ marginLeft: 4, color: '#1890ff' }}>*</span>
              )}
            </span>
          </Button>
        </div>
        {advancedExpanded && (
          <>
            <Form.Item
              label={t(getKey('headers'))}
              name="headers"
              extra={i18nPrefix === 'settings' ? <Text type="secondary" style={{ fontSize: 12 }}>{t('settings.provider.headersHint')}</Text> : undefined}
            >
              <HeadersEditor
                outputFormat={headersOutputFormat}
                height={200}
                onValidationChange={setHeadersValid}
                placeholder={i18nPrefix === 'opencode' ? `{
    "Helicone-Cache-Enabled": "true",
    "Helicone-User-Id": "opencode"
}` : undefined}
              />
            </Form.Item>
            
            {showOpenCodeAdvanced && (
              <>
                {/* Timeout field */}
                <Form.Item
                  label={t('opencode.provider.timeout')}
                  extra={<Text type="secondary" style={{ fontSize: 12 }}>{t('opencode.provider.timeoutHint')}</Text>}
                >
                  <Form.Item
                    noStyle
                    shouldUpdate={(prev, curr) => prev.disableTimeout !== curr.disableTimeout}
                  >
                    {({ getFieldValue }) => (
                      <Space style={{ width: '100%' }}>
                        {!getFieldValue('disableTimeout') && (
                          <Form.Item name="timeout" noStyle>
                            <InputNumber
                              style={{ width: 200 }}
                              placeholder={t('opencode.provider.timeoutPlaceholder')}
                              min={0}
                              addonAfter="ms"
                            />
                          </Form.Item>
                        )}
                        <Form.Item name="disableTimeout" valuePropName="checked" noStyle>
                          <Checkbox>{t('opencode.provider.disableTimeout')}</Checkbox>
                        </Form.Item>
                      </Space>
                    )}
                  </Form.Item>
                </Form.Item>

                {/* SetCacheKey field */}
                <Form.Item
                  label={t('opencode.provider.setCacheKey')}
                  name="setCacheKey"
                  valuePropName="checked"
                  extra={<Text type="secondary" style={{ fontSize: 12 }}>{t('opencode.provider.setCacheKeyHint')}</Text>}
                >
                  <Switch />
                </Form.Item>

                {/* ExtraOptions field */}
                <Form.Item
                  label={t('opencode.provider.extraOptions')}
                  name="extraOptions"
                  extra={<Text type="secondary" style={{ fontSize: 12 }}>{t('opencode.provider.extraOptionsHint')}</Text>}
                  getValueFromEvent={(value: unknown, isValid: boolean) => {
                    setExtraOptionsValid(isValid);
                    // 始终返回值，保留用户输入，即使 JSON 无效也不清空
                    // 提交时会通过 extraOptionsValid 状态来阻止无效的 JSON 被保存
                    return value;
                  }}
                >
                  <JsonEditor
                    value={form.getFieldValue('extraOptions')}
                    height={150}
                    minHeight={100}
                    maxHeight={300}
                    resizable
                    placeholder={`{
    "organization": "my-org",
    "project": "my-project"
}`}
                  />
                </Form.Item>
              </>
            )}
          </>
        )}
      </Form>
    </Modal>
  );
};

export default ProviderFormModal;
