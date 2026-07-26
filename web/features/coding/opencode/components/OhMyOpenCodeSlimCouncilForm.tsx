import React from 'react';
import { Alert, Button, Collapse, Divider, Form, Input, InputNumber, Select, Switch, message } from 'antd';
import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import JsonEditor from '@/components/common/JsonEditor';
import type { OhMyOpenCodeSlimCouncilExecutionMode } from '@/types/ohMyOpenCodeSlim';
import {
  buildSlimCouncilConfig,
  mergeCouncilAgentIntoAgents,
  parseSlimCouncilFormValues,
  type CouncilAgentFormValue,
  type ParseSlimCouncilFormValuesInput,
} from './ohMyOpenCodeSlimCouncilUtils';
import styles from './OhMyOpenCodeSlimCouncilForm.module.less';

export type {
  CouncilAgentFormValue,
  ParseSlimCouncilFormValuesInput,
};
export {
  buildSlimCouncilConfig,
  mergeCouncilAgentIntoAgents,
  parseSlimCouncilFormValues,
};

const { TextArea } = Input;

export type SlimCouncilModelOption =
  | { label: string; value: string; disabled?: boolean }
  | { label: string; options: { label: string; value: string; disabled?: boolean }[] };

type FieldPath = Array<string | number>;

interface CouncilPresetFormValue {
  name?: string;
  councillors?: Array<{
    name?: string;
    model?: string;
    variant?: string;
    prompt?: string;
  }>;
}

const EXECUTION_MODE_OPTIONS: Array<{ label: string; value: OhMyOpenCodeSlimCouncilExecutionMode }> = [
  { label: 'parallel', value: 'parallel' },
  { label: 'serial', value: 'serial' },
];

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

const ModelVariantField: React.FC<{
  form: ReturnType<typeof Form.useForm>[0];
  modelName: FieldPath;
  variantName: FieldPath;
  modelValuePath?: FieldPath;
  variantValuePath?: FieldPath;
  modelOptions: SlimCouncilModelOption[];
  modelVariantsMap: Record<string, string[]>;
  modelPlaceholder: string;
  variantPlaceholder: string;
}> = ({
  form,
  modelName,
  variantName,
  modelValuePath,
  variantValuePath,
  modelOptions,
  modelVariantsMap,
  modelPlaceholder,
  variantPlaceholder,
}) => {
  const effectiveModelValuePath = modelValuePath ?? modelName;
  const effectiveVariantValuePath = variantValuePath ?? variantName;

  return (
    <Form.Item
      noStyle
      shouldUpdate={(previousValues, currentValues) => {
        const previousModel = getPathValue(previousValues, effectiveModelValuePath);
        const currentModel = getPathValue(currentValues, effectiveModelValuePath);
        const previousVariant = getPathValue(previousValues, effectiveVariantValuePath);
        const currentVariant = getPathValue(currentValues, effectiveVariantValuePath);
        return previousModel !== currentModel || previousVariant !== currentVariant;
      }}
    >
      {() => {
        const selectedModel = form.getFieldValue(effectiveModelValuePath);
        const currentVariant = form.getFieldValue(effectiveVariantValuePath);
        const mappedVariants = typeof selectedModel === 'string' ? modelVariantsMap[selectedModel] ?? [] : [];
        const variantOptions = [...mappedVariants];

        if (typeof currentVariant === 'string' && currentVariant && !variantOptions.includes(currentVariant)) {
          variantOptions.unshift(currentVariant);
        }

        const showVariantSelect = variantOptions.length > 0 || (typeof currentVariant === 'string' && currentVariant !== '');

        return (
          <div className={styles.compactFieldRow}>
            <Form.Item name={modelName} noStyle>
              <Select
                options={modelOptions}
                allowClear
                showSearch
                optionFilterProp="label"
                placeholder={modelPlaceholder}
                className={styles.compactModelSelect}
                onChange={(nextModel) => {
                  const nextVariants = typeof nextModel === 'string' ? modelVariantsMap[nextModel] ?? [] : [];
                  const existingVariant = form.getFieldValue(effectiveVariantValuePath);
                  if (nextVariants.length === 0 || (existingVariant && !nextVariants.includes(existingVariant))) {
                    form.setFieldValue(effectiveVariantValuePath, undefined);
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
                  className={styles.variantSelect}
                />
              </Form.Item>
            )}
          </div>
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

  const sectionLabel = (
    <div className={styles.sectionLabel}>
      <div className={styles.sectionLabelMain}>
        <span className={styles.sectionTitle}>{t('opencode.ohMyOpenCodeSlim.councilSettings')}</span>
      </div>
      <span className={styles.sectionHint}>{t('opencode.ohMyOpenCodeSlim.councilHint')}</span>
    </div>
  );

  const renderEnabledContent = () => (
    <div className={styles.sectionBody}>
      <div className={styles.mainCard}>
        <div className={styles.cardHeader}>
          <div className={styles.cardHeaderMeta}>
            <span className={styles.cardTitle}>{t('opencode.ohMyOpenCodeSlim.councilAgent')}</span>
            <span className={styles.cardHint}>{t('opencode.ohMyOpenCodeSlim.councilAgentHint')}</span>
          </div>
        </div>

        <div className={styles.settingsGrid}>
          <Form.Item className={styles.fullWidthItem} label={t('opencode.ohMyOpenCodeSlim.councilAgentModel')} required>
            <ModelVariantField
              form={form}
              modelName={['councilAgent', 'model']}
              variantName={['councilAgent', 'variant']}
              modelOptions={modelOptions}
              modelVariantsMap={modelVariantsMap}
              modelPlaceholder={t('opencode.ohMyOpenCode.selectModel')}
              variantPlaceholder={t('opencode.ohMyOpenCodeSlim.councilVariantPlaceholder')}
            />
          </Form.Item>

          <Form.Item
            className={styles.fullWidthItem}
            label={t('opencode.ohMyOpenCodeSlim.councilAgentPrompt')}
            name={['councilAgent', 'prompt']}
          >
            <TextArea rows={4} placeholder={t('opencode.ohMyOpenCodeSlim.councilPromptPlaceholder')} />
          </Form.Item>
        </div>
      </div>

      <div className={styles.mainCard}>
        <div className={styles.cardHeader}>
          <div className={styles.cardHeaderMeta}>
            <span className={styles.cardTitle}>{t('opencode.ohMyOpenCodeSlim.councilPresets')}</span>
            <span className={styles.cardHint}>{t('opencode.ohMyOpenCodeSlim.councilPresetSelectionHint')}</span>
          </div>
        </div>

        <div className={styles.settingsGrid}>
          <Form.Item label={t('opencode.ohMyOpenCodeSlim.councilDefaultPreset')} name="councilDefaultPreset">
            <Select
              allowClear
              showSearch
              optionFilterProp="label"
              options={presetOptions}
              placeholder={t('opencode.ohMyOpenCodeSlim.councilDefaultPresetPlaceholder')}
            />
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

          <Form.Item label={t('opencode.ohMyOpenCodeSlim.councilCouncillorsTimeout')} name="councilCouncillorsTimeout">
            <InputNumber min={0} addonAfter="ms" style={{ width: '100%' }} />
          </Form.Item>

          <Form.Item label={t('opencode.ohMyOpenCodeSlim.councilRetries')} name="councilRetries">
            <InputNumber min={0} max={5} style={{ width: '100%' }} />
          </Form.Item>
        </div>
      </div>

      <div className={styles.mainCard}>
        <Divider className={styles.divider}>{t('opencode.ohMyOpenCodeSlim.councilPresetList')}</Divider>

        <Form.List name="councilPresets">
          {(presetFields, { add: addPreset, remove: removePreset }) => (
            <>
              <div className={styles.listActions}>
                <Button type="dashed" icon={<PlusOutlined />} onClick={() => addPreset({ councillors: [{}] })}>
                  {t('opencode.ohMyOpenCodeSlim.councilAddPreset')}
                </Button>
              </div>

              <div className={styles.presetList}>
                {presetFields.map((presetField, presetIndex) => (
                  <div key={presetField.key} className={styles.presetCard}>
                    <div className={styles.cardHeader}>
                      <div className={styles.cardHeaderMeta}>
                        <span className={styles.cardTitle}>{t('opencode.ohMyOpenCodeSlim.councilPresetTitle', { index: presetIndex + 1 })}</span>
                        <span className={styles.cardHint}>{t('opencode.ohMyOpenCodeSlim.councilPresetHint')}</span>
                      </div>
                      <Button
                        danger
                        type="text"
                        icon={<DeleteOutlined />}
                        onClick={() => removePreset(presetField.name)}
                        className={styles.iconButton}
                      />
                    </div>

                    <div className={styles.settingsGrid}>
                      <Form.Item label={t('opencode.ohMyOpenCodeSlim.councilPresetName')} name={[presetField.name, 'name']}>
                        <Input placeholder={t('opencode.ohMyOpenCodeSlim.councilPresetNamePlaceholder')} />
                      </Form.Item>
                    </div>

                    <Divider plain className={styles.divider}>{t('opencode.ohMyOpenCodeSlim.councilCouncillors')}</Divider>

                    <Form.List name={[presetField.name, 'councillors']}>
                      {(councillorFields, { add: addCouncillor, remove: removeCouncillor }) => (
                        <>
                          <div className={styles.listActions}>
                            <Button type="dashed" icon={<PlusOutlined />} onClick={() => addCouncillor({})}>
                              {t('opencode.ohMyOpenCodeSlim.councilAddCouncillor')}
                            </Button>
                          </div>

                          <div className={styles.councillorList}>
                            {councillorFields.map((councillorField, councillorIndex) => (
                              <div key={councillorField.key} className={styles.subCard}>
                                <div className={styles.cardHeader}>
                                  <div className={styles.cardHeaderMeta}>
                                    <span className={styles.cardTitle}>{t('opencode.ohMyOpenCodeSlim.councilCouncillorTitle', { index: councillorIndex + 1 })}</span>
                                  </div>
                                  <Button
                                    danger
                                    type="text"
                                    icon={<DeleteOutlined />}
                                    onClick={() => {
                                      if (councillorFields.length <= 1) {
                                        message.warning(t('opencode.ohMyOpenCodeSlim.councilCouncillorDeleteBlocked'));
                                        return;
                                      }
                                      removeCouncillor(councillorField.name);
                                    }}
                                    className={styles.iconButton}
                                  />
                                </div>

                                <div className={styles.settingsGrid}>
                                  <Form.Item
                                    label={t('opencode.ohMyOpenCodeSlim.councilCouncillorName')}
                                    name={[councillorField.name, 'name']}
                                  >
                                    <Input placeholder={t('opencode.ohMyOpenCodeSlim.councilCouncillorNamePlaceholder')} />
                                  </Form.Item>

                                  <Form.Item className={styles.fullWidthItem} label={t('opencode.ohMyOpenCodeSlim.councilCouncillorModel')}>
                                    <ModelVariantField
                                      form={form}
                                      modelName={[councillorField.name, 'model']}
                                      variantName={[councillorField.name, 'variant']}
                                      modelValuePath={['councilPresets', presetField.name, 'councillors', councillorField.name, 'model']}
                                      variantValuePath={['councilPresets', presetField.name, 'councillors', councillorField.name, 'variant']}
                                      modelOptions={modelOptions}
                                      modelVariantsMap={modelVariantsMap}
                                      modelPlaceholder={t('opencode.ohMyOpenCode.selectModel')}
                                      variantPlaceholder={t('opencode.ohMyOpenCodeSlim.councilVariantPlaceholder')}
                                    />
                                  </Form.Item>

                                  <Form.Item
                                    className={styles.fullWidthItem}
                                    label={t('opencode.ohMyOpenCodeSlim.councilCouncillorPrompt')}
                                    name={[councillorField.name, 'prompt']}
                                  >
                                    <TextArea rows={3} placeholder={t('opencode.ohMyOpenCodeSlim.councilPromptPlaceholder')} />
                                  </Form.Item>
                                </div>
                              </div>
                            ))}
                          </div>
                        </>
                      )}
                    </Form.List>
                  </div>
                ))}
              </div>
            </>
          )}
        </Form.List>
      </div>

      <div className={styles.mainCard}>
        <div className={styles.cardHeader}>
          <div className={styles.cardHeaderMeta}>
            <span className={styles.cardTitle}>{t('opencode.ohMyOpenCodeSlim.otherFields')}</span>
            <span className={styles.cardHint}>{t('opencode.ohMyOpenCodeSlim.councilOtherFieldsHint')}</span>
          </div>
        </div>

        <Form.Item
          className={styles.editorItem}
          name="councilOtherFields"
          labelCol={{ span: 24 }}
          wrapperCol={{ span: 24 }}
        >
          <JsonEditor
            value={emptyToUndefined(form.getFieldValue('councilOtherFields'))}
            onChange={(value, isValid) => {
              councilOtherFieldsValidRef.current = isValid;
              if (value === null || value === undefined) {
                form.setFieldValue('councilOtherFields', undefined);
                return;
              }
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
      </div>
    </div>
  );

  return (
    <Collapse
      className={styles.sectionCollapse}
      defaultActiveKey={[]}
      ghost
      items={[
        {
          key: 'council',
          label: sectionLabel,
          children: (
            <div className={styles.sectionBody}>
              <div className={styles.switchRow}>
                <div className={styles.switchContent}>
                  <span className={styles.switchTitle}>{t('opencode.ohMyOpenCodeSlim.councilEnabled')}</span>
                </div>
                <Form.Item name="councilEnabled" valuePropName="checked" noStyle>
                  <Switch />
                </Form.Item>
              </div>

              {councilEnabled ? renderEnabledContent() : (
                <Alert
                  className={styles.disabledState}
                  type="info"
                  showIcon
                  message={t('opencode.ohMyOpenCodeSlim.councilDisabledHint')}
                />
              )}
            </div>
          ),
        },
      ]}
    />
  );
};

export default OhMyOpenCodeSlimCouncilForm;
