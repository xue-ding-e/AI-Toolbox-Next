import React from 'react';
import { App, Empty, Select, Typography } from 'antd';
import { RobotOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { saveOmoNativeModelSettings } from '@/services/omoNativeApi';
import type {
  OmoNativeBuiltinProvider,
  OmoNativeProvider,
  OmoNativeRuntimeConfig,
} from '@/types/omoNative';
import { getOmoNativeModelEntries, getStringField } from '../utils/omoNativeProviders';
import styles from './OmoNativeModelSettingsSection.module.less';

const { Text, Title } = Typography;

/**
 * 引擎的思考级别（`docs/settings.md` 的 "Model & Thinking"）。
 *
 * 与 Pi 同一份名单——两边都是 senpi 引擎，`thinkingLevelMap` 的键就是这些。
 */
const OMO_NATIVE_THINKING_LEVELS = [
  'off',
  'minimal',
  'low',
  'medium',
  'high',
  'xhigh',
  'max',
];

/** 模型级 `thinkingLevelMap` 里被显式关掉（值为 `null`）的级别不列出来。 */
const supportedThinkingLevels = (
  model: Record<string, unknown> | undefined,
): string[] => {
  if (!model || model.reasoning === false) return [];
  const map = model.thinkingLevelMap;
  if (!map || typeof map !== 'object' || Array.isArray(map)) {
    return model.reasoning === true ? OMO_NATIVE_THINKING_LEVELS : [];
  }
  const entries = map as Record<string, unknown>;
  return OMO_NATIVE_THINKING_LEVELS.filter((level) => entries[level] !== null);
};

interface OmoNativeModelSettingsSectionProps {
  runtimeConfig: OmoNativeRuntimeConfig | null;
  providers: OmoNativeProvider[];
  builtinProviders: OmoNativeBuiltinProvider[];
  onSaved: (config: OmoNativeRuntimeConfig) => void;
}

/**
 * 「模型设置」区块：`settings.json` 里的 `defaultProvider` / `defaultModel` /
 * `defaultThinkingLevel`——引擎启动时选中的 provider / 模型 / 思考级别，
 * 也就是 `/model` 里 Ctrl+S 保存的那三个键。
 *
 * 放在 OmO 页面的**第一个**区块，与 OpenCode / Pi 同位置（2026-10-07 用户要求）。
 *
 * 三个下拉都是**选中即保存**（没有确定按钮）：这是「当前默认值」这类设置的
 * 常见形态，与 Pi 一致。
 */
const OmoNativeModelSettingsSection: React.FC<OmoNativeModelSettingsSectionProps> = ({
  runtimeConfig,
  providers,
  builtinProviders,
  onSaved,
}) => {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [saving, setSaving] = React.useState(false);

  const selection = runtimeConfig?.modelSettings;

  /**
   * provider 下拉项：**只列可用的**。
   *
   * 引擎要求 provider 有 auth 才把它列进 `/model`（`docs/models.md`），所以没配
   * 凭据的渠道选不了、也不该出现在这里——`providers` 数组里那 48 个内建名字
   * 大部分是没凭据的（它们只是「另有 N 个引擎内建」的计数来源）。
   *
   * 两处来源：
   * - `builtinProviders`：已过 `omo auth check` 的内建渠道（含环境变量命中）；
   * - `providers` 里 `custom` 且配了密钥的：用户自己在 `models.json` 写的。
   */
  const providerOptions = React.useMemo(() => {
    const options = new Map<string, string>();
    builtinProviders.forEach((provider) => {
      options.set(provider.id, `${provider.name} (${provider.id})`);
    });
    providers
      .filter((provider) => provider.custom && provider.apiKey)
      .forEach((provider) => {
        // 自定义条目对同名内建 key 的覆盖：用户自己的显示名更该被看到。
        const name = getStringField(provider.config, 'name') || provider.key;
        options.set(provider.key, `${name} (${provider.key})`);
      });
    // 当前值可能已经不在可用列表里（凭据被删了），补一个选项——
    // 否则下拉显示空白，用户会以为从来没设置过。
    const current = selection?.providerKey;
    if (current && !options.has(current)) {
      options.set(current, current);
    }
    return Array.from(options.entries()).map(([value, label]) => ({ value, label }));
  }, [providers, builtinProviders, selection?.providerKey]);

  /** 当前 provider 的模型目录：自定义优先，其次已配置的内建渠道。 */
  const modelOptions = React.useMemo(() => {
    const providerKey = selection?.providerKey;
    if (!providerKey) return [];
    const modelIds = new Set<string>();
    const custom = providers.find((provider) => provider.key === providerKey);
    if (custom) {
      getOmoNativeModelEntries(custom.config).forEach((entry) => modelIds.add(entry.id));
    }
    const builtin = builtinProviders.find((provider) => provider.id === providerKey);
    builtin?.models.forEach((model) => modelIds.add(model.id));
    const current = selection?.modelId;
    if (current) modelIds.add(current);
    return Array.from(modelIds).map((modelId) => ({ value: modelId, label: modelId }));
  }, [providers, builtinProviders, selection?.providerKey, selection?.modelId]);

  /** 当前模型的思考级别选项；模型目录里找不到就退化成引擎的全量名单。 */
  const thinkingLevelOptions = React.useMemo(() => {
    const providerKey = selection?.providerKey;
    const modelId = selection?.modelId;
    if (!providerKey || !modelId) return [];
    const custom = providers.find((provider) => provider.key === providerKey);
    const model = custom
      ? getOmoNativeModelEntries(custom.config).find((entry) => entry.id === modelId)?.model
      : undefined;
    const levels = supportedThinkingLevels(model);
    return levels.map((level) => ({ value: level, label: level }));
  }, [providers, selection?.providerKey, selection?.modelId]);

  const persist = React.useCallback(
    async (input: {
      defaultProvider?: string;
      defaultModel?: string;
      defaultThinkingLevel?: string;
    }) => {
      setSaving(true);
      try {
        const nextConfig = await saveOmoNativeModelSettings(input);
        onSaved(nextConfig);
      } catch (error) {
        console.error('Failed to save OmO model settings:', error);
        message.error(error instanceof Error ? error.message : t('common.error'));
      } finally {
        setSaving(false);
      }
    },
    [message, onSaved, t],
  );

  /**
   * 改 provider 时**顺手清掉模型与思考级别**：它们属于旧 provider，
   * 留着会让引擎启动时去找一个不存在的组合。
   */
  const handleProviderChange = (value?: string) => {
    void persist({ defaultProvider: value ?? '', defaultModel: '', defaultThinkingLevel: '' });
  };

  const handleModelChange = (value?: string) => {
    void persist({ defaultModel: value ?? '', defaultThinkingLevel: '' });
  };

  const handleThinkingLevelChange = (value?: string) => {
    void persist({ defaultThinkingLevel: value ?? '' });
  };

  return (
    <div
      id="omo-native-model-settings"
      data-sidebar-section="true"
      data-sidebar-title={t('omoNative.modelSettings.title')}
    >
      {/* 不可折叠的卡片，与 Pi / OpenCode 的同名区块同形（2026-10-07 用户要求）。 */}
      <div className={styles.modelCard}>
        <Title level={5} className={styles.modelCardTitle}>
          <RobotOutlined style={{ marginRight: 8 }} />
          {t('omoNative.modelSettings.title')}
        </Title>
        <div className={styles.modelCardContent}>
          <div className={styles.modelSettingsGrid}>
            <div>
              <div style={{ marginBottom: 4 }}>
                <Text>{t('omoNative.modelSettings.defaultProvider')}</Text>
              </div>
              <Select
                value={selection?.providerKey}
                onChange={handleProviderChange}
                placeholder={t('omoNative.modelSettings.defaultProviderPlaceholder')}
                allowClear
                showSearch
                options={providerOptions}
                style={{ width: '100%' }}
                disabled={saving}
                notFoundContent={
                  <Empty
                    image={Empty.PRESENTED_IMAGE_SIMPLE}
                    description={t('omoNative.modelSettings.noProviders')}
                  />
                }
              />
            </div>

            <div>
              <div style={{ marginBottom: 4 }}>
                <Text>{t('omoNative.modelSettings.defaultModel')}</Text>
              </div>
              <Select
                value={selection?.modelId}
                onChange={handleModelChange}
                placeholder={t('omoNative.modelSettings.defaultModelPlaceholder')}
                allowClear
                showSearch
                options={modelOptions}
                style={{ width: '100%' }}
                disabled={saving || !selection?.providerKey}
                notFoundContent={
                  <Empty
                    image={Empty.PRESENTED_IMAGE_SIMPLE}
                    description={t('omoNative.modelSettings.noModels')}
                  />
                }
              />
            </div>

            <div>
              <div style={{ marginBottom: 4 }}>
                <Text>{t('omoNative.modelSettings.thinkingLevel')}</Text>
              </div>
              <Select
                value={selection?.thinkingLevel}
                onChange={handleThinkingLevelChange}
                placeholder={t('omoNative.modelSettings.thinkingLevelPlaceholder')}
                allowClear
                showSearch
                options={thinkingLevelOptions}
                style={{ width: '100%' }}
                disabled={saving || !selection?.modelId}
                notFoundContent={
                  <Empty
                    image={Empty.PRESENTED_IMAGE_SIMPLE}
                    description={t('omoNative.modelSettings.noThinkingLevels')}
                  />
                }
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default OmoNativeModelSettingsSection;
