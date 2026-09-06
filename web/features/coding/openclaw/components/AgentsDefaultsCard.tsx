import React from 'react';
import { Alert, Form, Modal, Select, Typography, message } from 'antd';
import { useTranslation } from 'react-i18next';
import { setOpenClawAgentsDefaults } from '@/services/openclawApi';
import JsonEditor from '@/components/common/JsonEditor';
import type { OpenClawAgentsDefaults, OpenClawConfig } from '@/types/openclaw';

const { Text } = Typography;

interface Props {
  defaults: OpenClawAgentsDefaults | null;
  config: OpenClawConfig | null;
  onSaved: () => void;
  /** Notified with the provider part of `model.primary` after a successful save. */
  onProviderUsed?: (providerId: string) => void;
}

export interface AgentsDefaultsCardRef {
  openMoreParams: () => void;
}

const formItemLayout = {
  labelCol: { span: 2 },
  wrapperCol: { span: 22 },
};

/** Keys managed by dedicated form fields — excluded from "more params" editor */
const MANAGED_KEYS = new Set(['model', 'models', 'thinkingDefault']);

/**
 * OpenClaw 官方思考等级取值(agents.defaults.thinkingDefault),单值。
 * 比 Hermes 多一档 `adaptive`(provider 托管的动态思考)。
 */
const OPENCLAW_THINKING_LEVELS = [
  'off', 'minimal', 'low', 'medium', 'high', 'xhigh', 'adaptive', 'max', 'ultra',
] as const;

const OPENCLAW_THINKING_OPTIONS = OPENCLAW_THINKING_LEVELS.map((level) => ({ value: level, label: level }));

/** 规范化思考等级输入;非字符串 / 空白 / 不在枚举内返回 undefined。 */
const parseThinkingLevel = (value: unknown): string | undefined => {
  if (typeof value !== 'string') {
    return undefined;
  }
  const trimmed = value.trim();
  return (OPENCLAW_THINKING_LEVELS as readonly string[]).includes(trimmed) ? trimmed : undefined;
};

const AgentsDefaultsCard = React.forwardRef<AgentsDefaultsCardRef, Props>(({ defaults, config, onSaved, onProviderUsed }, ref) => {
  const { t } = useTranslation();

  // Local editable state
  const [primaryModel, setPrimaryModel] = React.useState<string | undefined>(undefined);
  const [fallbacks, setFallbacks] = React.useState<string[]>([]);
  const [thinkingLevel, setThinkingLevel] = React.useState<string | undefined>(undefined);

  // More params modal
  const [moreParamsOpen, setMoreParamsOpen] = React.useState(false);
  const [extraParams, setExtraParams] = React.useState<Record<string, unknown>>({});
  const [extraParamsValid, setExtraParamsValid] = React.useState(true);

  React.useEffect(() => {
    if (defaults) {
      setPrimaryModel(defaults.model?.primary || undefined);
      setFallbacks(defaults.model?.fallbacks || []);
      setThinkingLevel(parseThinkingLevel(defaults.thinkingDefault));
    }
  }, [defaults]);

  // Build model options from all providers
  const modelOptions = React.useMemo(() => {
    if (!config?.models?.providers) return [];
    const groups = new Map<string, { label: string; options: { label: string; value: string }[] }>();

    for (const [providerId, provider] of Object.entries(config.models.providers)) {
      const groupLabel = providerId;
      const entry = groups.get(providerId) || { label: groupLabel, options: [] };

      for (const model of provider.models || []) {
        const fullId = `${providerId}/${model.id}`;
        const modelName = model.name || model.id;
        // Keep provider prefix for each option to avoid same model name confusion.
        entry.options.push({ label: `${providerId} / ${modelName}`, value: fullId });
      }

      groups.set(providerId, entry);
    }

    const result = Array.from(groups.values());
    for (const g of result) {
      g.options.sort((a, b) => a.label.localeCompare(b.label));
    }
    result.sort((a, b) => a.label.localeCompare(b.label));
    return result;
  }, [config]);

  // Build the full defaults object from current state + extra params
  const buildDefaults = React.useCallback((overrides?: {
    primaryModel?: string | undefined;
    fallbacks?: string[];
    thinkingLevel?: string | undefined;
    extra?: Record<string, unknown>;
  }): OpenClawAgentsDefaults => {
    const pm = overrides && 'primaryModel' in overrides ? overrides.primaryModel : primaryModel;
    const fb = overrides && 'fallbacks' in overrides ? overrides.fallbacks : fallbacks;
    const tl = overrides && 'thinkingLevel' in overrides ? overrides.thinkingLevel : thinkingLevel;

    // Start from extra/unknown fields in defaults (excluding managed keys)
    const extraFields: Record<string, unknown> = {};
    if (defaults) {
      for (const [k, v] of Object.entries(defaults)) {
        if (!MANAGED_KEYS.has(k)) {
          extraFields[k] = v;
        }
      }
    }

    // Merge explicit extra overrides if provided
    const extra = overrides?.extra;
    const merged = extra !== undefined ? extra : extraFields;

    const result: OpenClawAgentsDefaults = {
      ...merged,
      model: { primary: pm || '', fallbacks: fb ?? [] },
      models: defaults?.models,
    };

    if (tl) {
      result.thinkingDefault = tl;
    } else {
      delete result.thinkingDefault;
    }

    return result;
  }, [defaults, primaryModel, fallbacks, thinkingLevel]);

  const doSave = React.useCallback(async (overrides?: {
    primaryModel?: string | undefined;
    fallbacks?: string[];
    thinkingLevel?: string | undefined;
    extra?: Record<string, unknown>;
  }) => {
    try {
      const newDefaults = buildDefaults(overrides);
      await setOpenClawAgentsDefaults(newDefaults);
      // `model.primary` is "providerId/modelId"; empty or bare-model values
      // carry no provider and are skipped.
      const primary = newDefaults.model?.primary ?? '';
      const separatorIndex = primary.lastIndexOf('/');
      if (separatorIndex > 0) {
        onProviderUsed?.(primary.slice(0, separatorIndex));
      }
      onSaved();
    } catch (error) {
      console.error('Failed to save agents defaults:', error);
      message.error(t('common.error'));
    }
  }, [buildDefaults, onSaved, onProviderUsed, t]);

  // Select changes save immediately
  const handlePrimaryModelChange = (value: string | undefined) => {
    setPrimaryModel(value);
    doSave({ primaryModel: value });
  };

  const handleFallbacksChange = (value: string[]) => {
    setFallbacks(value);
    doSave({ fallbacks: value });
  };

  const handleThinkingLevelChange = (value: string | undefined) => {
    const next = parseThinkingLevel(value);
    setThinkingLevel(next);
    doSave({ thinkingLevel: next });
  };

  // More params modal
  const handleOpenMoreParams = () => {
    // Extract non-managed fields
    const extra: Record<string, unknown> = {};
    if (defaults) {
      for (const [k, v] of Object.entries(defaults)) {
        if (!MANAGED_KEYS.has(k)) {
          extra[k] = v;
        }
      }
    }
    setExtraParams(extra);
    setExtraParamsValid(true);
    setMoreParamsOpen(true);
  };

  const handleSaveMoreParams = async () => {
    if (!extraParamsValid) {
      message.error(t('common.error'));
      return;
    }
    await doSave({ extra: extraParams });
    setMoreParamsOpen(false);
  };

  // Expose openMoreParams to parent via ref
  React.useImperativeHandle(ref, () => ({
    openMoreParams: handleOpenMoreParams,
  }));

  return (
    <>
      {defaults?.timeout !== undefined && (
        <Alert
          type="warning"
          showIcon
          style={{ marginBottom: 16 }}
          message={t('openclaw.agents.legacyTimeoutHint')}
        />
      )}
      <Form layout="horizontal" {...formItemLayout}>
        {/* Primary Model (left-aligned) + Default Thinking Level (inline, no label) */}
        <Form.Item label={<Text strong>{t('openclaw.agents.primaryModel')}</Text>}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            <Select
              value={primaryModel}
              onChange={handlePrimaryModelChange}
              placeholder={t('openclaw.agents.primaryModelPlaceholder')}
              allowClear
              showSearch
              optionFilterProp="label"
              options={modelOptions}
              optionLabelProp="label"
              style={{ flex: 1 }}
              notFoundContent={t('openclaw.agents.noModels')}
            />
            <Select
              value={thinkingLevel}
              onChange={handleThinkingLevelChange}
              placeholder={t('openclaw.agents.thinkingDefaultPlaceholder')}
              allowClear
              options={OPENCLAW_THINKING_OPTIONS}
              style={{ width: 200, flexShrink: 0 }}
            />
          </div>
        </Form.Item>

        {/* Fallbacks */}
        <Form.Item label={<Text strong>{t('openclaw.agents.fallbacks')}</Text>}>
          <Select
            mode="multiple"
            value={fallbacks}
            onChange={handleFallbacksChange}
            placeholder={t('openclaw.agents.fallbacksPlaceholder')}
            allowClear
            showSearch
            optionFilterProp="label"
            options={modelOptions}
            optionLabelProp="label"
            style={{ width: '100%' }}
            notFoundContent={t('openclaw.agents.noModels')}
          />
        </Form.Item>
      </Form>

      {/* More Parameters Modal */}
      <Modal
        title={t('openclaw.agents.moreParamsTitle')}
        open={moreParamsOpen}
        onCancel={() => setMoreParamsOpen(false)}
        onOk={handleSaveMoreParams}
        okText={t('common.save')}
        cancelText={t('common.cancel')}
        width={600}
        destroyOnHidden
      >
        <JsonEditor
          value={extraParams}
          onChange={(val, valid) => {
            if (typeof val === 'object' && val !== null && !Array.isArray(val)) {
              setExtraParams(val as Record<string, unknown>);
            }
            setExtraParamsValid(valid);
          }}
          height={300}
        />
      </Modal>
    </>
  );
});

AgentsDefaultsCard.displayName = 'AgentsDefaultsCard';

export default AgentsDefaultsCard;
