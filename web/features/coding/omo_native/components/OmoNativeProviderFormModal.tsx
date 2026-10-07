import React from 'react';
import { App, Form, Input, Modal, Select } from 'antd';
import { Settings2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import JsonEditor from '@/components/common/JsonEditor';
import ProviderConfigCollapse from '@/features/coding/shared/providerConfig/ProviderConfigCollapse';
import { saveOmoNativeProvider } from '@/services/omoNativeApi';
import type { OmoNativeProvider } from '@/types/omoNative';
import {
  OMO_NATIVE_API_OPTIONS,
  asRecord,
  getStringField,
  isRecordEmpty,
} from '../utils/omoNativeProviders';

interface OmoNativeProviderFormModalProps {
  open: boolean;
  /** Provider being edited; omit for "add". */
  provider?: OmoNativeProvider;
  /** True when the form is seeded from `provider` but saves as a new one (copy). */
  isCopy?: boolean;
  onCancel: () => void;
  /** Fired after a successful save: the list and the tray menu both need it. */
  onSaved: () => Promise<void> | void;
}

/**
 * OmO Native 的供应商编辑弹窗。
 *
 * 形态与其余 CLI 的 `*ProviderFormModal.tsx` 一致：**水平布局**
 * （`labelCol` / `wrapperCol`，字段一行一个），高级设置走共享的
 * `ProviderConfigCollapse`。**不套 `ProviderFormSections`**——那个组件无条件渲染
 * 「备注」，而本模块的 provider 存在引擎的 `models.json` 里，没有备注字段可存，
 * 加了就是写出没人读的键（网关计费/请求头/模型改写三个分区同理，本 CLI 不在网关
 * 支持列表内，一律不渲染）。
 */
const OmoNativeProviderFormModal: React.FC<OmoNativeProviderFormModalProps> = ({
  open,
  provider,
  isCopy = false,
  onCancel,
  onSaved,
}) => {
  const { t, i18n } = useTranslation();
  const { message } = App.useApp();
  const [form] = Form.useForm();
  const [headersJson, setHeadersJson] = React.useState<Record<string, unknown>>({});
  const [headersJsonValid, setHeadersJsonValid] = React.useState(true);
  const [advancedExpanded, setAdvancedExpanded] = React.useState(false);
  const [saving, setSaving] = React.useState(false);

  // 起点配置：`models` 目录与引擎写的未知键都要原样带走——保存一条 provider
  // 不能顺手删掉它的模型。
  const baseConfig = React.useMemo(() => asRecord(provider?.config), [provider]);

  React.useEffect(() => {
    if (!open) return;
    setHeadersJson(asRecord(baseConfig.headers));
    setHeadersJsonValid(true);
    setAdvancedExpanded(false);
    form.setFieldsValue({
      providerKey: isCopy && provider ? `${provider.key}_copy` : provider?.key,
      displayName: getStringField(baseConfig, 'name'),
      api: getStringField(baseConfig, 'api') || undefined,
      baseUrl: getStringField(baseConfig, 'baseUrl'),
      // 回填**明文**密钥：本应用是配置管理器，用户看自己的密钥是天经地义的，
      // 与 Claude Code / Codex / Kimi 的弹窗一致。复制模式下把源密钥也带过去
      // （用户多半就是要换个 key 用的同一家）。
      apiKey: provider?.apiKey ?? '',
    });
  }, [open, provider, isCopy, baseConfig, form]);

  const handleSubmit = async () => {
    if (!headersJsonValid) return;
    const values = await form.validateFields();
    const providerKey = (values.providerKey as string | undefined)?.trim();
    if (!providerKey) {
      message.error(t('omoNative.providers.providerKeyRequired'));
      return;
    }

    setSaving(true);
    try {
      const nextConfig: Record<string, unknown> = { ...baseConfig };
      const setOptionalString = (key: string, value?: string) => {
        const trimmed = value?.trim();
        if (trimmed) {
          nextConfig[key] = trimmed;
        } else {
          delete nextConfig[key];
        }
      };
      setOptionalString('name', values.displayName);
      setOptionalString('api', values.api);
      setOptionalString('baseUrl', values.baseUrl);
      if (isRecordEmpty(headersJson)) {
        delete nextConfig.headers;
      } else {
        nextConfig.headers = headersJson;
      }

      await saveOmoNativeProvider({
        key: providerKey,
        config: nextConfig,
        apiKey: (values.apiKey as string | undefined)?.trim() || undefined,
      });
      message.success(t('common.success'));
      await onSaved();
    } catch (error) {
      console.error('Failed to save OmO Native provider:', error);
      message.error(t('common.error'));
    } finally {
      setSaving(false);
    }
  };

  const labelCol = { span: i18n.language === 'zh-CN' ? 4 : 6 };
  const wrapperCol = { span: 20 };

  return (
    <Modal
      // 不带 provider id：标题里的 id 是 `models.json` 的键，对用户不是身份，
      // 只是实现细节（与 ZCode / Kimi 的「编辑供应商」同）。要认人看表单里的
      // 「名称」字段。
      title={
        provider && !isCopy
          ? t('omoNative.providers.editSupplierTitle')
          : t('omoNative.providers.addSupplierTitle')
      }
      open={open}
      width={720}
      confirmLoading={saving}
      okText={t('common.save')}
      cancelText={t('common.cancel')}
      onCancel={onCancel}
      onOk={() => void handleSubmit()}
      destroyOnHidden
    >
      <Form form={form} layout="horizontal" labelCol={labelCol} wrapperCol={wrapperCol}>
        <Form.Item
          name="providerKey"
          label={t('omoNative.providers.providerKey')}
          rules={[{ required: true, message: t('omoNative.providers.providerKeyRequired') }]}
        >
          {/* 编辑时 key 是 `models.json` 的索引，改了等于换一条记录；
              复制走的虽也是这条记录，但要落成一个**新** key，所以可编辑。 */}
          <Input disabled={Boolean(provider) && !isCopy} placeholder="my-provider" />
        </Form.Item>
        <Form.Item name="displayName" label={t('omoNative.providers.displayName')}>
          <Input placeholder={t('omoNative.providers.displayNamePlaceholder')} />
        </Form.Item>
        <Form.Item name="api" label={t('omoNative.providers.apiType')}>
          {/* 留空是合法状态：上游允许 `api` 只写在模型级，此时 provider 级不写。 */}
          <Select options={OMO_NATIVE_API_OPTIONS} allowClear />
        </Form.Item>
        <Form.Item name="baseUrl" label={t('omoNative.providers.baseUrl')}>
          <Input placeholder="https://api.example.com/v1" />
        </Form.Item>
        <Form.Item
          name="apiKey"
          label={t('omoNative.providers.providerApiKey')}
          extra={<span style={{ fontSize: 12 }}>{t('omoNative.providers.apiKeyHint')}</span>}
        >
          <Input.Password autoComplete="off" />
        </Form.Item>

        <Form.Item wrapperCol={{ span: 24 }}>
          <ProviderConfigCollapse
            title={t('omoNative.providers.headersJson')}
            expanded={advancedExpanded}
            onExpandedChange={setAdvancedExpanded}
            icon={<Settings2 size={14} />}
          >
            <JsonEditor
              value={isRecordEmpty(headersJson) ? undefined : headersJson}
              height={160}
              onChange={(value, isValid) => {
                if (isValid) setHeadersJson(asRecord(value));
                setHeadersJsonValid(isValid);
              }}
            />
          </ProviderConfigCollapse>
        </Form.Item>
      </Form>
    </Modal>
  );
};

export default OmoNativeProviderFormModal;