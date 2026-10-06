import React, { useEffect, useMemo, useState } from 'react';
import { Alert, Form, Modal, Select, Switch, Tabs, message } from 'antd';
import { useTranslation } from 'react-i18next';
import TomlEditor from '@/components/common/TomlEditor';
import type { KimiCommonConfig, KimiCommonConfigInput } from '@/types/kimi';
import { buildKimiCommonConfigSubmitValues } from '../utils/commonConfigForm';
import {
  KIMI_SECONDARY_MODEL_PRIMARY_KEY,
  applyKimiSecondaryModelToml,
  emptyKimiSecondaryModelConfig,
  parseKimiSecondaryModelConfig,
  validateKimiSecondaryModelConfig,
  type KimiSecondaryModelConfig,
} from '../utils/secondaryModelForm';

interface KimiCommonConfigModalProps {
  open: boolean;
  config: KimiCommonConfig | null;
  /** Model alias keys the current provider declares, offered as pool options. */
  modelAliasKeys?: string[];
  onCancel: () => void;
  onSubmit: (config: KimiCommonConfigInput) => Promise<void>;
}

const KimiCommonConfigModal: React.FC<KimiCommonConfigModalProps> = ({
  open,
  config,
  modelAliasKeys = [],
  onCancel,
  onSubmit,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm<KimiCommonConfigInput>();
  const [submitting, setSubmitting] = React.useState(false);
  const [activeTab, setActiveTab] = useState('general');
  const [swarmConfig, setSwarmConfig] = useState<KimiSecondaryModelConfig>(
    emptyKimiSecondaryModelConfig,
  );
  const swarmForce = swarmConfig.force;

  useEffect(() => {
    if (!open) {
      return;
    }
    const stored = config?.config ?? '';
    form.setFieldsValue({ config: stored });
    setSwarmConfig(parseKimiSecondaryModelConfig(stored));
    setActiveTab('general');
  }, [config, form, open]);

  // Pool options come from the applied provider's catalog; a key already stored
  // in the pool must stay selectable even when the catalog no longer lists it.
  const poolOptions = useMemo(() => {
    const keys = new Set<string>(modelAliasKeys);
    for (const model of swarmConfig.models) {
      if (model.trim()) {
        keys.add(model.trim());
      }
    }
    keys.delete(KIMI_SECONDARY_MODEL_PRIMARY_KEY);
    return [...keys].map((key) => ({ value: key, label: key }));
  }, [modelAliasKeys, swarmConfig.models]);

  const handleOk = async () => {
    try {
      const values = await form.validateFields();
      const swarmError = validateKimiSecondaryModelConfig(swarmConfig);
      if (swarmError) {
        setActiveTab('swarm');
        message.error(swarmError);
        return;
      }
      setSubmitting(true);
      // The swarm form owns `[secondary_model]`; every other line of the free
      // TOML stays byte-identical (see applyKimiSecondaryModelToml).
      const merged = applyKimiSecondaryModelToml(values.config ?? '', swarmConfig);
      await onSubmit(buildKimiCommonConfigSubmitValues(merged));
    } catch (error) {
      // Tauri invoke rejections are plain strings, not Error instances.
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setSubmitting(false);
    }
  };

  const generalTab = (
    <Form form={form} layout="vertical">
      <Form.Item name="config">
        <TomlEditorFormItem placeholder={t('kimi.commonConfig.description')} />
      </Form.Item>
    </Form>
  );

  const swarmTab = (
    <Form layout="vertical">
      <Alert
        type="info"
        showIcon
        message={t('kimi.commonConfig.swarmHint')}
        style={{ marginBottom: 16 }}
      />

      <Form.Item
        label={t('kimi.commonConfig.swarmDefaultModel')}
        extra={t('kimi.commonConfig.swarmDefaultModelHint')}
      >
        <Select
          showSearch
          allowClear
          placeholder={t('kimi.commonConfig.swarmDefaultModelPlaceholder')}
          value={swarmConfig.defaultModel || undefined}
          options={poolOptions}
          onChange={(value) =>
            setSwarmConfig((current) => ({ ...current, defaultModel: value ?? '' }))
          }
        />
      </Form.Item>

      <Form.Item
        label={t('kimi.commonConfig.swarmForce')}
        extra={t('kimi.commonConfig.swarmForceHint')}
      >
        <Switch
          checked={swarmConfig.force}
          onChange={(checked) =>
            setSwarmConfig((current) => ({
              ...current,
              force: checked,
              // force and the pool are mutually exclusive in the CLI schema, so
              // enabling force clears the pool instead of failing on save.
              models: checked ? [] : current.models,
            }))
          }
        />
      </Form.Item>

      <Form.Item
        label={t('kimi.commonConfig.swarmModels')}
        extra={t('kimi.commonConfig.swarmModelsHint')}
      >
        <Select
          mode="multiple"
          allowClear
          disabled={swarmForce}
          placeholder={t('kimi.commonConfig.swarmModelsPlaceholder')}
          value={swarmConfig.models}
          options={poolOptions}
          onChange={(value: string[]) =>
            setSwarmConfig((current) => ({ ...current, models: value }))
          }
        />
      </Form.Item>

      <Alert type="warning" showIcon message={t('kimi.commonConfig.swarmGatewayNote')} />
    </Form>
  );

  return (
    <Modal
      open={open}
      title={t('kimi.commonConfig.title')}
      onOk={() => void handleOk()}
      onCancel={onCancel}
      confirmLoading={submitting}
      width={800}
      destroyOnHidden
    >
      <Tabs
        activeKey={activeTab}
        onChange={setActiveTab}
        items={[
          { key: 'general', label: t('kimi.commonConfig.tabGeneral'), children: generalTab },
          { key: 'swarm', label: t('kimi.commonConfig.tabSwarm'), children: swarmTab },
        ]}
      />
    </Modal>
  );
};

// TomlEditor 与 antd Form.Item 集成的包装组件（TomlEditor 的 value 是必填 prop，
// Form.Item 在运行时注入 value/onChange，但 TS 静态类型无法感知，需要桥接）。
const TomlEditorFormItem: React.FC<{
  value?: string;
  onChange?: (value: string) => void;
  placeholder?: string;
}> = ({ value = '', onChange, placeholder }) => (
  <TomlEditor
    value={value}
    onChange={onChange}
    height={300}
    placeholder={placeholder}
  />
);

export default KimiCommonConfigModal;
