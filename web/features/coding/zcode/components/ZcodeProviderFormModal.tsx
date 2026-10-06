import React from 'react';
import { Form, Input, Modal, Select, Typography, message } from 'antd';
import { useTranslation } from 'react-i18next';
import {
  createZcodeProvider,
  listZcodeProviderTemplates,
  saveZcodeProvider,
  updateZcodeProvider,
} from '@/services/zcodeApi';
import ProviderFormSections from '@/features/coding/shared/providerConfig/ProviderFormSections';
import {
  getBillingConfigFromMeta,
  mergeBillingConfigIntoMeta,
  type BillingConfigState,
} from '@/features/coding/shared/providerBilling/billingConfigUtils';
import {
  getCustomHeadersFromMeta,
  mergeCustomHeadersIntoMeta,
  type CustomHeadersState,
} from '@/features/coding/shared/providerHeaders/customHeadersUtils';
import {
  getModelRewritesFromMeta,
  mergeModelRewritesIntoMeta,
  type ModelRewritesState,
} from '@/features/coding/shared/providerModelRewrites/modelRewritesUtils';
import type { ZcodeProvider, ZcodeProviderTemplate, ZcodeSettingsConfig } from '@/types/zcode';
import { ZCODE_API_TYPES } from '@/types/zcode';
import {
  buildZcodeProviderId,
  parseZcodeProviderSettings,
} from '../utils/zcodeSettingsConfig';

const { Text } = Typography;

interface ZcodeProviderFormModalProps {
  open: boolean;
  provider: ZcodeProvider | null;
  onCancel: () => void;
  onSaved: () => Promise<void> | void;
}

interface ZcodeProviderFormValues {
  name: string;
  providerId: string;
  templateId?: string;
  apiType: string;
  baseUrl: string;
  apiKey?: string;
  notes?: string;
}

/**
 * ZCode resolves a provider's request shape from `templateId` when set, so a
 * template-driven provider only needs the credentials that differ from it.
 */
const ZcodeProviderFormModal: React.FC<ZcodeProviderFormModalProps> = ({
  open,
  provider,
  onCancel,
  onSaved,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm<ZcodeProviderFormValues>();
  const [templates, setTemplates] = React.useState<ZcodeProviderTemplate[]>([]);
  const [saving, setSaving] = React.useState(false);
  const [billingConfig, setBillingConfig] = React.useState<BillingConfigState>(() =>
    getBillingConfigFromMeta(provider?.meta ?? undefined),
  );
  const [customHeaders, setCustomHeaders] = React.useState<CustomHeadersState>(() =>
    getCustomHeadersFromMeta(provider?.meta ?? undefined),
  );
  const [modelRewrites, setModelRewrites] = React.useState<ModelRewritesState>(() =>
    getModelRewritesFromMeta(provider?.meta ?? undefined),
  );
  const isEditing = Boolean(provider);

  React.useEffect(() => {
    if (!open) {
      return;
    }
    void listZcodeProviderTemplates()
      .then(setTemplates)
      .catch((error) => console.error('Failed to load ZCode templates:', error));
  }, [open]);

  React.useEffect(() => {
    if (!open) {
      return;
    }
    if (provider) {
      const settings = parseZcodeProviderSettings(provider.settingsConfig);
      form.setFieldsValue({
        name: provider.name,
        providerId: settings?.providerId ?? provider.id,
        templateId: settings?.templateId,
        apiType: settings?.config?.api?.type ?? 'anthropic-messages',
        baseUrl: settings?.config?.api?.baseUrl ?? '',
        apiKey: settings?.config?.access?.apiKey ?? '',
        notes: provider.notes ?? '',
      });
    } else {
      form.resetFields();
      form.setFieldsValue({ apiType: 'anthropic-messages' });
    }
    setBillingConfig(getBillingConfigFromMeta(provider?.meta ?? undefined));
    setCustomHeaders(getCustomHeadersFromMeta(provider?.meta ?? undefined));
    setModelRewrites(getModelRewritesFromMeta(provider?.meta ?? undefined));
  }, [open, provider, form]);

  const handleTemplateChange = (templateId: string | undefined) => {
    const template = templates.find((item) => item.templateId === templateId);
    if (!template) {
      return;
    }
    form.setFieldsValue({
      apiType: template.apiType ?? form.getFieldValue('apiType'),
      baseUrl: template.baseUrl ?? form.getFieldValue('baseUrl'),
    });
  };

  const handleSubmit = async () => {
    let values: ZcodeProviderFormValues;
    try {
      values = await form.validateFields();
    } catch {
      return;
    }
    // Models are edited on the provider card, not here. Reuse whatever the
    // stored provider already has so saving the form does not wipe the catalog.
    const existingModels = provider
      ? (parseZcodeProviderSettings(provider.settingsConfig)?.models ?? [])
      : [];

    const providerId = values.providerId?.trim() || buildZcodeProviderId(values.name);
    if (providerId.startsWith('builtin:') || providerId.startsWith('account:')) {
      void message.error(
        t('zcode.form.reservedId', {
          defaultValue: '`builtin:` 与 `account:` 前缀由 ZCode 保留，请换一个 ID。',
        }),
      );
      return;
    }

    const settings: ZcodeSettingsConfig = {
      providerId,
      providerName: values.name,
      templateId: values.templateId || undefined,
      config: {
        api: {
          type: values.apiType,
          baseUrl: values.baseUrl,
        },
        access: values.apiKey
          ? { type: 'api-key', apiKey: values.apiKey }
          : undefined,
      },
      models: existingModels,
      defaultModelId: existingModels.find((model) => model.isDefault)?.modelId,
    };

    setSaving(true);
    try {
      // The section editors keep their state outside the form, so their values
      // are merged back into `meta` here. Skipping this silently drops the
      // user's billing / header / rewrite edits on save.
      const nextMeta = mergeModelRewritesIntoMeta(
        mergeCustomHeadersIntoMeta(
          mergeBillingConfigIntoMeta(provider?.meta ?? undefined, billingConfig),
          customHeaders,
        ),
        modelRewrites,
      );
      const payload = {
        name: values.name,
        category: 'custom',
        settingsConfig: JSON.stringify(settings),
        notes: values.notes ?? null,
        meta: nextMeta,
      };
      // The DB row alone does not reach ZCode; every save must also project the
      // provider into the registry so the desktop app can see the new values.
      if (provider) {
        const updated = await updateZcodeProvider({
          ...provider,
          name: payload.name,
          settingsConfig: payload.settingsConfig,
          notes: payload.notes,
          meta: payload.meta,
        });
        await saveZcodeProvider({
          id: updated.id,
          name: updated.name,
          category: updated.category,
          settingsConfig: updated.settingsConfig,
          notes: updated.notes,
        });
      } else {
        const created = await createZcodeProvider({
          ...payload,
          id: providerId,
        });
        await saveZcodeProvider({
          id: created.id,
          name: created.name,
          category: created.category,
          settingsConfig: created.settingsConfig,
          notes: created.notes,
        });
      }
      await onSaved();
    } catch (error) {
      console.error('Failed to save ZCode provider:', error);
      void message.error(String(error));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      open={open}
      title={
        isEditing
          ? t('zcode.form.editTitle', { defaultValue: '编辑供应商' })
          : t('zcode.form.addTitle', { defaultValue: '添加供应商' })
      }
      width={720}
      onCancel={onCancel}
      onOk={() => void handleSubmit()}
      confirmLoading={saving}
      okText={t('common.save', { defaultValue: '保存' })}
      cancelText={t('common.cancel', { defaultValue: '取消' })}
      destroyOnHidden
    >
      <Form form={form} layout="vertical">
                <Form.Item
                  name="name"
                  label={t('zcode.form.name', { defaultValue: '名称' })}
                  rules={[{ required: true, message: t('common.required', { defaultValue: '必填' }) }]}
                >
                  <Input placeholder="DeepSeek" />
                </Form.Item>
                <Form.Item
                  name="providerId"
                  label="Provider ID"
                  extra={
                    <Text type="secondary" style={{ fontSize: 11 }}>
                      {t('zcode.form.providerIdHint', {
                        defaultValue:
                          '写入 ZCode 的稳定标识，留空则按名称自动生成。创建后不建议修改。',
                      })}
                    </Text>
                  }
                >
                  <Input placeholder="custom:deepseek" disabled={isEditing} />
                </Form.Item>
                <Form.Item
                  name="templateId"
                  label={t('zcode.form.template', { defaultValue: '内置模板' })}
                  extra={
                    <Text type="secondary" style={{ fontSize: 11 }}>
                      {t('zcode.form.templateHint', {
                        defaultValue:
                          '选择模板后，ZCode 会沿用该供应商的请求格式与模型能力，只需填写凭据。',
                      })}
                    </Text>
                  }
                >
                  <Select
                    allowClear
                    placeholder={t('zcode.form.templatePlaceholder', { defaultValue: '不使用模板' })}
                    onChange={handleTemplateChange}
                    options={templates.map((template) => ({
                      value: template.templateId,
                      label: template.name,
                    }))}
                  />
                </Form.Item>
                <Form.Item
                  name="apiType"
                  label={t('zcode.form.apiType', { defaultValue: 'API 格式' })}
                  rules={[{ required: true }]}
                >
                  <Select options={ZCODE_API_TYPES.map((item) => ({ ...item }))} />
                </Form.Item>
                <Form.Item
                  name="baseUrl"
                  label="Base URL"
                  rules={[{ required: true, message: t('common.required', { defaultValue: '必填' }) }]}
                >
                  <Input placeholder="https://api.deepseek.com/anthropic" />
                </Form.Item>
                <Form.Item name="apiKey" label="API Key">
                  <Input.Password placeholder="sk-..." />
                </Form.Item>
        <ProviderFormSections
          editable
          billing={billingConfig}
          onBillingChange={setBillingConfig}
          customHeaders={customHeaders}
          onCustomHeadersChange={setCustomHeaders}
          modelRewrites={modelRewrites}
          onModelRewritesChange={setModelRewrites}
          notesRows={2}
          notesResetKey={`zcode-provider-notes-${provider?.id ?? 'new'}`}
        />
      </Form>
    </Modal>
  );
};

export default ZcodeProviderFormModal;
