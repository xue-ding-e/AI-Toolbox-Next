import React, { useState, useEffect, useMemo, useCallback } from 'react';
import {
  Modal,
  Form,
  Input,
  Select,
  AutoComplete,
  Button,
  Table,
  Alert,
  Tooltip,
  Popconfirm,
  message,
} from 'antd';
import {
  PlusOutlined,
  DeleteOutlined,
  InfoCircleOutlined,
} from '@ant-design/icons';
import { FileCode2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import JsonEditor from '@/components/common/JsonEditor';
import type {
  KimiProvider,
  KimiProviderFormData,
  KimiCatalogModel,
  KimiProviderCategory,
} from '@/types/kimi';
import BillingConfigCollapse from '@/features/coding/shared/providerBilling/BillingConfigCollapse';
import CustomHeadersCollapse from '@/features/coding/shared/providerHeaders/CustomHeadersCollapse';
import ProviderConfigCollapse from '@/features/coding/shared/providerConfig/ProviderConfigCollapse';
import ProviderNotesCollapse from '@/features/coding/shared/providerConfig/ProviderNotesCollapse';
import {
  getBillingConfigFromMeta,
  mergeBillingConfigIntoMeta,
} from '@/features/coding/shared/providerBilling/billingConfigUtils';
import {
  getCustomHeadersFromMeta,
  mergeCustomHeadersIntoMeta,
} from '@/features/coding/shared/providerHeaders/customHeadersUtils';
import {
  getModelRewritesFromMeta,
  mergeModelRewritesIntoMeta,
  type ModelRewritesState,
} from '@/features/coding/shared/providerModelRewrites/modelRewritesUtils';
import ModelRewritesCollapse from '@/features/coding/shared/providerModelRewrites/ModelRewritesCollapse';
import {
  parseKimiSettingsConfig,
  buildKimiSettingsConfig,
  CUSTOM_KIMI_PROVIDER_KEY,
  KIMI_OFFICIAL_DEFAULT_MODEL_ID,
  KIMI_OFFICIAL_DEFAULT_MODEL_KEY,
  KIMI_OFFICIAL_DEFAULT_MODEL_MAX_CONTEXT_SIZE,
} from '../utils/settingsConfig';
import styles from './KimiProviderFormModal.module.less';

/**
 * Default catalog entry for a newly created provider: the current official
 * model also serves as the default for custom Kimi relays. The CLI
 * hard-requires a positive max_context_size on every projected model.
 */
const DEFAULT_NEW_PROVIDER_CATALOG_MODEL: KimiCatalogModel = {
  key: KIMI_OFFICIAL_DEFAULT_MODEL_KEY,
  model: KIMI_OFFICIAL_DEFAULT_MODEL_ID,
  provider: CUSTOM_KIMI_PROVIDER_KEY,
  maxContextSize: KIMI_OFFICIAL_DEFAULT_MODEL_MAX_CONTEXT_SIZE,
};

interface KimiProviderFormModalProps {
  open: boolean;
  provider: KimiProvider | null;
  onCancel: () => void;
  onSubmit: (values: KimiProviderFormData) => Promise<void>;
}

interface FormValues {
  name: string;
  category: KimiProviderCategory;
  notes?: string;
  apiKey?: string;
  baseUrl?: string;
  defaultModelKey?: string;
}

// Common context-window presets; the field also accepts any custom number.
const CONTEXT_SIZE_PRESETS = [
  { value: '32768', label: '32K' },
  { value: '65536', label: '64K' },
  { value: '131072', label: '128K' },
  { value: '204800', label: '200K' },
  { value: '262144', label: '256K' },
  { value: '278528', label: '272K' },
  { value: '409600', label: '400K' },
  { value: '1048576', label: '1M' },
];

const KimiProviderFormModal: React.FC<KimiProviderFormModalProps> = ({
  open,
  provider,
  onCancel,
  onSubmit,
}) => {
  const { t, i18n } = useTranslation();
  const labelCol = { span: i18n.language === 'zh-CN' ? 4 : 6 };
  const wrapperCol = { span: 20 };
  const sectionWrapperCol = { span: 24 };

  const [form] = Form.useForm<FormValues>();
  const [loading, setLoading] = useState(false);

  // Category state
  const [category, setCategory] = useState<KimiProviderCategory>('custom');

  // Model catalog list state
  const [catalogModels, setCatalogModels] = useState<KimiCatalogModel[]>([]);

  // Raw settingsConfig state for advanced JSON editor
  const [rawJson, setRawJson] = useState<string>('');
  const [rawObject, setRawObject] = useState<Record<string, unknown>>({});
  const [providerKey, setProviderKey] = useState<string>(CUSTOM_KIMI_PROVIDER_KEY);
  const [customTomlConfig, setCustomTomlConfig] = useState<string>('');

  // Gateway meta sections (billing / custom headers / model rewrites),
  // mirroring the Grok form.
  const [billingConfig, setBillingConfig] = useState(() => getBillingConfigFromMeta(provider?.meta));
  const [customHeaders, setCustomHeaders] = useState(() => getCustomHeadersFromMeta(provider?.meta));
  const [modelRewrites, setModelRewrites] = useState<ModelRewritesState>(() => getModelRewritesFromMeta(provider?.meta));

  // Advanced JSON section expand state (shared self-drawn collapse)
  const [advancedExpanded, setAdvancedExpanded] = useState(false);

  const isOfficial = category === 'official';

  const notesCollapseResetKey = `${open ? 'open' : 'closed'}:${provider?.id ?? 'new'}`;

  // Initialize form data when modal opens or provider changes
  useEffect(() => {
    if (!open) return;

    if (provider) {
      const parsed = parseKimiSettingsConfig(provider.settingsConfig);
      const cat = (provider.category || 'custom') as KimiProviderCategory;
      setCategory(cat);
      setCatalogModels(parsed.catalogModels);
      setRawObject(parsed.rawObject);
      setRawJson(parsed.rawJson || (parsed.rawObject && Object.keys(parsed.rawObject).length > 0 ? JSON.stringify(parsed.rawObject, null, 2) : ''));
      setProviderKey(parsed.providerKey || CUSTOM_KIMI_PROVIDER_KEY);
      setCustomTomlConfig(parsed.customTomlConfig || '');
      setBillingConfig(getBillingConfigFromMeta(provider.meta));
      setCustomHeaders(getCustomHeadersFromMeta(provider.meta));
      setModelRewrites(getModelRewritesFromMeta(provider.meta));

      form.setFieldsValue({
        name: provider.name || '',
        category: cat,
        notes: provider.notes || '',
        apiKey: parsed.apiKey || '',
        baseUrl: parsed.baseUrl || '',
        defaultModelKey: parsed.defaultModelKey || '',
      });
    } else {
      setCategory('custom');
      setCatalogModels([{ ...DEFAULT_NEW_PROVIDER_CATALOG_MODEL }]);
      setRawObject({});
      setRawJson('');
      setProviderKey(CUSTOM_KIMI_PROVIDER_KEY);
      setCustomTomlConfig('');
      setBillingConfig(getBillingConfigFromMeta(undefined));
      setCustomHeaders(getCustomHeadersFromMeta(undefined));
      setModelRewrites(getModelRewritesFromMeta(undefined));

      form.setFieldsValue({
        name: '',
        category: 'custom',
        notes: '',
        apiKey: '',
        baseUrl: '',
        defaultModelKey: DEFAULT_NEW_PROVIDER_CATALOG_MODEL.key,
      });
    }
    setAdvancedExpanded(false);
  }, [open, provider, form]);

  // Sync structured form to raw JSON
  const syncFormToRawJson = useCallback(() => {
    const currentValues = form.getFieldsValue();
    const generatedJson = buildKimiSettingsConfig({
      category,
      apiKey: currentValues.apiKey,
      baseUrl: currentValues.baseUrl,
      providerKey,
      defaultModelKey: currentValues.defaultModelKey,
      catalogModels,
      customTomlConfig,
      rawObject,
    });
    setRawJson(generatedJson);
  }, [category, form, providerKey, catalogModels, customTomlConfig, rawObject]);

  // Reconcile raw-JSON edits back into the structured form state. Used when
  // collapsing the advanced panel and at submit time — submitting with the
  // panel open must treat the raw JSON as the freshest source instead of
  // silently overwriting it with stale form values.
  const applyRawJsonToForm = useCallback((text: string) => {
    const parsed = parseKimiSettingsConfig(text);
    if (parsed.parseError) return parsed;
    setRawObject(parsed.rawObject);
    setCatalogModels(parsed.catalogModels);
    setProviderKey(parsed.providerKey || CUSTOM_KIMI_PROVIDER_KEY);
    setCustomTomlConfig(parsed.customTomlConfig || '');
    form.setFieldsValue({
      apiKey: parsed.apiKey || '',
      baseUrl: parsed.baseUrl || '',
      defaultModelKey: parsed.defaultModelKey || '',
    });
    return parsed;
  }, [form]);

  // Handle advanced section toggle: bidirectional sync
  const handleAdvancedExpandedChange = (expanded: boolean) => {
    if (expanded && !advancedExpanded) {
      // Opening advanced panel: generate raw JSON from current form state
      syncFormToRawJson();
    } else if (!expanded && advancedExpanded) {
      // Closing advanced panel: parse raw JSON back into form fields
      if (rawJson.trim() && applyRawJsonToForm(rawJson).parseError) {
        message.warning(t('kimi.providerForm.invalidJsonPrompt'));
      }
    }

    setAdvancedExpanded(expanded);
  };

  // Add new model row. Pick the first unused `custom-model-N` index instead of
  // `length + 1`: after deleting a middle row, length+1 would collide with an
  // existing key.
  const handleAddModel = () => {
    const usedIndices = new Set(
      catalogModels
        .map((model) => /^custom-model-(\d+)$/.exec(model.key)?.[1])
        .filter((index): index is string => Boolean(index))
        .map((index) => Number.parseInt(index, 10)),
    );
    let nextIndex = 1;
    while (usedIndices.has(nextIndex)) nextIndex += 1;
    const newModel: KimiCatalogModel = {
      key: `custom-model-${nextIndex}`,
      model: `model-${nextIndex}`,
      provider: providerKey || CUSTOM_KIMI_PROVIDER_KEY,
      // Kimi CLI hard-requires a positive max_context_size per model.
      maxContextSize: KIMI_OFFICIAL_DEFAULT_MODEL_MAX_CONTEXT_SIZE,
    };
    const nextModels = [...catalogModels, newModel];
    setCatalogModels(nextModels);

    // If defaultModelKey is empty, set it to the first model's key
    const currentDefault = form.getFieldValue('defaultModelKey');
    if (!currentDefault) {
      form.setFieldsValue({ defaultModelKey: newModel.key });
    }
  };

  // Update model row
  const handleUpdateModel = (
    index: number,
    field: keyof KimiCatalogModel,
    val: string | number | undefined,
  ) => {
    const nextModels = [...catalogModels];
    const target = { ...nextModels[index], [field]: val };
    nextModels[index] = target;
    setCatalogModels(nextModels);

    // If the changed key was the selected defaultModelKey, keep them in sync
    if (field === 'key') {
      const oldKey = catalogModels[index].key;
      const currentDefault = form.getFieldValue('defaultModelKey');
      if (currentDefault === oldKey && typeof val === 'string') {
        form.setFieldsValue({ defaultModelKey: val });
      }
    }
  };

  // Update model row numeric context size (empty input clears the override;
  // projection falls back to 262144 so the CLI still accepts the model).
  const handleUpdateModelContextSize = (index: number, raw: string) => {
    const trimmed = raw.trim();
    const parsed = trimmed === '' ? undefined : Number.parseInt(trimmed, 10);
    handleUpdateModel(index, 'maxContextSize', Number.isFinite(parsed) ? parsed : undefined);
  };

  // Delete model row
  const handleDeleteModel = (index: number) => {
    const deletedKey = catalogModels[index]?.key;
    const nextModels = catalogModels.filter((_, idx) => idx !== index);
    setCatalogModels(nextModels);

    // If deleted model was selected as defaultModelKey, update to next available or clear
    const currentDefault = form.getFieldValue('defaultModelKey');
    if (currentDefault === deletedKey) {
      form.setFieldsValue({
        defaultModelKey: nextModels.length > 0 ? nextModels[0].key : '',
      });
    }
  };

  // Options for defaultModelKey select
  const defaultModelOptions = useMemo(() => {
    const options = catalogModels
      .filter((m) => m.key && m.key.trim())
      .map((m) => ({
        label: m.displayName ? `${m.displayName} (${m.key})` : m.key,
        value: m.key,
      }));

    // Include the current form value if not in the list
    const currentVal = form.getFieldValue('defaultModelKey');
    if (currentVal && !options.some((opt) => opt.value === currentVal)) {
      options.unshift({
        label: currentVal,
        value: currentVal,
      });
    }

    return options;
  }, [catalogModels, form]);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();

      // Pre-validation for custom provider:
      // Backend constraint: non-official provider with defaultModelKey must have non-empty modelCatalog.models
      const selectedCategory = values.category || category;

      // When the advanced JSON panel is open, rawJson is the freshest source:
      // reconcile it into the structured inputs first so these validations and
      // the build below cannot silently overwrite raw-JSON edits with stale
      // form values (same reconciliation as collapsing the panel).
      let reconciled = {
        apiKey: values.apiKey ?? '',
        baseUrl: values.baseUrl ?? '',
        defaultModelKey: values.defaultModelKey ?? '',
        providerKey,
        catalogModels,
        customTomlConfig,
        rawObject,
      };
      if (advancedExpanded && rawJson.trim()) {
        const parsed = applyRawJsonToForm(rawJson);
        if (parsed.parseError) {
          message.error(t('kimi.providerForm.invalidJsonPrompt'));
          return;
        }
        reconciled = {
          apiKey: parsed.apiKey,
          baseUrl: parsed.baseUrl,
          defaultModelKey: parsed.defaultModelKey,
          providerKey: parsed.providerKey || CUSTOM_KIMI_PROVIDER_KEY,
          catalogModels: parsed.catalogModels,
          // Parsed value is authoritative: a deleted `config` key must stay
          // deleted instead of being resurrected from the previous state.
          customTomlConfig: parsed.customTomlConfig,
          rawObject: parsed.rawObject,
        };
      }

      const trimmedDefaultModel = reconciled.defaultModelKey.trim();

      if (selectedCategory !== 'official' && trimmedDefaultModel) {
        const validModels = reconciled.catalogModels.filter((m) => m.key?.trim() && m.model?.trim());
        if (validModels.length === 0) {
          message.error(t('kimi.providerForm.modelListRequiredForDefaultModel'));
          return;
        }
      }

      // Check if any model rows are incomplete (have key but missing model, or vice versa)
      if (selectedCategory !== 'official' && reconciled.catalogModels.length > 0) {
        const hasIncompleteRow = reconciled.catalogModels.some(
          (m) => (!m.key?.trim() && m.model?.trim()) || (m.key?.trim() && !m.model?.trim()),
        );
        if (hasIncompleteRow) {
          message.error(t('kimi.providerForm.modelRowIncomplete'));
          return;
        }
      }

      setLoading(true);

      const settingsConfigStr = buildKimiSettingsConfig({
        category: selectedCategory,
        apiKey: reconciled.apiKey,
        baseUrl: reconciled.baseUrl,
        providerKey: reconciled.providerKey,
        defaultModelKey: reconciled.defaultModelKey,
        catalogModels: reconciled.catalogModels,
        customTomlConfig: reconciled.customTomlConfig,
        rawObject: reconciled.rawObject,
      });

      // Official providers always go through the real Kimi channel, so the
      // gateway billing/header overrides are meaningless for them.
      const isOfficialCategory = selectedCategory === 'official';
      const meta = mergeModelRewritesIntoMeta(
        mergeCustomHeadersIntoMeta(
          mergeBillingConfigIntoMeta(provider?.meta, isOfficialCategory
            ? { enabled: false, pricingModelSource: 'inherit' }
            : billingConfig),
          isOfficialCategory
            ? { enabled: false, headers: [] }
            : customHeaders,
        ),
        isOfficialCategory
          ? { enabled: false, rewrites: [] }
          : modelRewrites,
      );

      await onSubmit({
        name: values.name.trim(),
        category: selectedCategory,
        notes: values.notes?.trim() || undefined,
        settingsConfig: settingsConfigStr,
        meta,
      });

      onCancel();
    } catch (err) {
      if (err && typeof err === 'object' && 'errorFields' in err) {
        // Form validation error, do nothing
        return;
      }
      message.error(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const modelColumns = [
    {
      title: (
        <span>
          {t('kimi.providerForm.modelCatalogKey')}
          <span style={{ color: 'var(--color-status-error)', marginLeft: 4 }}>*</span>
        </span>
      ),
      dataIndex: 'key',
      key: 'key',
      width: '28%',
      render: (_: unknown, record: KimiCatalogModel, index: number) => (
        <Input
          size="small"
          value={record.key}
          placeholder="e.g. kimi-code/kimi-for-coding"
          onChange={(e) => handleUpdateModel(index, 'key', e.target.value)}
        />
      ),
    },
    {
      title: (
        <span>
          {t('kimi.providerForm.modelCatalogModel')}
          <span style={{ color: 'var(--color-status-error)', marginLeft: 4 }}>*</span>
        </span>
      ),
      dataIndex: 'model',
      key: 'model',
      width: '28%',
      render: (_: unknown, record: KimiCatalogModel, index: number) => (
        <Input
          size="small"
          value={record.model}
          placeholder="e.g. kimi-for-coding"
          onChange={(e) => handleUpdateModel(index, 'model', e.target.value)}
        />
      ),
    },
    {
      title: (
        <span>
          {t('kimi.providerForm.modelCatalogContextSize')}
          <Tooltip title={t('kimi.providerForm.modelCatalogContextSizeTooltip')}>
            <InfoCircleOutlined style={{ marginLeft: 6, color: 'var(--color-text-tertiary)' }} />
          </Tooltip>
        </span>
      ),
      dataIndex: 'maxContextSize',
      key: 'maxContextSize',
      width: '22%',
      render: (_: unknown, record: KimiCatalogModel, index: number) => (
        <AutoComplete
          size="small"
          value={record.maxContextSize != null ? String(record.maxContextSize) : ''}
          placeholder="262144"
          options={CONTEXT_SIZE_PRESETS}
          filterOption={(input, option) =>
            String(option?.value ?? '').includes(input) ||
            String(option?.label ?? '').toLowerCase().includes(input.toLowerCase())
          }
          onChange={(value) => handleUpdateModelContextSize(index, String(value ?? ''))}
        />
      ),
    },
    {
      title: '',
      key: 'actions',
      width: '10%',
      align: 'center' as const,
      render: (_: unknown, __: KimiCatalogModel, index: number) => (
        <Popconfirm
          title={t('kimi.providerForm.deleteModelConfirm')}
          onConfirm={() => handleDeleteModel(index)}
          okText={t('common.confirm')}
          cancelText={t('common.cancel')}
        >
          <Button
            type="text"
            danger
            size="small"
            icon={<DeleteOutlined />}
          />
        </Popconfirm>
      ),
    },
  ];

  return (
    <Modal
      open={open}
      title={provider ? t('kimi.providerForm.editTitle') : t('kimi.providerForm.addTitle')}
      onCancel={onCancel}
      onOk={handleSubmit}
      confirmLoading={loading}
      width={720}
      destroyOnHidden
    >
      <Form
        form={form}
        layout="horizontal"
        labelCol={labelCol}
        wrapperCol={wrapperCol}
      >
        {/* Basic Fields */}
        <Form.Item
          name="name"
          label={t('kimi.providerForm.name')}
          rules={[{ required: true, message: t('kimi.providerForm.nameRequired') }]}
        >
          <Input placeholder="e.g. My Kimi Provider" />
        </Form.Item>

        <Form.Item
          name="category"
          label={t('kimi.providerForm.category')}
        >
          <Select
            onChange={(val: KimiProviderCategory) => setCategory(val)}
            options={[
              { label: t('kimi.providerForm.categoryCustom'), value: 'custom' },
              { label: t('kimi.providerForm.categoryOfficial'), value: 'official' },
            ]}
          />
        </Form.Item>

        {/* Official Provider Notice */}
        {isOfficial ? (
          <Form.Item wrapperCol={sectionWrapperCol}>
            <Alert
              type="info"
              showIcon
              message={t('kimi.providerForm.officialNoticeTitle')}
              description={t('kimi.providerForm.officialNoticeDesc')}
              style={{ marginBottom: 0 }}
            />
          </Form.Item>
        ) : (
          <>
            {/* Connection Fields */}
            <Form.Item
              name="apiKey"
              label={
                <span>
                  {t('kimi.providerForm.apiKey')}
                  <Tooltip title={t('kimi.providerForm.apiKeyTooltip')}>
                    <InfoCircleOutlined style={{ marginLeft: 6, color: 'var(--color-text-tertiary)' }} />
                  </Tooltip>
                </span>
              }
              rules={[{ required: true, message: t('kimi.providerForm.apiKeyRequired') }]}
            >
              <Input.Password placeholder="sk-..." />
            </Form.Item>

            <Form.Item
              name="baseUrl"
              label={
                <span>
                  {t('kimi.providerForm.baseUrl')}
                  <Tooltip title={t('kimi.providerForm.baseUrlTooltip')}>
                    <InfoCircleOutlined style={{ marginLeft: 6, color: 'var(--color-text-tertiary)' }} />
                  </Tooltip>
                </span>
              }
              rules={[{ required: true, message: t('kimi.providerForm.baseUrlRequired') }]}
            >
              <Input placeholder="https://api.example.com/v1" />
            </Form.Item>

            {/* Default Model Selection */}
            <Form.Item
              name="defaultModelKey"
              label={
                <span>
                  {t('kimi.providerForm.defaultModelKey')}
                  <Tooltip title={t('kimi.providerForm.defaultModelKeyTooltip')}>
                    <InfoCircleOutlined style={{ marginLeft: 6, color: 'var(--color-text-tertiary)' }} />
                  </Tooltip>
                </span>
              }
            >
              <Select
                showSearch
                placeholder={t('kimi.providerForm.defaultModelKeyPlaceholder')}
                options={defaultModelOptions}
              />
            </Form.Item>

            {/* Model Catalog Table */}
            <Form.Item wrapperCol={sectionWrapperCol}>
              <div className={styles.sectionHeader}>
                <span className={styles.sectionTitle}>
                  {t('kimi.providerForm.modelCatalog')}
                </span>
                <Button
                  type="dashed"
                  size="small"
                  icon={<PlusOutlined />}
                  onClick={handleAddModel}
                >
                  {t('kimi.providerForm.addModel')}
                </Button>
              </div>

              <div className={styles.modelsTableWrapper}>
                <Table
                  dataSource={catalogModels}
                  columns={modelColumns}
                  rowKey={(record, index) => `${record.key || ''}_${index}`}
                  pagination={false}
                  size="small"
                  locale={{ emptyText: t('kimi.providerForm.noModelsConfigured') }}
                />
              </div>
            </Form.Item>
          </>
        )}

        {/* Advanced JSON Section */}
        <Form.Item wrapperCol={sectionWrapperCol}>
          <ProviderConfigCollapse
            title={t('kimi.providerForm.advancedSettings')}
            expanded={advancedExpanded}
            onExpandedChange={handleAdvancedExpandedChange}
            icon={<FileCode2 />}
          >
            <p className={styles.advancedHint}>
              {t('kimi.providerForm.advancedSettingsDesc')}
            </p>
            <JsonEditor
              value={rawJson}
              onRawChange={setRawJson}
              mode="text"
              height={180}
              minHeight={140}
              maxHeight={360}
              resizable
            />
          </ProviderConfigCollapse>
        </Form.Item>

        {/* Gateway billing / custom header overrides (custom providers only) */}
        {!isOfficial && (
          <>
            <Form.Item wrapperCol={sectionWrapperCol}>
              <BillingConfigCollapse
                value={billingConfig}
                onChange={setBillingConfig}
              />
            </Form.Item>

            <Form.Item wrapperCol={sectionWrapperCol}>
              <CustomHeadersCollapse
                value={customHeaders}
                onChange={setCustomHeaders}
              />
            </Form.Item>

            <Form.Item wrapperCol={sectionWrapperCol}>
              <ModelRewritesCollapse
                value={modelRewrites}
                onChange={setModelRewrites}
              />
            </Form.Item>
          </>
        )}

        {/* Notes */}
        <Form.Item name="notes" wrapperCol={sectionWrapperCol} style={{ marginBottom: 0 }}>
          <ProviderNotesCollapse
            title={t('kimi.providerForm.notes')}
            placeholder={t('kimi.providerForm.notesPlaceholder')}
            rows={2}
            resetKey={notesCollapseResetKey}
          />
        </Form.Item>
      </Form>
    </Modal>
  );
};

export default KimiProviderFormModal;
