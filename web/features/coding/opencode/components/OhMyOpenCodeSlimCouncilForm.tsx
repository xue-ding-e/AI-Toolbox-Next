import React from 'react';
import { Alert, Button, Collapse, Divider, Form, Input, InputNumber, Select, Space, Switch, Typography } from 'antd';
import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import JsonEditor from '@/components/common/JsonEditor';
import type { OhMyOpenCodeSlimCouncilExecutionMode } from '@/types/ohMyOpenCodeSlim';

const { Text } = Typography;
const { TextArea } = Input;

export type SlimCouncilModelOption =
  | { label: string; value: string; disabled?: boolean }
  | { label: string; options: { label: string; value: string; disabled?: boolean }[] };

type FieldPath = Array<string | number>;

interface CouncilMasterFormValue {
  model?: string;
  variant?: string;
  prompt?: string;
}

interface CouncilCouncillorFormValue {
  name?: string;
  model?: string;
  variant?: string;
  prompt?: string;
}

interface CouncilPresetFormValue {
  name?: string;
  master?: CouncilMasterFormValue;
  councillors?: CouncilCouncillorFormValue[];
}

const EMPTY_OBJECT: Record<string, unknown> = {};
const EXECUTION_MODE_OPTIONS: Array<{ label: string; value: OhMyOpenCodeSlimCouncilExecutionMode }> = [
  { label: 'parallel', value: 'parallel' },
  { label: 'serial', value: 'serial' },
];

const cleanObject = (obj: Record<string, unknown>): Record<string, unknown> => {
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(obj)) {
    if (value === null || value === undefined) continue;
    if (Array.isArray(value) && value.length === 0) continue;
    if (typeof value === 'object' && value !== null && !Array.isArray(value) && Object.keys(value).length === 0) continue;
    if (typeof value === 'string' && value.trim() === '') continue;
    result[key] = value;
  }
  return result;
};

const asObject = (value: unknown): Record<string, unknown> | undefined => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return undefined;
  }
  return value as Record<string, unknown>;
};

const asStringArray = (value: unknown): string[] | undefined => {
  if (!Array.isArray(value)) {
    return undefined;
  }
  const items = value.filter((item): item is string => typeof item === 'string' && item.trim() !== '');
  return items.length > 0 ? items : undefined;
};

const asNumber = (value: unknown): number | undefined => {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
};

const emptyToUndefined = (value: unknown): unknown => {
  if (value === null || value === undefined) {
    return undefined;
  }

  if (typeof value === 'object' && !Array.isArray(value) && Object.keys(value as Record<string, unknown>).length === 0) {
    return undefined;
  }

  return value;
};

const getPathValue = (values: Record<string, unknown>, path: FieldPath): unknown => {
  let current: unknown = values;
  for (const key of path) {
    if (!current || typeof current !== 'object') {
      return undefined;
    }
    current = (current as Record<string, unknown>)[String(key)];
  }
  return current;
};

const serializeMasterConfig = (master: CouncilMasterFormValue | undefined): Record<string, unknown> | undefined => {
  if (!master) {
    return undefined;
  }

  const result = cleanObject({
    model: master.model?.trim(),
    variant: master.variant?.trim(),
    prompt: master.prompt?.trim(),
  });

  return Object.keys(result).length > 0 ? result : undefined;
};

export const parseSlimCouncilFormValues = (rawCouncil: Record<string, unknown> | null | undefined) => {
  const council = asObject(rawCouncil);
  if (!council) {
    return {
      councilEnabled: false,
      councilMaster: undefined,
      councilDefaultPreset: undefined,
      councilMasterTimeout: 300000,
      councilCouncillorsTimeout: 180000,
      councilMasterFallback: [],
      councilExecutionMode: 'parallel' as OhMyOpenCodeSlimCouncilExecutionMode,
      councilRetries: 3,
      councilPresets: [] as CouncilPresetFormValue[],
      councilOtherFields: undefined,
    };
  }

  const presetsObject = asObject(council.presets) ?? EMPTY_OBJECT;
  const parsedPresets: CouncilPresetFormValue[] = Object.entries(presetsObject).map(([presetName, presetValue]) => {
    const presetObject = asObject(presetValue) ?? EMPTY_OBJECT;
    const presetMaster = asObject(presetObject.master);
    const councillors: CouncilCouncillorFormValue[] = Object.entries(presetObject)
      .filter(([key]) => key !== 'master')
      .map(([councillorName, councillorValue]) => {
        const councillorObject = asObject(councillorValue) ?? EMPTY_OBJECT;
        return {
          name: councillorName,
          model: typeof councillorObject.model === 'string' ? councillorObject.model : undefined,
          variant: typeof councillorObject.variant === 'string' ? councillorObject.variant : undefined,
          prompt: typeof councillorObject.prompt === 'string' ? councillorObject.prompt : undefined,
        };
      });

    return {
      name: presetName,
      master: {
        model: typeof presetMaster?.model === 'string' ? presetMaster.model : undefined,
        variant: typeof presetMaster?.variant === 'string' ? presetMaster.variant : undefined,
        prompt: typeof presetMaster?.prompt === 'string' ? presetMaster.prompt : undefined,
      },
      councillors,
    };
  });

  const councilOtherFields = { ...council };
  delete councilOtherFields.master;
  delete councilOtherFields.presets;
  delete councilOtherFields.default_preset;
  delete councilOtherFields.master_timeout;
  delete councilOtherFields.councillors_timeout;
  delete councilOtherFields.master_fallback;
  delete councilOtherFields.councillor_execution_mode;
  delete councilOtherFields.councillor_retries;

  return {
    councilEnabled: true,
    councilMaster: {
      model: typeof asObject(council.master)?.model === 'string' ? String(asObject(council.master)?.model) : undefined,
      variant: typeof asObject(council.master)?.variant === 'string' ? String(asObject(council.master)?.variant) : undefined,
      prompt: typeof asObject(council.master)?.prompt === 'string' ? String(asObject(council.master)?.prompt) : undefined,
    },
    councilDefaultPreset: typeof council.default_preset === 'string' ? council.default_preset : undefined,
    councilMasterTimeout: asNumber(council.master_timeout) ?? 300000,
    councilCouncillorsTimeout: asNumber(council.councillors_timeout) ?? 180000,
    councilMasterFallback: asStringArray(council.master_fallback) ?? [],
    councilExecutionMode: council.councillor_execution_mode === 'serial' ? 'serial' as const : 'parallel' as const,
    councilRetries: asNumber(council.councillor_retries) ?? 3,
    councilPresets: parsedPresets,
    councilOtherFields: Object.keys(councilOtherFields).length > 0 ? councilOtherFields : undefined,
  };
};

export const buildSlimCouncilConfig = (
  formValues: Record<string, unknown>,
  t: (key: string, options?: Record<string, unknown>) => string,
): { council: Record<string, unknown> | null; errorMessage?: string } => {
  if (!formValues.councilEnabled) {
    return { council: null };
  }

  const councilMaster = asObject(cleanObject(serializeMasterConfig(formValues.councilMaster as CouncilMasterFormValue | undefined) ?? EMPTY_OBJECT));
  if (!councilMaster?.model || typeof councilMaster.model !== 'string') {
    return { council: null, errorMessage: t('opencode.ohMyOpenCodeSlim.councilMasterModelRequired') };
  }

  const presets = Array.isArray(formValues.councilPresets)
    ? (formValues.councilPresets as CouncilPresetFormValue[])
    : [];

  if (presets.length === 0) {
    return { council: null, errorMessage: t('opencode.ohMyOpenCodeSlim.councilPresetRequired') };
  }

  const serializedPresets: Record<string, Record<string, unknown>> = {};
  const seenPresetNames = new Set<string>();

  for (const preset of presets) {
    const presetName = preset?.name?.trim();
    if (!presetName) {
      return { council: null, errorMessage: t('opencode.ohMyOpenCodeSlim.councilPresetNameRequired') };
    }

    if (seenPresetNames.has(presetName)) {
      return {
        council: null,
        errorMessage: t('opencode.ohMyOpenCodeSlim.councilPresetNameDuplicate', { name: presetName }),
      };
    }
    seenPresetNames.add(presetName);

    const councillors = Array.isArray(preset.councillors) ? preset.councillors : [];
    if (councillors.length === 0) {
      return {
        council: null,
        errorMessage: t('opencode.ohMyOpenCodeSlim.councilPresetEmpty', { name: presetName }),
      };
    }

    const serializedPreset: Record<string, unknown> = {};
    const presetMaster = serializeMasterConfig(preset.master);
    if (presetMaster) {
      serializedPreset.master = presetMaster;
    }

    const seenCouncillorNames = new Set<string>();
    for (const councillor of councillors) {
      const councillorName = councillor?.name?.trim();
      if (!councillorName) {
        return {
          council: null,
          errorMessage: t('opencode.ohMyOpenCodeSlim.councilCouncillorNameRequired', { preset: presetName }),
        };
      }

      if (councillorName === 'master') {
        return {
          council: null,
          errorMessage: t('opencode.ohMyOpenCodeSlim.councilCouncillorNameReserved', { preset: presetName }),
        };
      }

      if (seenCouncillorNames.has(councillorName)) {
        return {
          council: null,
          errorMessage: t('opencode.ohMyOpenCodeSlim.councilCouncillorNameDuplicate', {
            preset: presetName,
            name: councillorName,
          }),
        };
      }
      seenCouncillorNames.add(councillorName);

      const councillorModel = councillor?.model?.trim();
      if (!councillorModel) {
        return {
          council: null,
          errorMessage: t('opencode.ohMyOpenCodeSlim.councilCouncillorModelRequired', {
            preset: presetName,
            name: councillorName,
          }),
        };
      }

      serializedPreset[councillorName] = cleanObject({
        model: councillorModel,
        variant: councillor.variant?.trim(),
        prompt: councillor.prompt?.trim(),
      });
    }

    serializedPresets[presetName] = serializedPreset;
  }

  const defaultPreset = typeof formValues.councilDefaultPreset === 'string' && formValues.councilDefaultPreset.trim() !== ''
    ? formValues.councilDefaultPreset.trim()
    : Object.keys(serializedPresets)[0];

  if (!serializedPresets[defaultPreset]) {
    return {
      council: null,
      errorMessage: t('opencode.ohMyOpenCodeSlim.councilDefaultPresetMissing', { name: defaultPreset }),
    };
  }

  const councilOtherFields = asObject(formValues.councilOtherFields);
  const councilConfig = cleanObject({
    master: councilMaster,
    default_preset: defaultPreset,
    master_timeout: typeof formValues.councilMasterTimeout === 'number' ? formValues.councilMasterTimeout : undefined,
    councillors_timeout: typeof formValues.councilCouncillorsTimeout === 'number' ? formValues.councilCouncillorsTimeout : undefined,
    master_fallback: asStringArray(formValues.councilMasterFallback),
    councillor_execution_mode: formValues.councilExecutionMode,
    councillor_retries: typeof formValues.councilRetries === 'number' ? formValues.councilRetries : undefined,
    presets: serializedPresets,
    ...(councilOtherFields ?? {}),
  });

  return { council: councilConfig };
};

const ModelVariantField: React.FC<{
  form: ReturnType<typeof Form.useForm>[0];
  modelName: FieldPath;
  variantName: FieldPath;
  modelOptions: SlimCouncilModelOption[];
  modelVariantsMap: Record<string, string[]>;
  modelPlaceholder: string;
  variantPlaceholder: string;
}> = ({
  form,
  modelName,
  variantName,
  modelOptions,
  modelVariantsMap,
  modelPlaceholder,
  variantPlaceholder,
}) => {
  return (
    <Form.Item
      noStyle
      shouldUpdate={(previousValues, currentValues) => {
        const previousModel = getPathValue(previousValues, modelName);
        const currentModel = getPathValue(currentValues, modelName);
        const previousVariant = getPathValue(previousValues, variantName);
        const currentVariant = getPathValue(currentValues, variantName);
        return previousModel !== currentModel || previousVariant !== currentVariant;
      }}
    >
      {() => {
        const selectedModel = form.getFieldValue(modelName);
        const currentVariant = form.getFieldValue(variantName);
        const mappedVariants = typeof selectedModel === 'string' ? modelVariantsMap[selectedModel] ?? [] : [];
        const variantOptions = [...mappedVariants];

        if (typeof currentVariant === 'string' && currentVariant && !variantOptions.includes(currentVariant)) {
          variantOptions.unshift(currentVariant);
        }

        const showVariantSelect = variantOptions.length > 0 || (typeof currentVariant === 'string' && currentVariant !== '');

        return (
          <Space.Compact style={{ width: '100%' }}>
            <Form.Item name={modelName} noStyle>
              <Select
                options={modelOptions}
                allowClear
                showSearch
                optionFilterProp="label"
                placeholder={modelPlaceholder}
                style={{ width: showVariantSelect ? 'calc(100% - 120px)' : '100%' }}
                onChange={(nextModel) => {
                  const nextVariants = typeof nextModel === 'string' ? modelVariantsMap[nextModel] ?? [] : [];
                  const existingVariant = form.getFieldValue(variantName);
                  if (nextVariants.length === 0 || (existingVariant && !nextVariants.includes(existingVariant))) {
                    form.setFieldValue(variantName, undefined);
                  }
                }}
              />
            </Form.Item>
            {showVariantSelect && (
              <Form.Item name={variantName} noStyle>
                <Select
                  allowClear
                  placeholder={variantPlaceholder}
                  options={variantOptions.map((variant) => ({ label: variant, value: variant }))}
                  style={{ width: 120 }}
                />
              </Form.Item>
            )}
          </Space.Compact>
        );
      }}
    </Form.Item>
  );
};

interface SlimCouncilFormSectionProps {
  form: ReturnType<typeof Form.useForm>[0];
  modelOptions: SlimCouncilModelOption[];
  modelVariantsMap: Record<string, string[]>;
  councilOtherFieldsValidRef: React.MutableRefObject<boolean>;
}

const OhMyOpenCodeSlimCouncilForm: React.FC<SlimCouncilFormSectionProps> = ({
  form,
  modelOptions,
  modelVariantsMap,
  councilOtherFieldsValidRef,
}) => {
  const { t } = useTranslation();
  const councilEnabled = Form.useWatch('councilEnabled', form) ?? false;
  const councilPresets = Form.useWatch('councilPresets', form) as CouncilPresetFormValue[] | undefined;

  const presetOptions = React.useMemo(() => {
    if (!Array.isArray(councilPresets)) {
      return [];
    }

    return councilPresets
      .map((preset) => preset?.name?.trim())
      .filter((name): name is string => Boolean(name))
      .map((name) => ({ label: name, value: name }));
  }, [councilPresets]);

  return (
    <Collapse
      defaultActiveKey={[]}
      style={{ marginTop: 8 }}
      ghost
      items={[
        {
          key: 'council',
          label: <Text strong>{t('opencode.ohMyOpenCodeSlim.councilSettings')}</Text>,
          children: (
            <>
              <Form.Item
                label={t('opencode.ohMyOpenCodeSlim.councilEnabled')}
                name="councilEnabled"
                valuePropName="checked"
              >
                <Switch />
              </Form.Item>

              <Text type="secondary" style={{ display: 'block', marginBottom: 16 }}>
                {t('opencode.ohMyOpenCodeSlim.councilHint')}
              </Text>

              {councilEnabled ? (
                <>
                  <div style={{ border: '1px solid var(--color-border)', borderRadius: 8, padding: 16, marginBottom: 16 }}>
                    <Text strong>{t('opencode.ohMyOpenCodeSlim.councilMaster')}</Text>
                    <Text type="secondary" style={{ display: 'block', marginTop: 4, marginBottom: 16 }}>
                      {t('opencode.ohMyOpenCodeSlim.councilMasterHint')}
                    </Text>

                    <Form.Item label={t('opencode.ohMyOpenCodeSlim.councilMasterModel')} required>
                      <ModelVariantField
                        form={form}
                        modelName={['councilMaster', 'model']}
                        variantName={['councilMaster', 'variant']}
                        modelOptions={modelOptions}
                        modelVariantsMap={modelVariantsMap}
                        modelPlaceholder={t('opencode.ohMyOpenCode.selectModel')}
                        variantPlaceholder={t('opencode.ohMyOpenCodeSlim.councilVariantPlaceholder')}
                      />
                    </Form.Item>

                    <Form.Item label={t('opencode.ohMyOpenCodeSlim.councilMasterPrompt')} name={['councilMaster', 'prompt']}>
                      <TextArea rows={4} placeholder={t('opencode.ohMyOpenCodeSlim.councilPromptPlaceholder')} />
                    </Form.Item>
                  </div>

                  <Form.Item label={t('opencode.ohMyOpenCodeSlim.councilDefaultPreset')} name="councilDefaultPreset">
                    <Select
                      allowClear
                      showSearch
                      optionFilterProp="label"
                      options={presetOptions}
                      placeholder={t('opencode.ohMyOpenCodeSlim.councilDefaultPresetPlaceholder')}
                    />
                  </Form.Item>

                  <Form.Item label={t('opencode.ohMyOpenCodeSlim.councilMasterTimeout')} name="councilMasterTimeout">
                    <InputNumber min={0} addonAfter="ms" style={{ width: '100%' }} />
                  </Form.Item>

                  <Form.Item label={t('opencode.ohMyOpenCodeSlim.councilCouncillorsTimeout')} name="councilCouncillorsTimeout">
                    <InputNumber min={0} addonAfter="ms" style={{ width: '100%' }} />
                  </Form.Item>

                  <Form.Item label={t('opencode.ohMyOpenCodeSlim.councilExecutionMode')} name="councilExecutionMode">
                    <Select
                      options={EXECUTION_MODE_OPTIONS.map((option) => ({
                        value: option.value,
                        label: option.value === 'parallel'
                          ? t('opencode.ohMyOpenCodeSlim.councilExecutionModeParallel')
                          : t('opencode.ohMyOpenCodeSlim.councilExecutionModeSerial'),
                      }))}
                    />
                  </Form.Item>

                  <Form.Item label={t('opencode.ohMyOpenCodeSlim.councilRetries')} name="councilRetries">
                    <InputNumber min={0} max={5} style={{ width: '100%' }} />
                  </Form.Item>

                  <Form.Item label={t('opencode.ohMyOpenCodeSlim.councilMasterFallback')} name="councilMasterFallback">
                    <Select
                      mode="tags"
                      allowClear
                      showSearch
                      optionFilterProp="label"
                      options={modelOptions}
                      placeholder={t('opencode.ohMyOpenCodeSlim.councilMasterFallbackPlaceholder')}
                    />
                  </Form.Item>

                  <Divider>{t('opencode.ohMyOpenCodeSlim.councilPresets')}</Divider>

                  <Form.List name="councilPresets">
                    {(presetFields, { add: addPreset, remove: removePreset }) => (
                      <>
                        <Space style={{ marginBottom: 16 }}>
                          <Button type="dashed" icon={<PlusOutlined />} onClick={() => addPreset({ councillors: [{}] })}>
                            {t('opencode.ohMyOpenCodeSlim.councilAddPreset')}
                          </Button>
                        </Space>

                        {presetFields.map((presetField, presetIndex) => (
                          <div
                            key={presetField.key}
                            style={{
                              border: '1px solid var(--color-border)',
                              borderRadius: 8,
                              padding: 16,
                              marginBottom: 16,
                              background: 'var(--color-bg-container)',
                            }}
                          >
                            <Space style={{ width: '100%', justifyContent: 'space-between', marginBottom: 8 }}>
                              <Text strong>{t('opencode.ohMyOpenCodeSlim.councilPresetTitle', { index: presetIndex + 1 })}</Text>
                              <Button danger type="text" icon={<DeleteOutlined />} onClick={() => removePreset(presetField.name)} />
                            </Space>

                            <Form.Item
                              label={t('opencode.ohMyOpenCodeSlim.councilPresetName')}
                              name={[presetField.name, 'name']}
                            >
                              <Input placeholder={t('opencode.ohMyOpenCodeSlim.councilPresetNamePlaceholder')} />
                            </Form.Item>

                            <Divider plain>{t('opencode.ohMyOpenCodeSlim.councilPresetMasterOverride')}</Divider>
                            <Text type="secondary" style={{ display: 'block', marginBottom: 12 }}>
                              {t('opencode.ohMyOpenCodeSlim.councilPresetMasterOverrideHint')}
                            </Text>

                            <Form.Item label={t('opencode.ohMyOpenCodeSlim.councilMasterModel')}>
                              <ModelVariantField
                                form={form}
                                modelName={['councilPresets', presetField.name, 'master', 'model']}
                                variantName={['councilPresets', presetField.name, 'master', 'variant']}
                                modelOptions={modelOptions}
                                modelVariantsMap={modelVariantsMap}
                                modelPlaceholder={t('opencode.ohMyOpenCode.selectModel')}
                                variantPlaceholder={t('opencode.ohMyOpenCodeSlim.councilVariantPlaceholder')}
                              />
                            </Form.Item>

                            <Form.Item
                              label={t('opencode.ohMyOpenCodeSlim.councilMasterPrompt')}
                              name={['councilPresets', presetField.name, 'master', 'prompt']}
                            >
                              <TextArea rows={3} placeholder={t('opencode.ohMyOpenCodeSlim.councilPromptPlaceholder')} />
                            </Form.Item>

                            <Divider plain>{t('opencode.ohMyOpenCodeSlim.councilCouncillors')}</Divider>

                            <Form.List name={[presetField.name, 'councillors']}>
                              {(councillorFields, { add: addCouncillor, remove: removeCouncillor }) => (
                                <>
                                  <Space style={{ marginBottom: 16 }}>
                                    <Button type="dashed" icon={<PlusOutlined />} onClick={() => addCouncillor({})}>
                                      {t('opencode.ohMyOpenCodeSlim.councilAddCouncillor')}
                                    </Button>
                                  </Space>

                                  {councillorFields.map((councillorField, councillorIndex) => (
                                    <div
                                      key={councillorField.key}
                                      style={{
                                        padding: 12,
                                        borderRadius: 8,
                                        border: '1px dashed var(--color-border)',
                                        marginBottom: 12,
                                      }}
                                    >
                                      <Space style={{ width: '100%', justifyContent: 'space-between', marginBottom: 8 }}>
                                        <Text>{t('opencode.ohMyOpenCodeSlim.councilCouncillorTitle', { index: councillorIndex + 1 })}</Text>
                                        <Button
                                          danger
                                          type="text"
                                          icon={<DeleteOutlined />}
                                          onClick={() => removeCouncillor(councillorField.name)}
                                        />
                                      </Space>

                                      <Form.Item
                                        label={t('opencode.ohMyOpenCodeSlim.councilCouncillorName')}
                                        name={[councillorField.name, 'name']}
                                      >
                                        <Input placeholder={t('opencode.ohMyOpenCodeSlim.councilCouncillorNamePlaceholder')} />
                                      </Form.Item>

                                      <Form.Item label={t('opencode.ohMyOpenCodeSlim.councilCouncillorModel')}>
                                        <ModelVariantField
                                          form={form}
                                          modelName={['councilPresets', presetField.name, 'councillors', councillorField.name, 'model']}
                                          variantName={['councilPresets', presetField.name, 'councillors', councillorField.name, 'variant']}
                                          modelOptions={modelOptions}
                                          modelVariantsMap={modelVariantsMap}
                                          modelPlaceholder={t('opencode.ohMyOpenCode.selectModel')}
                                          variantPlaceholder={t('opencode.ohMyOpenCodeSlim.councilVariantPlaceholder')}
                                        />
                                      </Form.Item>

                                      <Form.Item
                                        label={t('opencode.ohMyOpenCodeSlim.councilCouncillorPrompt')}
                                        name={[councillorField.name, 'prompt']}
                                      >
                                        <TextArea rows={3} placeholder={t('opencode.ohMyOpenCodeSlim.councilPromptPlaceholder')} />
                                      </Form.Item>
                                    </div>
                                  ))}
                                </>
                              )}
                            </Form.List>
                          </div>
                        ))}
                      </>
                    )}
                  </Form.List>

                  <Form.Item
                    name="councilOtherFields"
                    labelCol={{ span: 24 }}
                    wrapperCol={{ span: 24 }}
                    help={t('opencode.ohMyOpenCodeSlim.councilOtherFieldsHint')}
                  >
                    <JsonEditor
                      value={emptyToUndefined(form.getFieldValue('councilOtherFields'))}
                      onChange={(value, isValid) => {
                        councilOtherFieldsValidRef.current = isValid;
                        if (isValid && typeof value === 'object' && value !== null && !Array.isArray(value)) {
                          form.setFieldValue('councilOtherFields', value);
                        }
                      }}
                      height={180}
                      minHeight={120}
                      maxHeight={260}
                      resizable
                      mode="text"
                      placeholder={`{
  "custom_flag": true
}`}
                    />
                  </Form.Item>
                </>
              ) : (
                <Alert
                  type="info"
                  showIcon
                  message={t('opencode.ohMyOpenCodeSlim.councilDisabledHint')}
                />
              )}
            </>
          ),
        },
      ]}
    />
  );
};

export default OhMyOpenCodeSlimCouncilForm;
