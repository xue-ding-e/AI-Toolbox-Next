import React from 'react';
import {
  Checkbox,
  Divider,
  Form,
  Input,
  InputNumber,
  Modal,
  Radio,
  Select,
  Space,
  Tag,
  Typography,
} from 'antd';
import { useTranslation } from 'react-i18next';
import {
  PRESET_MODELS,
  getPresetModelsVersion,
  subscribePresetModels,
  type PresetModel,
} from '@/constants/presetModels';
import type {
  ZcodeModelOptionSpecs,
  ZcodeModelProperties,
  ZcodeModelRow,
} from '@/types/zcode';
import {
  ZCODE_MODALITY_FIELDS,
  buildInputFormat,
  compactObject,
  preferredPresetNpmTypes,
  presetInputModalities,
  readInputModalities,
  zcodeModalityValuesFor,
} from '../utils/zcodeModelFields';

const { Text } = Typography;

/**
 * ZCode's own reasoning levels, ordered low to high — the same list its desktop
 * editor offers. Users may still type custom levels, so the field is a tags
 * select rather than a fixed multi-select.
 */
const ZCODE_REASONING_LEVEL_PRESETS = ['none', 'low', 'medium', 'high', 'xhigh', 'max'];

interface ZcodeModelFormValues {
  modelId: string;
  displayName?: string;
  ruleKind: 'smart' | 'manual';
  enabled: boolean;
  contextWindow?: number;
  /** Selected modalities; an empty list leaves ZCode's catalog to decide. */
  inputModalities: string[];
  supportsToolCall: boolean;
  supportsJsonSchemaOutput: boolean;
  supportsNativeWebSearch: boolean;
  supportsMidConversationSystem: boolean;
  requiresMfjsToolSchema: boolean;
  maxOutputTokensMax?: number;
  maxOutputTokensMap?: string;
  reasoningLevels?: string[];
  reasoningLevelMap?: string;
}

interface ZcodeModelFormModalProps {
  open: boolean;
  isEdit: boolean;
  initialValues?: ZcodeModelRow;
  /**
   * The owning provider's API format (`config.api.type`), used to pick which
   * preset catalog to show first. Presets are how a user fills a model's
   * parameters in one click instead of typing a dozen fields from memory.
   */
  apiType?: string;
  onCancel: () => void;
  onSubmit: (model: ZcodeModelRow) => void | Promise<void>;
}

const toFormValues = (row: ZcodeModelRow | undefined): Partial<ZcodeModelFormValues> => {
  if (!row) {
    return { ruleKind: 'smart', enabled: true };
  }
  const properties: ZcodeModelProperties = row.properties ?? {};
  const optionSpecs: ZcodeModelOptionSpecs = row.optionSpecs ?? {};
  return {
    modelId: row.modelId,
    displayName: row.displayName,
    ruleKind: row.ruleKind,
    // ZCode treats an absent key as enabled, so only an explicit `false` reads
    // as off.
    enabled: row.enabled !== false,
    contextWindow: properties.contextWindow,
    inputModalities: readInputModalities(properties.inputFormat ?? {}),
    // Capabilities are plain switches: unchecked leaves the key unwritten, so
    // ZCode's catalog keeps deciding; checked asserts the model has it.
    supportsToolCall: properties.supportsToolCall === true,
    supportsJsonSchemaOutput: properties.supportsJsonSchemaOutput === true,
    supportsNativeWebSearch: properties.supportsNativeWebSearch === true,
    supportsMidConversationSystem: properties.supportsMidConversationSystem === true,
    requiresMfjsToolSchema: properties.requiresMfjsToolSchema === true,
    maxOutputTokensMax: optionSpecs.maxOutputTokens?.max,
    maxOutputTokensMap: optionSpecs.maxOutputTokens?.map,
    reasoningLevels: optionSpecs.reasoningLevel?.values,
    reasoningLevelMap: optionSpecs.reasoningLevel?.map,
  };
};

const toModelRow = (
  values: ZcodeModelFormValues,
  initialValues: ZcodeModelRow | undefined,
): ZcodeModelRow => {
  const properties = compactObject<ZcodeModelProperties>({
    contextWindow: values.contextWindow,
    inputFormat: buildInputFormat(values.inputModalities ?? []),
    // A ticked box writes `true`; an unticked one writes nothing, leaving the
    // value to ZCode's catalog.
    supportsToolCall: values.supportsToolCall || undefined,
    supportsJsonSchemaOutput: values.supportsJsonSchemaOutput || undefined,
    supportsNativeWebSearch: values.supportsNativeWebSearch || undefined,
    supportsMidConversationSystem: values.supportsMidConversationSystem || undefined,
    requiresMfjsToolSchema: values.requiresMfjsToolSchema || undefined,
  });

  const optionSpecs = compactObject<ZcodeModelOptionSpecs>({
    maxOutputTokens: compactObject({
      max: values.maxOutputTokensMax,
      map: values.maxOutputTokensMap?.trim() || undefined,
    }),
    reasoningLevel: compactObject({
      values:
        values.reasoningLevels && values.reasoningLevels.length > 0
          ? values.reasoningLevels
          : undefined,
      map: values.reasoningLevelMap?.trim() || undefined,
    }),
  });

  return {
    modelId: values.modelId.trim(),
    displayName: values.displayName?.trim() || undefined,
    ruleKind: values.ruleKind,
    enabled: values.enabled !== false,
    properties,
    optionSpecs,
    isDefault: initialValues?.isDefault ?? false,
  };
};

/**
 * Model editor for one ZCode provider — the AI Toolbox counterpart of ZCode's
 * own "编辑模型配置" dialog. Writes into the provider's `settings_config.models`
 * array; the backend projects each row into ZCode's model rules.
 */
const ZcodeModelFormModal: React.FC<ZcodeModelFormModalProps> = ({
  open,
  isEdit,
  initialValues,
  apiType,
  onCancel,
  onSubmit,
}) => {
  const { t, i18n } = useTranslation();
  const [form] = Form.useForm<ZcodeModelFormValues>();
  // Labels sit in a left column, as in every other model form (the shared
  // `ModelFormModal` and the four module-local ones). Spans match the shared
  // default — the longest label here is 最大输出参数映射, which fits.
  const labelCol = { span: i18n.language === 'zh-CN' ? 6 : 8 };
  const wrapperCol = { span: i18n.language === 'zh-CN' ? 18 : 16 };
  const [submitting, setSubmitting] = React.useState(false);
  const [presetsExpanded, setPresetsExpanded] = React.useState(false);
  const ruleKind = Form.useWatch('ruleKind', form) as 'smart' | 'manual' | undefined;
  const reasoningLevels = Form.useWatch('reasoningLevels', form) as string[] | undefined;
  const isManual = ruleKind === 'manual';
  const presetModelsVersion = React.useSyncExternalStore(
    subscribePresetModels,
    getPresetModelsVersion,
    getPresetModelsVersion,
  );

  // Protocol-matched presets first; the rest stay reachable below the divider.
  const { primaryPresets, otherPresets } = React.useMemo(() => {
    const primaryNpmTypes = preferredPresetNpmTypes(apiType);
    const primaryNpmSet = new Set(primaryNpmTypes);
    const primary: PresetModel[] = [];
    const other: PresetModel[] = [];
    const seen = new Set<string>();

    const pushUnique = (target: PresetModel[], preset: PresetModel) => {
      const id = preset.id?.trim();
      if (!id || seen.has(id)) {
        return;
      }
      seen.add(id);
      target.push(preset);
    };

    primaryNpmTypes.forEach((npmType) => {
      (PRESET_MODELS[npmType] || []).forEach((preset) => pushUnique(primary, preset));
    });
    Object.entries(PRESET_MODELS).forEach(([npmType, models]) => {
      if (primaryNpmSet.has(npmType)) {
        return;
      }
      models.forEach((preset) => pushUnique(other, preset));
    });

    return { primaryPresets: primary, otherPresets: other };
  }, [apiType, presetModelsVersion]);

  const modalityOptions = React.useMemo(() => {
    // The manual schema accepts fewer modalities than the smart one; the field
    // table knows the difference.
    const values = new Set(zcodeModalityValuesFor(isManual ? 'manual' : 'smart'));
    return ZCODE_MODALITY_FIELDS.filter((field) => values.has(field.value)).map((field) => ({
      value: field.value,
      label: t(field.labelKey),
    }));
  }, [isManual, t]);

  /**
   * Fills the form from a preset.
   *
   * Only fields ZCode actually stores are written; the preset's cost, options
   * and variants have no ZCode counterpart and are dropped rather than forced
   * into a shape the CLI would reject. The model ID is left alone when editing —
   * changing it would orphan the row.
   */
  const handlePresetSelect = (preset: PresetModel) => {
    form.setFieldsValue({
      ...(isEdit ? {} : { modelId: preset.id }),
      displayName: preset.name,
      contextWindow: preset.contextLimit,
      maxOutputTokensMax: preset.outputLimit,
      // Modalities this mode cannot carry are dropped rather than shown and
      // then silently discarded on save.
      inputModalities: presetInputModalities(preset, isManual ? 'manual' : 'smart'),
      ...(preset.tool_call === undefined ? {} : { supportsToolCall: preset.tool_call }),
    });
  };

  React.useEffect(() => {
    if (!open) {
      return;
    }
    form.resetFields();
    form.setFieldsValue({
      ruleKind: 'smart',
      enabled: true,
      inputModalities: [],
      supportsToolCall: false,
      supportsJsonSchemaOutput: false,
      supportsNativeWebSearch: false,
      supportsMidConversationSystem: false,
      requiresMfjsToolSchema: false,
      ...toFormValues(initialValues),
    });
  }, [form, initialValues, open]);

  const handleOk = async () => {
    try {
      const values = await form.validateFields();
      setSubmitting(true);
      await onSubmit(toModelRow(values, initialValues));
    } finally {
      setSubmitting(false);
    }
  };

  /**
   * Manual rows must spell every field out — ZCode validates them strictly and
   * refuses the whole file when one is missing.
   */
  const manualRequiredRule = isManual
    ? [{ required: true, message: t('common.error') }]
    : undefined;

  /**
   * Capability switches, as checkboxes. `tool_call` and `requiresMfjsToolSchema`
   * are smart-only: ZCode's manual rule schema does not declare them, so they are
   * hidden rather than silently dropped on save.
   */
  const capabilityFields: Array<{ name: keyof ZcodeModelFormValues; labelKey: string }> = [
    ...(isManual
      ? []
      : [{ name: 'supportsToolCall' as const, labelKey: 'zcode.model.supportsToolCall' }]),
    { name: 'supportsJsonSchemaOutput', labelKey: 'zcode.model.supportsJsonSchemaOutput' },
    { name: 'supportsNativeWebSearch', labelKey: 'zcode.model.supportsNativeWebSearch' },
    { name: 'supportsMidConversationSystem', labelKey: 'zcode.model.supportsMidConversationSystem' },
    ...(isManual
      ? []
      : [
          {
            name: 'requiresMfjsToolSchema' as const,
            labelKey: 'zcode.model.requiresMfjsToolSchema',
          },
        ]),
  ];

  return (
    <Modal
      open={open}
      title={isEdit ? t('zcode.model.editTitle') : t('zcode.model.addTitle')}
      onCancel={onCancel}
      onOk={() => void handleOk()}
      confirmLoading={submitting}
      destroyOnHidden
      width={680}
      okText={t('common.save')}
      cancelText={t('common.cancel')}
    >
      <Form form={form} layout="horizontal" labelCol={labelCol} wrapperCol={wrapperCol}>
        <Form.Item label={t('zcode.model.modelId')} required>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            <Form.Item
              name="modelId"
              noStyle
              rules={[{ required: true, message: t('common.required') }]}
            >
              <Input placeholder="deepseek-chat" disabled={isEdit} style={{ flex: 1 }} />
            </Form.Item>
            {primaryPresets.length + otherPresets.length > 0 && (
              <a
                style={{
                  flexShrink: 0,
                  fontSize: 12,
                  fontWeight: 500,
                  color: 'var(--ant-color-text-secondary)',
                  cursor: 'pointer',
                  userSelect: 'none',
                  whiteSpace: 'nowrap',
                }}
                onClick={() => setPresetsExpanded(!presetsExpanded)}
              >
                {t('common.model.selectPreset')}
                {presetsExpanded ? ' ▴' : ' ▾'}
              </a>
            )}
          </div>
        </Form.Item>

        {presetsExpanded && (
          <Form.Item
            wrapperCol={{ offset: labelCol.span, span: wrapperCol.span }}
            style={{ marginTop: -8 }}
          >
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
              {primaryPresets.map((preset) => (
                <Tag
                  key={preset.id}
                  style={{ cursor: 'pointer', transition: 'all 0.2s' }}
                  onClick={() => handlePresetSelect(preset)}
                >
                  {preset.name}
                </Tag>
              ))}
            </div>
            {otherPresets.length > 0 && (
              <>
                <Divider
                  style={{ margin: '12px 0', fontSize: 12, color: 'var(--color-text-tertiary)' }}
                >
                  {t('common.model.otherPresets')}
                </Divider>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
                  {otherPresets.map((preset) => (
                    <Tag
                      key={preset.id}
                      style={{ cursor: 'pointer', transition: 'all 0.2s' }}
                      onClick={() => handlePresetSelect(preset)}
                    >
                      {preset.name}
                    </Tag>
                  ))}
                </div>
              </>
            )}
          </Form.Item>
        )}

        <Form.Item name="displayName" label={t('zcode.model.displayName')}>
          <Input placeholder={t('zcode.model.displayNamePlaceholder')} />
        </Form.Item>

        <Form.Item
          name="ruleKind"
          label={t('zcode.model.ruleKind')}
          extra={
            <Text type="secondary" style={{ fontSize: 12 }}>
              {t('zcode.model.ruleKindHint')}
            </Text>
          }
        >
          <Radio.Group>
            <Radio.Button value="smart">{t('zcode.model.ruleKindSmart')}</Radio.Button>
            <Radio.Button value="manual">{t('zcode.model.ruleKindManual')}</Radio.Button>
          </Radio.Group>
        </Form.Item>

        <Form.Item
          name="enabled"
          label={t('zcode.model.enabledLabel')}
          valuePropName="checked"
          extra={
            <Text type="secondary" style={{ fontSize: 12 }}>
              {t('zcode.model.enabledHint')}
            </Text>
          }
        >
          <Checkbox />
        </Form.Item>

        <Form.Item
          name="contextWindow"
          label={t('zcode.model.contextWindow')}
          rules={manualRequiredRule}
        >
          <InputNumber
            min={1}
            style={{ width: '100%' }}
            placeholder={t('zcode.model.contextWindowPlaceholder')}
          />
        </Form.Item>

        <Form.Item
          name="maxOutputTokensMax"
          label={t('zcode.model.maxOutputTokens')}
          rules={manualRequiredRule}
        >
          <InputNumber
            min={1}
            style={{ width: '100%' }}
            placeholder={t('zcode.model.maxOutputTokensPlaceholder')}
          />
        </Form.Item>

        <Form.Item
          name="inputModalities"
          label={t('zcode.model.inputModalities')}
          extra={
            <Text type="secondary" style={{ fontSize: 12 }}>
              {t('zcode.model.inputModalitiesHint')}
            </Text>
          }
        >
          <Select
            mode="multiple"
            allowClear
            placeholder={t('zcode.model.inputModalitiesPlaceholder')}
            options={modalityOptions}
          />
        </Form.Item>

        <Form.Item
          label={t('zcode.model.capabilities')}
          extra={
            <Text type="secondary" style={{ fontSize: 12 }}>
              {t('zcode.model.capabilitiesHint')}
            </Text>
          }
        >
          <Space wrap size="small">
            {capabilityFields.map((field) => (
              <Form.Item key={field.name} name={field.name} valuePropName="checked" noStyle>
                <Checkbox>{t(field.labelKey)}</Checkbox>
              </Form.Item>
            ))}
          </Space>
        </Form.Item>

        <Form.Item
          name="reasoningLevels"
          label={t('zcode.model.reasoningLevels')}
          rules={manualRequiredRule}
          extra={
            <Text type="secondary" style={{ fontSize: 12 }}>
              {t('zcode.model.reasoningLevelsHint')}
            </Text>
          }
        >
          <Select
            mode="tags"
            allowClear
            placeholder={t('zcode.model.reasoningLevelsPlaceholder')}
            options={ZCODE_REASONING_LEVEL_PRESETS.map((level) => ({ value: level, label: level }))}
          />
        </Form.Item>

        <Form.Item
          name="reasoningLevelMap"
          label={t('zcode.model.reasoningLevelMapping')}
          rules={manualRequiredRule}
          extra={
            <Text type="secondary" style={{ fontSize: 12 }}>
              {t('zcode.model.mapHint')}
            </Text>
          }
        >
          <Input
            placeholder={'{"reasoning_effort": reasoningLevel}'}
            disabled={!reasoningLevels || reasoningLevels.length === 0}
          />
        </Form.Item>

        {/* Smart-only: ZCode's manual rule schema does not declare this map. */}
        {!isManual && (
          <Form.Item
            name="maxOutputTokensMap"
            label={t('zcode.model.maxOutputTokensMapping')}
            extra={
              <Text type="secondary" style={{ fontSize: 12 }}>
                {t('zcode.model.mapHint')}
              </Text>
            }
          >
            <Input placeholder={'{"max_tokens": maxOutputTokens}'} />
          </Form.Item>
        )}
      </Form>
    </Modal>
  );
};

export default ZcodeModelFormModal;