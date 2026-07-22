import React from 'react';
import {
  Modal,
  Form,
  Input,
  Select,
  InputNumber,
  Checkbox,
  Typography,
  Tag,
  Divider,
} from 'antd';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '@/stores';
import {
  PRESET_MODELS,
  getPresetModelsVersion,
  subscribePresetModels,
  type PresetModel,
} from '@/constants/presetModels';
import {
  extractReasoningEffortsFromPresetVariants,
  GROK_MODEL_REASONING_EFFORT_OPTIONS,
  pickDefaultReasoningEffort,
  type GrokModelFormValues,
} from '../utils/grokProviderModels';

const { Text } = Typography;

const XAI_NPM = '@ai-sdk/xai';

interface GrokModelFormModalProps {
  open: boolean;
  isEdit: boolean;
  initialValues?: Partial<GrokModelFormValues>;
  existingKeys?: string[];
  onCancel: () => void;
  onSubmit: (values: GrokModelFormValues) => void | Promise<void>;
}

/**
 * Grok model editor: reuses the OpenCode modal shell (preset strip + form layout)
 * but fields map to Grok Build catalog / config.toml semantics.
 *
 * Presets: show the FULL preset catalog (all SDKs), not only @ai-sdk/xai.
 * Selecting a preset parses its variants into `reasoningEfforts` (all selected).
 */
const GrokModelFormModal: React.FC<GrokModelFormModalProps> = ({
  open,
  isEdit,
  initialValues,
  existingKeys = [],
  onCancel,
  onSubmit,
}) => {
  const { t } = useTranslation();
  const language = useAppStore((state) => state.language);
  const [form] = Form.useForm<GrokModelFormValues>();
  const [submitting, setSubmitting] = React.useState(false);
  const [presetsExpanded, setPresetsExpanded] = React.useState(false);
  const reasoningEfforts = Form.useWatch('reasoningEfforts', form) as string[] | undefined;
  const presetModelsVersion = React.useSyncExternalStore(
    subscribePresetModels,
    getPresetModelsVersion,
    getPresetModelsVersion,
  );

  // Full preset catalog: primary strip prefers xAI models, then every other SDK
  // model with no truncation (user requirement: show all presets).
  const { primaryPresets, otherPresets, allPresets } = React.useMemo(() => {
    const primary = PRESET_MODELS[XAI_NPM] || [];
    const other = Object.entries(PRESET_MODELS)
      .filter(([npm]) => npm !== XAI_NPM)
      .flatMap(([, models]) => models);
    // Dedupe by id while keeping first occurrence (xAI first).
    const seen = new Set<string>();
    const all: PresetModel[] = [];
    [...primary, ...other].forEach((preset) => {
      const id = preset.id?.trim();
      if (!id || seen.has(id)) {
        return;
      }
      seen.add(id);
      all.push(preset);
    });
    return {
      primaryPresets: primary,
      otherPresets: other,
      allPresets: all,
    };
  }, [presetModelsVersion]);

  React.useEffect(() => {
    if (!open) {
      return;
    }
    form.setFieldsValue({
      key: '',
      model: '',
      displayName: '',
      contextWindow: undefined,
      reasoningEfforts: [],
      reasoningEffort: undefined,
      supportsBackendSearch: false,
      ...initialValues,
    });
    setPresetsExpanded(false);
  }, [form, initialValues, open]);

  React.useEffect(() => {
    if (!open) {
      return;
    }
    const current = form.getFieldValue('reasoningEffort') as string | undefined;
    if (current && reasoningEfforts && reasoningEfforts.length > 0 && !reasoningEfforts.includes(current)) {
      form.setFieldValue('reasoningEffort', pickDefaultReasoningEffort(reasoningEfforts));
    }
    if ((!reasoningEfforts || reasoningEfforts.length === 0) && current) {
      form.setFieldValue('reasoningEffort', undefined);
    }
  }, [form, open, reasoningEfforts]);

  const applyPreset = (preset: PresetModel) => {
    // Parse every effort-like token from variants and select ALL of them.
    // Local key and upstream model id always stay the same value.
    const mergedEfforts = extractReasoningEffortsFromPresetVariants(preset.variants);
    const modelId = preset.id;
    const nextKey = isEdit
      ? (form.getFieldValue('model') as string) || modelId
      : modelId;

    form.setFieldsValue({
      key: nextKey,
      model: nextKey,
      displayName: preset.name || nextKey,
      contextWindow: preset.contextLimit,
      // Default: full multi-select of every parsed effort level.
      reasoningEfforts: mergedEfforts,
      reasoningEffort: pickDefaultReasoningEffort(mergedEfforts),
    });
    setPresetsExpanded(false);
  };

  const handleOk = async () => {
    try {
      const values = await form.validateFields();
      setSubmitting(true);
      const efforts = (values.reasoningEfforts || [])
        .map((effort) => effort.trim())
        .filter(Boolean);
      const effort = values.reasoningEffort?.trim();
      // Keep local catalog key identical to the upstream model id.
      const modelId = values.model.trim();
      await onSubmit({
        key: modelId,
        model: modelId,
        displayName: values.displayName?.trim() || modelId,
        contextWindow: values.contextWindow,
        reasoningEfforts: efforts,
        reasoningEffort: effort && efforts.includes(effort) ? effort : efforts[0],
        supportsBackendSearch: values.supportsBackendSearch === true,
      });
    } finally {
      setSubmitting(false);
    }
  };

  const previousModelId = (initialValues?.model || initialValues?.key || '').trim();
  const labelCol = { span: language === 'zh-CN' ? 5 : 7 };
  const wrapperCol = { span: 19 };

  return (
    <Modal
      open={open}
      title={isEdit ? t('grok.model.editTitle') : t('grok.model.addTitle')}
      onCancel={onCancel}
      onOk={() => void handleOk()}
      confirmLoading={submitting}
      destroyOnHidden
      width={640}
    >
      <Form
        form={form}
        layout="horizontal"
        labelCol={labelCol}
        wrapperCol={wrapperCol}
        style={{ marginTop: 24 }}
      >
        <Form.Item label={t('grok.model.upstreamId')} required>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            <Form.Item
              name="model"
              noStyle
              rules={[
                { required: true, message: t('common.error') },
                {
                  validator: async (_, value: string) => {
                    const modelId = value?.trim();
                    if (!modelId) {
                      return;
                    }
                    if (modelId.includes('[') || modelId.includes(']')) {
                      throw new Error(t('grok.model.invalidKey'));
                    }
                    // New model, or rename: reject colliding ids in the channel catalog.
                    if (existingKeys.includes(modelId) && modelId !== previousModelId) {
                      throw new Error(t('grok.model.duplicateKey'));
                    }
                  },
                },
              ]}
            >
              <Input
                placeholder="grok-4.5"
                style={{ flex: 1 }}
                onChange={(event) => {
                  // Keep hidden key field in sync so legacy readers still see key===model.
                  const modelId = event.target.value.trim();
                  form.setFieldValue('key', modelId);
                }}
                onBlur={(event) => {
                  const modelId = event.target.value.trim();
                  if (!form.getFieldValue('displayName')) {
                    form.setFieldValue('displayName', modelId);
                  }
                }}
              />
            </Form.Item>
            {/* Hidden key always mirrors model id (Grok local key === upstream id). */}
            <Form.Item name="key" hidden>
              <Input />
            </Form.Item>
            {allPresets.length > 0 && (
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
                onClick={() => setPresetsExpanded((prev) => !prev)}
              >
                {t('grok.model.selectPreset')}
                {presetsExpanded ? ' ▴' : ' ▾'}
              </a>
            )}
          </div>
          <Text type="secondary" style={{ fontSize: 12 }}>
            {t('grok.model.upstreamIdSameAsKeyHint')}
          </Text>
        </Form.Item>

        {presetsExpanded && allPresets.length > 0 && (
          <Form.Item wrapperCol={{ offset: language === 'zh-CN' ? 5 : 7, span: 19 }} style={{ marginTop: -8 }}>
            {/* Full catalog: xAI first (if any), then every other SDK model — no slice cap. */}
            {primaryPresets.length > 0 && (
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, marginBottom: otherPresets.length > 0 ? 0 : 0 }}>
                {primaryPresets.map((preset) => (
                  <Tag
                    key={`primary-${preset.id}`}
                    style={{ cursor: 'pointer' }}
                    onClick={() => applyPreset(preset)}
                  >
                    {preset.name}
                  </Tag>
                ))}
              </div>
            )}
            {otherPresets.length > 0 && (
              <>
                <Divider style={{ margin: '12px 0', fontSize: 12 }}>
                  {t('grok.model.otherPresets')}
                </Divider>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8, maxHeight: 220, overflowY: 'auto' }}>
                  {otherPresets.map((preset) => (
                    <Tag
                      key={`other-${preset.id}-${preset.name}`}
                      style={{ cursor: 'pointer' }}
                      onClick={() => applyPreset(preset)}
                    >
                      {preset.name}
                    </Tag>
                  ))}
                </div>
              </>
            )}
          </Form.Item>
        )}

        <Form.Item name="displayName" label={t('grok.model.displayName')}>
          <Input placeholder={t('grok.model.displayNamePlaceholder')} />
        </Form.Item>

        <Form.Item name="contextWindow" label={t('grok.model.contextWindow')}>
          <InputNumber min={1} style={{ width: '100%' }} placeholder="256000" />
        </Form.Item>

        <Form.Item
          name="reasoningEfforts"
          label={t('grok.model.reasoningEfforts')}
          extra={<Text type="secondary" style={{ fontSize: 12 }}>{t('grok.model.reasoningEffortsHint')}</Text>}
        >
          <Select
            mode="multiple"
            allowClear
            placeholder={t('grok.model.reasoningEffortsPlaceholder')}
            options={GROK_MODEL_REASONING_EFFORT_OPTIONS.map((value) => ({
              value,
              label: value,
            }))}
          />
        </Form.Item>

        <Form.Item
          name="reasoningEffort"
          label={t('grok.model.reasoningEffortDefault')}
          extra={<Text type="secondary" style={{ fontSize: 12 }}>{t('grok.model.reasoningEffortDefaultHint')}</Text>}
        >
          <Select
            allowClear
            placeholder={t('grok.model.reasoningEffortDefaultPlaceholder')}
            options={(reasoningEfforts || []).map((value) => ({
              value,
              label: value,
            }))}
            disabled={!reasoningEfforts || reasoningEfforts.length === 0}
          />
        </Form.Item>

        <Form.Item
          name="supportsBackendSearch"
          valuePropName="checked"
          wrapperCol={{ offset: language === 'zh-CN' ? 5 : 7, span: 19 }}
        >
          <Checkbox>{t('grok.model.supportsBackendSearch')}</Checkbox>
        </Form.Item>
      </Form>
    </Modal>
  );
};

export default GrokModelFormModal;
