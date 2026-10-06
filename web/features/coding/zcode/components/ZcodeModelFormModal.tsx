import React from 'react';
import { Divider, Form, Input, InputNumber, Modal, Radio, Select, Space, Tag, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import {
  PRESET_MODELS,
  getPresetModelsVersion,
  subscribePresetModels,
  type PresetModel,
} from '@/constants/presetModels';
import type {
  ZcodeModelInputFormat,
  ZcodeModelOptionSpecs,
  ZcodeModelProperties,
  ZcodeModelRow,
} from '@/types/zcode';

const { Text } = Typography;

/**
 * ZCode's own reasoning levels, ordered low to high — the same list its desktop
 * editor offers. Users may still type custom levels, so the field is a tags
 * select rather than a fixed multi-select.
 */
const ZCODE_REASONING_LEVEL_PRESETS = ['none', 'low', 'medium', 'high', 'xhigh', 'max'];

/**
 * Tri-state for capability switches.
 *
 * `undefined` means "do not write the key" — ZCode then inherits the value from
 * its built-in catalog. An explicit `false` pins the capability off, which is
 * different from leaving it unset, so the form must be able to express all
 * three states.
 */
type TriState = 'inherit' | 'on' | 'off';

const INHERIT = 'inherit' as const;

const triStateOptions = (t: (key: string, options?: Record<string, unknown>) => string) => [
  { value: 'inherit', label: t('zcode.model.inherit', { defaultValue: '继承默认' }) },
  { value: 'on', label: t('zcode.model.enabled', { defaultValue: '启用' }) },
  { value: 'off', label: t('zcode.model.disabled', { defaultValue: '禁用' }) },
];

const toTriState = (value: boolean | undefined): TriState =>
  value === undefined ? INHERIT : value ? 'on' : 'off';

const fromTriState = (value: TriState | undefined): boolean | undefined =>
  value === 'on' ? true : value === 'off' ? false : undefined;

interface ZcodeModelFormValues {
  modelId: string;
  displayName?: string;
  ruleKind: 'smart' | 'manual';
  enabled: TriState;
  contextWindow?: number;
  supportsText: TriState;
  supportsImage: TriState;
  supportsVideo: TriState;
  supportsAudio: TriState;
  supportsPdf: TriState;
  supportsToolCall: TriState;
  supportsJsonSchemaOutput: TriState;
  supportsNativeWebSearch: TriState;
  supportsMidConversationSystem: TriState;
  requiresMfjsToolSchema: TriState;
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

/**
 * Preset catalogs to show first, most specific first.
 *
 * ZCode's three formats line up with the AI SDK provider families the preset
 * data is grouped by, so a DeepSeek model appears under the Anthropic entry
 * when the provider speaks `anthropic-messages`. Everything else still shows,
 * below the divider.
 */
const ZCODE_PRIMARY_PRESET_NPM_TYPES: Record<string, string[]> = {
  'anthropic-messages': ['@ai-sdk/anthropic'],
  'openai-chat-completions': ['@ai-sdk/openai', '@ai-sdk/openai-compatible'],
  'openai-responses': ['@ai-sdk/openai'],
};

const toFormValues = (row: ZcodeModelRow | undefined): Partial<ZcodeModelFormValues> => {
  if (!row) {
    return { ruleKind: 'smart' };
  }
  const properties: ZcodeModelProperties = row.properties ?? {};
  const inputFormat: ZcodeModelInputFormat = properties.inputFormat ?? {};
  const optionSpecs: ZcodeModelOptionSpecs = row.optionSpecs ?? {};
  return {
    modelId: row.modelId,
    displayName: row.displayName,
    ruleKind: row.ruleKind,
    enabled: toTriState(row.enabled),
    contextWindow: properties.contextWindow,
    supportsText: toTriState(inputFormat.supportsText),
    supportsImage: toTriState(inputFormat.supportsImage),
    supportsVideo: toTriState(inputFormat.supportsVideo),
    supportsAudio: toTriState(inputFormat.supportsAudio),
    supportsPdf: toTriState(inputFormat.supportsPdf),
    supportsToolCall: toTriState(properties.supportsToolCall),
    supportsJsonSchemaOutput: toTriState(properties.supportsJsonSchemaOutput),
    supportsNativeWebSearch: toTriState(properties.supportsNativeWebSearch),
    supportsMidConversationSystem: toTriState(properties.supportsMidConversationSystem),
    requiresMfjsToolSchema: toTriState(properties.requiresMfjsToolSchema),
    maxOutputTokensMax: optionSpecs.maxOutputTokens?.max,
    maxOutputTokensMap: optionSpecs.maxOutputTokens?.map,
    reasoningLevels: optionSpecs.reasoningLevel?.values,
    reasoningLevelMap: optionSpecs.reasoningLevel?.map,
  };
};

/**
 * Drops undefined keys so the projection layer can distinguish "inherit from
 * ZCode's catalog" from "explicitly off".
 */
const compactObject = <T extends object>(value: T): T | undefined => {
  const entries = Object.entries(value).filter(([, item]) => item !== undefined);
  return entries.length > 0 ? (Object.fromEntries(entries) as T) : undefined;
};

const toModelRow = (
  values: ZcodeModelFormValues,
  initialValues: ZcodeModelRow | undefined,
): ZcodeModelRow => {
  const properties = compactObject<ZcodeModelProperties>({
    contextWindow: values.contextWindow,
    inputFormat: compactObject<ZcodeModelInputFormat>({
      supportsText: fromTriState(values.supportsText),
      supportsImage: fromTriState(values.supportsImage),
      supportsVideo: fromTriState(values.supportsVideo),
      supportsAudio: fromTriState(values.supportsAudio),
      supportsPdf: fromTriState(values.supportsPdf),
    }),
    supportsToolCall: fromTriState(values.supportsToolCall),
    supportsJsonSchemaOutput: fromTriState(values.supportsJsonSchemaOutput),
    supportsNativeWebSearch: fromTriState(values.supportsNativeWebSearch),
    supportsMidConversationSystem: fromTriState(values.supportsMidConversationSystem),
    requiresMfjsToolSchema: fromTriState(values.requiresMfjsToolSchema),
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
    enabled: fromTriState(values.enabled),
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
  // `ModelFormModal` and the four module-local ones). ZCode's longest label is
  // 推理等级（从低到高）, so the column is a step wider than the shared default.
  const labelCol = { span: i18n.language === 'zh-CN' ? 7 : 9 };
  const wrapperCol = { span: i18n.language === 'zh-CN' ? 17 : 15 };
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
    const primaryNpmTypes = ZCODE_PRIMARY_PRESET_NPM_TYPES[apiType ?? ''] ?? [];
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

  /**
   * Fills the form from a preset.
   *
   * Only fields ZCode actually stores are written; the preset's cost, options
   * and variants have no ZCode counterpart and are dropped rather than forced
   * into a shape the CLI would reject. The model ID is left alone when editing —
   * changing it would orphan the row.
   */
  const handlePresetSelect = (preset: PresetModel) => {
    const inputModalities = preset.modalities?.input ?? [];
    form.setFieldsValue({
      ...(isEdit ? {} : { modelId: preset.id }),
      displayName: preset.name,
      contextWindow: preset.contextLimit,
      maxOutputTokensMax: preset.outputLimit,
      ...(inputModalities.length > 0
        ? {
            supportsText: inputModalities.includes('text') ? 'on' : 'off',
            supportsImage: inputModalities.includes('image') ? 'on' : 'off',
            supportsVideo: inputModalities.includes('video') ? 'on' : 'off',
            supportsAudio: inputModalities.includes('audio') ? 'on' : 'off',
            supportsPdf: inputModalities.includes('pdf') ? 'on' : 'off',
          }
        : {}),
      ...(preset.tool_call === undefined
        ? {}
        : { supportsToolCall: preset.tool_call ? 'on' : 'off' }),
    });
  };

  React.useEffect(() => {
    if (!open) {
      return;
    }
    form.resetFields();
    form.setFieldsValue({
      ruleKind: 'smart',
      enabled: INHERIT,
      supportsText: INHERIT,
      supportsImage: INHERIT,
      supportsVideo: INHERIT,
      supportsAudio: INHERIT,
      supportsPdf: INHERIT,
      supportsToolCall: INHERIT,
      supportsJsonSchemaOutput: INHERIT,
      supportsNativeWebSearch: INHERIT,
      supportsMidConversationSystem: INHERIT,
      requiresMfjsToolSchema: INHERIT,
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
    ? [{ required: true, message: t('common.error', { defaultValue: '请填写该项' }) }]
    : undefined;

  const capabilityFields: Array<{ name: keyof ZcodeModelFormValues; labelKey: string; fallback: string }> = [
    {
      name: 'supportsJsonSchemaOutput',
      labelKey: 'zcode.model.supportsJsonSchemaOutput',
      fallback: '结构化输出',
    },
    {
      name: 'supportsNativeWebSearch',
      labelKey: 'zcode.model.supportsNativeWebSearch',
      fallback: '原生联网搜索',
    },
    {
      name: 'supportsMidConversationSystem',
      labelKey: 'zcode.model.supportsMidConversationSystem',
      fallback: '对话中系统消息',
    },
    // `supportsToolCall` is smart-only: ZCode's manual rule schema does not
    // declare it, so it is hidden rather than silently dropped on save.
    ...(isManual
      ? []
      : [
          {
            name: 'supportsToolCall' as keyof ZcodeModelFormValues,
            labelKey: 'zcode.model.supportsToolCall',
            fallback: '工具调用',
          },
        ]),
  ];

  // `supportsText` and `supportsAudio` are smart-only for the same reason.
  const modalityFields: Array<{ name: keyof ZcodeModelFormValues; labelKey: string; fallback: string }> = [
    ...(isManual
      ? []
      : [
          { name: 'supportsText' as keyof ZcodeModelFormValues, labelKey: 'zcode.model.text', fallback: '文本' },
          { name: 'supportsAudio' as keyof ZcodeModelFormValues, labelKey: 'zcode.model.audio', fallback: '音频' },
        ]),
    { name: 'supportsImage', labelKey: 'zcode.model.image', fallback: '图片' },
    { name: 'supportsVideo', labelKey: 'zcode.model.video', fallback: '视频' },
    { name: 'supportsPdf', labelKey: 'zcode.model.pdf', fallback: 'PDF' },
  ];

  return (
    <Modal
      open={open}
      title={
        isEdit
          ? t('zcode.model.editTitle', { defaultValue: '编辑模型配置' })
          : t('zcode.model.addTitle', { defaultValue: '添加模型' })
      }
      onCancel={onCancel}
      onOk={() => void handleOk()}
      confirmLoading={submitting}
      destroyOnHidden
      width={680}
      okText={t('common.save', { defaultValue: '保存' })}
      cancelText={t('common.cancel', { defaultValue: '取消' })}
    >
      <Form form={form} layout="horizontal" labelCol={labelCol} wrapperCol={wrapperCol}>
        <Form.Item
          label={t('zcode.model.modelId')}
          required
        >
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
          <Form.Item wrapperCol={{ offset: labelCol.span, span: wrapperCol.span }} style={{ marginTop: -8 }}>
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
                <Divider style={{ margin: '12px 0', fontSize: 12, color: 'var(--color-text-tertiary)' }}>
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

        <Form.Item
          name="displayName"
          label={t('zcode.model.displayName', { defaultValue: '显示名称' })}
        >
          <Input placeholder={t('zcode.model.displayNamePlaceholder', { defaultValue: '留空则使用模型 ID' })} />
        </Form.Item>

        <Form.Item
          name="ruleKind"
          label={t('zcode.model.ruleKind', { defaultValue: '配置方式' })}
          extra={
            <Text type="secondary" style={{ fontSize: 12 }}>
              {t('zcode.model.ruleKindHint', {
                defaultValue:
                  '智能配置只需填写要覆盖的字段，其余沿用 ZCode 内置规则；手动配置要求完整填写，且只支持 ZCode 允许的字段，适用于内置规则无法覆盖的模型。',
              })}
            </Text>
          }
        >
          <Radio.Group>
            <Radio.Button value="smart">
              {t('zcode.model.ruleKindSmart', { defaultValue: '智能配置' })}
            </Radio.Button>
            <Radio.Button value="manual">
              {t('zcode.model.ruleKindManual', { defaultValue: '手动配置' })}
            </Radio.Button>
          </Radio.Group>
        </Form.Item>

        <Form.Item name="enabled" label={t('zcode.model.enabledLabel', { defaultValue: '启用' })}>
          <Select options={triStateOptions(t)} style={{ width: 200 }} />
        </Form.Item>

        <Form.Item
          name="contextWindow"
          label={t('zcode.model.contextWindow', { defaultValue: '上下文窗口' })}
          rules={manualRequiredRule}
        >
          <InputNumber
            min={1}
            style={{ width: '100%' }}
            placeholder={t('zcode.model.contextWindowPlaceholder', { defaultValue: '例如 128000' })}
          />
        </Form.Item>

        <Form.Item
          name="maxOutputTokensMax"
          label={t('zcode.model.maxOutputTokens', { defaultValue: '最大输出 Token' })}
          rules={manualRequiredRule}
        >
          <InputNumber
            min={1}
            style={{ width: '100%' }}
            placeholder={t('zcode.model.maxOutputTokensPlaceholder', { defaultValue: '例如 8192' })}
          />
        </Form.Item>

        <Form.Item
          label={t('zcode.model.inputModalities')}
          extra={
            <Text type="secondary" style={{ fontSize: 12 }}>
              {t('zcode.model.inputModalitiesHint')}
            </Text>
          }
        >
          {/* Each capability is its own compact column: the sub-label sits above
              its select, and the item still validates so a manual row cannot be
              saved with a field missing. */}
          <Space wrap size="small">
            {modalityFields.map((field) => (
              <div key={field.name} style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  {t(field.labelKey, { defaultValue: field.fallback })}
                </Text>
                <Form.Item
                  name={field.name}
                  rules={manualRequiredRule}
                  labelCol={{ span: 0 }}
                  wrapperCol={{ span: 24 }}
                  style={{ marginBottom: 0 }}
                >
                  <Select options={triStateOptions(t)} style={{ width: 116 }} />
                </Form.Item>
              </div>
            ))}
          </Space>
        </Form.Item>

        <Form.Item
          label={t('zcode.model.capabilities', { defaultValue: '模型能力' })}
          extra={
            <Text type="secondary" style={{ fontSize: 12 }}>
              {t('zcode.model.capabilitiesHint', {
                defaultValue: '「禁用」会显式写入 false，与「继承默认」含义不同。',
              })}
            </Text>
          }
        >
          <Space wrap size="small">
            {capabilityFields.map((field) => (
              <div key={field.name} style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  {t(field.labelKey, { defaultValue: field.fallback })}
                </Text>
                <Form.Item
                  name={field.name}
                  rules={manualRequiredRule}
                  labelCol={{ span: 0 }}
                  wrapperCol={{ span: 24 }}
                  style={{ marginBottom: 0 }}
                >
                  <Select options={triStateOptions(t)} style={{ width: 116 }} />
                </Form.Item>
              </div>
            ))}
          </Space>
        </Form.Item>

        <Form.Item
          name="reasoningLevels"
          label={t('zcode.model.reasoningLevels', { defaultValue: '推理等级（从低到高）' })}
          rules={manualRequiredRule}
          extra={
            <Text type="secondary" style={{ fontSize: 12 }}>
              {t('zcode.model.reasoningLevelsHint', {
                defaultValue: '可多选或直接输入自定义等级，顺序即为从低到高。',
              })}
            </Text>
          }
        >
          <Select
            mode="tags"
            allowClear
            placeholder={t('zcode.model.reasoningLevelsPlaceholder', { defaultValue: '选择或输入等级' })}
            options={ZCODE_REASONING_LEVEL_PRESETS.map((level) => ({ value: level, label: level }))}
          />
        </Form.Item>

        <Form.Item
          name="reasoningLevelMap"
          label={t('zcode.model.reasoningLevelMapping', { defaultValue: '推理参数映射' })}
          rules={manualRequiredRule}
          extra={
            <Text type="secondary" style={{ fontSize: 12 }}>
              {t('zcode.model.mapHint', {
                defaultValue:
                  '请求参数名到该字段的映射表达式，例如 {"reasoning_effort": reasoningLevel}；原样写入，不做解析。',
              })}
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
            label={t('zcode.model.maxOutputTokensMapping', { defaultValue: '最大输出参数映射' })}
            extra={
              <Text type="secondary" style={{ fontSize: 12 }}>
                {t('zcode.model.mapHint', {
                  defaultValue:
                    '请求参数名到该字段的映射表达式，例如 {"max_tokens": maxOutputTokens}；原样写入，不做解析。',
                })}
              </Text>
            }
          >
            <Input placeholder={'{"max_tokens": maxOutputTokens}'} />
          </Form.Item>
        )}

        {/* Smart-only: same reason as the modality and capability fields above. */}
        {!isManual && (
          <Form.Item
            name="requiresMfjsToolSchema"
            label={t('zcode.model.requiresMfjsToolSchema', { defaultValue: '需要 MFJS 工具 Schema' })}
          >
            <Select options={triStateOptions(t)} style={{ width: 200 }} />
          </Form.Item>
        )}
      </Form>
    </Modal>
  );
};

export default ZcodeModelFormModal;
