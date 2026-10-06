import React, { useState, useEffect, useMemo, useCallback } from 'react';
import {
  Modal,
  Form,
  Input,
  Select,
  Alert,
  Button,
  Tooltip,
  Typography,
  message,
} from 'antd';
import { InfoCircleOutlined } from '@ant-design/icons';
import { FileCode2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import JsonEditor from '@/components/common/JsonEditor';
import type {
  KimiProvider,
  KimiProviderFormData,
  KimiCatalogModel,
  KimiPresetModel,
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
import { buildKimiCatalogModelsFromPresets } from '../utils/kimiCatalogModels';
import { getKimiPresetModels } from '@/services/kimiApi';
import styles from './KimiProviderFormModal.module.less';

const { Text } = Typography;

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

  // Bundled models.dev catalog models matching the entered base URL. Empty
  // means "no match", which shows no hint rather than a wrong one.
  const [presetModels, setPresetModels] = useState<KimiPresetModel[]>([]);
  // Guards the blur lookup: overlapping responses must not let an older
  // provider's matches win, and a reset must invalidate any in-flight request.
  const presetRequestIdRef = React.useRef(0);

  const isOfficial = category === 'official';

  const notesCollapseResetKey = `${open ? 'open' : 'closed'}:${provider?.id ?? 'new'}`;

  // Initialize form data when modal opens or provider changes
  useEffect(() => {
    if (!open) return;

    // Drop any in-flight preset lookup and its result: the hint must never
    // carry over from the previously edited provider.
    presetRequestIdRef.current += 1;
    setPresetModels([]);

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

  /**
   * Look up the bundled models.dev catalog for the entered base URL. Runs on
   * blur so typing does not fire a request per keystroke; a miss clears the
   * hint instead of keeping a stale match.
   */
  const handleBaseUrlBlur = async (rawValue: string) => {
    const baseUrl = rawValue.trim();
    const requestId = ++presetRequestIdRef.current;
    if (!baseUrl) {
      setPresetModels([]);
      return;
    }
    try {
      const models = await getKimiPresetModels(baseUrl);
      // Two quick blurs can resolve out of order; only the newest wins.
      if (requestId === presetRequestIdRef.current) {
        setPresetModels(models);
      }
    } catch {
      // A lookup failure must never block manual entry.
      if (requestId === presetRequestIdRef.current) {
        setPresetModels([]);
      }
    }
  };

  const handleImportPresetModels = () => {
    const imported = buildKimiCatalogModelsFromPresets(
      presetModels,
      providerKey || CUSTOM_KIMI_PROVIDER_KEY,
      catalogModels,
      KIMI_OFFICIAL_DEFAULT_MODEL_MAX_CONTEXT_SIZE,
    );
    if (imported.length === 0) {
      message.info(t('kimi.providerForm.presetNoneNew'));
      return;
    }
    const nextModels = [...catalogModels, ...imported];
    setCatalogModels(nextModels);
    if (!form.getFieldValue('defaultModelKey') && nextModels[0]) {
      form.setFieldsValue({ defaultModelKey: nextModels[0].key });
    }
    message.success(t('kimi.providerForm.presetImported', { count: imported.length }));
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
              <Input
                placeholder="https://api.example.com/v1"
                onBlur={(event) => void handleBaseUrlBlur(event.target.value)}
              />
            </Form.Item>

            {/* Bundled-catalog preset hint (custom providers only). */}
            {!isOfficial && presetModels.length > 0 && (
              <Form.Item wrapperCol={sectionWrapperCol}>
                <Alert
                  type="info"
                  showIcon
                  message={t('kimi.providerForm.presetMatch', { count: presetModels.length })}
                  action={
                    <Button size="small" onClick={handleImportPresetModels}>
                      {t('kimi.providerForm.presetImport')}
                    </Button>
                  }
                />
              </Form.Item>
            )}

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

            {/* The model catalog is edited on the provider card's model
                section, not here — a full catalog form would dwarf the
                provider fields and the card list is the natural home for
                per-model actions (see KimiModelFormModal). */}
            <Form.Item wrapperCol={sectionWrapperCol}>
              <Text type="secondary" style={{ fontSize: 12 }}>
                {t('kimi.model.catalogMovedHint')}
              </Text>
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
