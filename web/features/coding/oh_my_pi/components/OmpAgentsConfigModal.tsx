import React from 'react';
import {
  Button,
  Collapse,
  Form,
  Input,
  message,
  Modal,
  Select,
  Space,
  Tag,
  Tooltip,
  Typography,
} from 'antd';
import {
  DeleteOutlined,
  InfoCircleOutlined,
  PlusOutlined,
  SettingOutlined,
} from '@ant-design/icons';
import { useTranslation } from 'react-i18next';

import JsonEditor from '@/components/common/JsonEditor';
import type {
  OmpAgentConfig,
  OmpModelRoleConfig,
  OmpRuntimeProviderView,
} from '@/types/ohMyPi';
import {
  OMP_CORE_MODEL_ROLES,
  OMP_CORE_MODEL_ROLE_KEYS,
  OMP_RESERVED_AGENT_NAMES,
  isValidOmpAgentFileName,
  ompAgentConfigToDraft,
  ompAgentDraftToConfig,
  parseOmpModelRoleEntry,
  getOmpThinkingOptionsForModel,
  type OmpAgentFormDraft,
} from '../utils/ompAgentsUtils';
import styles from './OmpAgentsConfigModal.module.less';

const { Text } = Typography;

export type ModelOption = { label: string; value: string; disabled?: boolean };
export type ModelOptionGroup = { label: string; options: ModelOption[] };
export type GroupedModelOptions = Array<ModelOption | ModelOptionGroup>;

interface OmpAgentsConfigModalProps {
  open: boolean;
  isEdit: boolean;
  initialValues?: {
    id?: string;
    name: string;
    modelRoles?: Record<string, OmpModelRoleConfig | string> | null;
    agents?: Record<string, OmpAgentConfig | undefined> | null;
    otherFields?: Record<string, unknown>;
  };
  modelOptions: GroupedModelOptions;
  providers?: OmpRuntimeProviderView[];
  onCancel: () => void;
  onSuccess: (values: {
    id?: string;
    name: string;
    modelRoles?: Record<string, OmpModelRoleConfig> | null;
    agents: Record<string, OmpAgentConfig>;
    otherFields?: Record<string, unknown>;
  }) => Promise<void> | void;
}

interface CustomAgentRowState {
  key: string;
  draft: OmpAgentFormDraft;
  advancedRaw: string;
  advancedValid: boolean;
  advancedExpanded: boolean;
}


const toRaw = (value: unknown): string => {
  if (value === undefined || value === null) {
    return '';
  }
  if (typeof value === 'string') {
    return value;
  }
  return JSON.stringify(value, null, 2);
};

const parseRawAdvanced = (raw: string): Record<string, unknown> => {
  const trimmed = raw.trim();
  if (!trimmed) {
    return {};
  }
  const parsed = JSON.parse(trimmed);
  if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
    throw new Error('invalid-json-object');
  }
  return parsed as Record<string, unknown>;
};

const OmpAgentsConfigModal: React.FC<OmpAgentsConfigModalProps> = ({
  open,
  isEdit,
  initialValues,
  modelOptions,
  providers,
  onCancel,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const [form] = Form.useForm();
  const [name, setName] = React.useState('');
  const [modelRolesState, setModelRolesState] = React.useState<
    Record<string, { model?: string; thinkingLevel?: string }>
  >({});
  const [customAgents, setCustomAgents] = React.useState<CustomAgentRowState[]>([]);
  const [newCustomName, setNewCustomName] = React.useState('');
  const [showAddCustom, setShowAddCustom] = React.useState(false);
  const [saving, setSaving] = React.useState(false);

  /** 核心角色专用选项列表：排除 @role 别名，强制绑定具体供应商模型 */
  const coreRoleModelOptions = React.useMemo(() => {
    return modelOptions
      .map((item) => {
        if ('options' in item) {
          const filtered = item.options.filter((opt) => !opt.value.startsWith('@'));
          if (filtered.length === 0) return null;
          return { ...item, options: filtered };
        }
        if (item.value.startsWith('@')) return null;
        return item;
      })
      .filter((item): item is ModelOption | ModelOptionGroup => item !== null);
  }, [modelOptions]);

  React.useEffect(() => {
    if (!open) {
      return;
    }
    setName(initialValues?.name ?? '');

    // 1. 初始化核心 modelRoles
    const nextRoles: Record<string, { model?: string; thinkingLevel?: string }> = {};
    const rawRoles = initialValues?.modelRoles ?? {};
    for (const roleDef of OMP_CORE_MODEL_ROLES) {
      const rawVal = rawRoles[roleDef.key];
      const parsed = parseOmpModelRoleEntry(rawVal);
      const model = parsed.model || undefined;
      let thinkingLevel = parsed.thinkingLevel || roleDef.defaultThinkingLevel || undefined;
      // 检查当前模型是否支持该思考级别
      if (model) {
        const thinkingInfo = getOmpThinkingOptionsForModel(model, providers);
        if (!thinkingInfo.supported) {
          thinkingLevel = undefined;
        } else if (
          thinkingLevel &&
          !thinkingInfo.options.some((opt) => opt.value === thinkingLevel)
        ) {
          thinkingLevel = thinkingInfo.defaultLevel || undefined;
        }
      }
      nextRoles[roleDef.key] = {
        model,
        thinkingLevel,
      };
    }
    setModelRolesState(nextRoles);

    // 2. 初始化自定义 agents(非核心 modelRoles 且非保留名的条目)
    const nextCustom: CustomAgentRowState[] = [];
    const schemeAgents = initialValues?.agents ?? {};
    if (schemeAgents) {
      Object.entries(schemeAgents).forEach(([agentKey, agent]) => {
        if (!agent || OMP_CORE_MODEL_ROLE_KEYS.has(agentKey) || OMP_RESERVED_AGENT_NAMES.has(agentKey)) {
          return;
        }
        const draft = ompAgentConfigToDraft(agent);
        // 检查自定义 agent 模型与思考级别支持
        if (draft.model) {
          const thinkingInfo = getOmpThinkingOptionsForModel(draft.model, providers);
          if (!thinkingInfo.supported) {
            draft.thinkingLevel = undefined;
          } else if (
            draft.thinkingLevel &&
            !thinkingInfo.options.some((opt) => opt.value === draft.thinkingLevel)
          ) {
            draft.thinkingLevel = thinkingInfo.defaultLevel || undefined;
          }
        }
        // 提取未知字段存入 advanced
        const knownKeys = new Set([
          'name',
          'description',
          'model',
          'thinkingLevel',
          'thinking',
          'tools',
          'blocking',
          'readSummarize',
          'prewalk',
          'advisor',
          'spawns',
          'autoloadSkills',
          'prompt',
        ]);
        const advanced: Record<string, unknown> = {};
        Object.entries(agent).forEach(([k, v]) => {
          if (!knownKeys.has(k)) {
            advanced[k] = v;
          }
        });
        nextCustom.push({
          key: agentKey,
          draft,
          advancedRaw: Object.keys(advanced).length > 0 ? toRaw(advanced) : '',
          advancedValid: true,
          advancedExpanded: false,
        });
      });
    }
    setCustomAgents(nextCustom);
    setNewCustomName('');
    setShowAddCustom(false);
    form.resetFields();
  }, [open, initialValues, form, providers]);

  const handleRoleChange = (
    roleKey: string,
    field: 'model' | 'thinkingLevel',
    value: string | undefined,
  ) => {
    setModelRolesState((prev) => {
      const current = prev[roleKey] || {};
      if (field === 'model') {
        const nextThinkingInfo = getOmpThinkingOptionsForModel(value, providers);
        let nextThinkingLevel = current.thinkingLevel;
        if (!nextThinkingInfo.supported) {
          nextThinkingLevel = undefined;
        } else if (
          nextThinkingLevel &&
          !nextThinkingInfo.options.some((opt) => opt.value === nextThinkingLevel)
        ) {
          nextThinkingLevel = nextThinkingInfo.defaultLevel || undefined;
        }
        return {
          ...prev,
          [roleKey]: {
            model: value || undefined,
            thinkingLevel: nextThinkingLevel,
          },
        };
      }
      return {
        ...prev,
        [roleKey]: {
          ...current,
          thinkingLevel: value || undefined,
        },
      };
    });
  };

  const handleCustomAgentModelChange = (agentKey: string, newModel?: string) => {
    const nextThinkingInfo = getOmpThinkingOptionsForModel(newModel, providers);
    updateCustomAgentDraft(agentKey, (d) => {
      let nextThinking = d.thinkingLevel;
      if (!nextThinkingInfo.supported) {
        nextThinking = undefined;
      } else if (
        nextThinking &&
        !nextThinkingInfo.options.some((opt) => opt.value === nextThinking)
      ) {
        nextThinking = nextThinkingInfo.defaultLevel || undefined;
      }
      return {
        ...d,
        model: newModel,
        thinkingLevel: nextThinking,
      };
    });
  };

  const handleAddCustomAgent = () => {
    const trimmed = newCustomName.trim();
    if (!trimmed) {
      message.error(t('ohMyPi.subagents.agentKeyRequired'));
      return;
    }
    if (!isValidOmpAgentFileName(trimmed)) {
      message.error(t('ohMyPi.subagents.agentKeyInvalid'));
      return;
    }
    if (OMP_RESERVED_AGENT_NAMES.has(trimmed)) {
      message.error(t('ohMyPi.subagents.agentKeyReserved'));
      return;
    }
    if (OMP_CORE_MODEL_ROLE_KEYS.has(trimmed)) {
      message.error(t('ohMyPi.subagents.agentKeyBuiltin'));
      return;
    }
    if (customAgents.some((c) => c.key === trimmed)) {
      message.error(t('ohMyPi.subagents.agentKeyDuplicate'));
      return;
    }
    const newDraft: OmpAgentFormDraft = {
      name: trimmed,
      description: '',
      tools: [],
      autoloadSkills: [],
    };
    setCustomAgents((prev) => [
      ...prev,
      {
        key: trimmed,
        draft: newDraft,
        advancedRaw: '',
        advancedValid: true,
        advancedExpanded: false,
      },
    ]);
    setNewCustomName('');
    setShowAddCustom(false);
  };

  const handleDeleteCustomAgent = (key: string) => {
    setCustomAgents((prev) => prev.filter((c) => c.key !== key));
  };

  const updateCustomAgentDraft = (
    key: string,
    updater: (current: OmpAgentFormDraft) => OmpAgentFormDraft,
  ) => {
    setCustomAgents((prev) =>
      prev.map((item) => (item.key === key ? { ...item, draft: updater(item.draft) } : item)),
    );
  };

  const handleSubmit = async () => {
    const trimmedName = name.trim();
    if (!trimmedName) {
      message.error(t('ohMyPi.subagents.nameRequired'));
      return;
    }

    // 检查自定义 agent 中的 JSON 是否合法
    for (const agent of customAgents) {
      if (!agent.advancedValid) {
        message.error(t('ohMyPi.subagents.invalidAgentJson'));
        return;
      }
    }

    // 组装 modelRoles
    const cleanModelRoles: Record<string, OmpModelRoleConfig> = {};
    for (const [roleKey, roleVal] of Object.entries(modelRolesState)) {
      if (roleVal.model && roleVal.model.trim()) {
        cleanModelRoles[roleKey] = {
          model: roleVal.model.trim(),
          thinkingLevel: roleVal.thinkingLevel ? roleVal.thinkingLevel.trim() : undefined,
        };
      }
    }

    // 组装自定义 agents
    const cleanAgents: Record<string, OmpAgentConfig> = {};
    for (const agent of customAgents) {
      let advancedObj: Record<string, unknown> = {};
      try {
        advancedObj = parseRawAdvanced(agent.advancedRaw);
      } catch {
        message.error(t('ohMyPi.subagents.invalidAgentJson'));
        return;
      }
      const baseConfig = ompAgentDraftToConfig(agent.draft, advancedObj);
      baseConfig.name = agent.key;
      cleanAgents[agent.key] = baseConfig;
    }

    setSaving(true);
    try {
      await onSuccess({
        id: initialValues?.id,
        name: trimmedName,
        modelRoles: Object.keys(cleanModelRoles).length > 0 ? cleanModelRoles : null,
        agents: cleanAgents,
        otherFields: initialValues?.otherFields,
      });
      onCancel();
    } catch (err) {
      logError(err);
    } finally {
      setSaving(false);
    }
  };

  const logError = (err: unknown) => {
    const msg = err instanceof Error ? err.message : String(err);
    message.error(msg);
  };

  return (
    <Modal
      title={isEdit ? t('ohMyPi.subagents.editConfig') : t('ohMyPi.subagents.addConfig')}
      open={open}
      width={860}
      onCancel={onCancel}
      onOk={() => void handleSubmit()}
      okText={t('common.save')}
      cancelText={t('common.cancel')}
      confirmLoading={saving}
      destroyOnHidden
    >
      <div className={styles.modalContent}>
        {/* 方案名称 */}
        <div className={styles.nameRow}>
          <Text className={styles.nameLabel}>{t('ohMyPi.subagents.configName')}</Text>
          <Input
            value={name}
            placeholder={t('ohMyPi.subagents.configNamePlaceholder')}
            onChange={(e) => setName(e.target.value)}
            style={{ flex: 1 }}
          />
        </div>

        {/* 第一部分：核心模型角色 (modelRoles) */}
        <div className={styles.sectionCard}>
          <div className={styles.sectionHeader}>
            <Text className={styles.sectionTitle}>{t('ohMyPi.subagents.coreRolesTitle')}</Text>
          </div>
          <Text className={styles.sectionDesc}>{t('ohMyPi.subagents.coreRolesDesc')}</Text>

          <div className={styles.rolesGrid}>
            {OMP_CORE_MODEL_ROLES.map((roleDef) => {
              const currentVal = modelRolesState[roleDef.key] || {};
              const roleDesc = t(roleDef.descriptionKey);
              const roleThinkingInfo = getOmpThinkingOptionsForModel(
                currentVal.model,
                providers,
              );

              return (
                <div key={roleDef.key} className={styles.roleRow}>
                  <div className={styles.roleInfo}>
                    <Tag
                      color={roleDef.color}
                      className={styles.roleTag}
                    >
                      {roleDef.tag}
                    </Tag>
                    <span className={styles.roleName}>{roleDef.name}</span>
                    <Tooltip title={roleDesc}>
                      <InfoCircleOutlined className={styles.roleDescIcon} />
                    </Tooltip>
                  </div>

                  <Select
                    showSearch
                    allowClear
                    placeholder={t('ohMyPi.subagents.modelPlaceholder')}
                    options={coreRoleModelOptions}
                    value={currentVal.model || undefined}
                    onChange={(val) => handleRoleChange(roleDef.key, 'model', val)}
                    filterOption={(input, option) => {
                      const label = typeof option?.label === 'string' ? option.label : '';
                      const value = option && 'value' in option ? String(option.value) : '';
                      const query = input.toLowerCase();
                      return (
                        label.toLowerCase().includes(query) || value.toLowerCase().includes(query)
                      );
                    }}
                  />

                  <Select
                    allowClear
                    disabled={!roleThinkingInfo.supported}
                    placeholder={
                      roleThinkingInfo.placeholderKey
                        ? t(roleThinkingInfo.placeholderKey)
                        : t('ohMyPi.subagents.thinkingLevelPlaceholder')
                    }
                    options={roleThinkingInfo.options}
                    value={currentVal.thinkingLevel || undefined}
                    onChange={(val) => handleRoleChange(roleDef.key, 'thinkingLevel', val)}
                  />
                </div>
              );
            })}
          </div>
        </div>

        {/* 第二部分：自定义 Subagents (<agentDir>/agents/*.md) */}
        <div className={styles.sectionCard}>
          <div className={styles.sectionHeader}>
            <div>
              <Text className={styles.sectionTitle}>
                {t('ohMyPi.subagents.customAgentsTitle')}
              </Text>
            </div>
            {!showAddCustom && (
              <Button
                type="dashed"
                size="small"
                icon={<PlusOutlined />}
                onClick={() => setShowAddCustom(true)}
              >
                {t('ohMyPi.subagents.addCustomAgent')}
              </Button>
            )}
          </div>
          <Text className={styles.sectionDesc}>{t('ohMyPi.subagents.customAgentsDesc')}</Text>

          {showAddCustom && (
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 0' }}>
              <Input
                size="small"
                value={newCustomName}
                placeholder={t('ohMyPi.subagents.newAgentPlaceholder')}
                onChange={(e) => setNewCustomName(e.target.value)}
                onPressEnter={handleAddCustomAgent}
                style={{ width: 220 }}
              />
              <Button size="small" type="primary" onClick={handleAddCustomAgent}>
                {t('common.add')}
              </Button>
              <Button size="small" onClick={() => setShowAddCustom(false)}>
                {t('common.cancel')}
              </Button>
              <Text type="secondary" style={{ fontSize: 11 }}>
                {t('ohMyPi.subagents.agentNameHint')}
              </Text>
            </div>
          )}

          {customAgents.length === 0 ? (
            <div className={styles.emptyCustomAgents}>
              {t('ohMyPi.subagents.noCustomAgents')}
            </div>
          ) : (
            <Collapse
              ghost
              items={customAgents.map((agent) => ({
                key: agent.key,
                label: (
                  <Space style={{ width: '100%', justifyContent: 'space-between' }}>
                    <Space>
                      <Tag color="cyan">{agent.key}</Tag>
                      <Text type="secondary" style={{ fontSize: 12 }}>
                        {agent.draft.description || t('ohMyPi.subagents.descriptionPlaceholder')}
                      </Text>
                    </Space>
                    <Button
                      type="text"
                      danger
                      size="small"
                      icon={<DeleteOutlined />}
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDeleteCustomAgent(agent.key);
                      }}
                    />
                  </Space>
                ),
                children: (
                  <div className={styles.customAgentCard}>
                    <div className={styles.agentFormRow}>
                      <span className={styles.agentFormLabel}>{t('ohMyPi.subagents.description')}</span>
                      <Input
                        value={agent.draft.description || ''}
                        placeholder={t('ohMyPi.subagents.descriptionPlaceholder')}
                        onChange={(e) =>
                          updateCustomAgentDraft(agent.key, (d) => ({
                            ...d,
                            description: e.target.value,
                          }))
                        }
                      />
                    </div>

                    <div className={styles.agentFormRow}>
                      <span className={styles.agentFormLabel}>{t('ohMyPi.subagents.model')}</span>
                      {(() => {
                        const agentThinkingInfo = getOmpThinkingOptionsForModel(
                          agent.draft.model,
                          providers,
                        );
                        return (
                          <div style={{ display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) 140px', gap: 8 }}>
                            <Select
                              showSearch
                              allowClear
                              placeholder={t('ohMyPi.subagents.modelPlaceholder')}
                              options={modelOptions}
                              value={agent.draft.model || undefined}
                              onChange={(val) => handleCustomAgentModelChange(agent.key, val)}
                              filterOption={(input, option) => {
                                const label = typeof option?.label === 'string' ? option.label : '';
                                const value = option && 'value' in option ? String(option.value) : '';
                                const query = input.toLowerCase();
                                return (
                                  label.toLowerCase().includes(query) ||
                                  value.toLowerCase().includes(query)
                                );
                              }}
                            />
                            <Select
                              allowClear
                              disabled={!agentThinkingInfo.supported}
                              placeholder={
                                agentThinkingInfo.placeholderKey
                                  ? t(agentThinkingInfo.placeholderKey)
                                  : t('ohMyPi.subagents.thinkingLevelPlaceholder')
                              }
                              options={agentThinkingInfo.options}
                              value={agent.draft.thinkingLevel || undefined}
                              onChange={(val) =>
                                updateCustomAgentDraft(agent.key, (d) => ({ ...d, thinkingLevel: val }))
                              }
                            />
                          </div>
                        );
                      })()}
                    </div>

                    <div className={styles.agentFormRow}>
                      <span className={styles.agentFormLabel}>{t('ohMyPi.subagents.tools')}</span>
                      <Select
                        mode="tags"
                        allowClear
                        placeholder={t('ohMyPi.subagents.toolsPlaceholder')}
                        value={agent.draft.tools}
                        onChange={(val) =>
                          updateCustomAgentDraft(agent.key, (d) => ({ ...d, tools: val }))
                        }
                      />
                    </div>

                    <div className={styles.agentFormRow}>
                      <span className={styles.agentFormLabel}>{t('ohMyPi.subagents.prompt')}</span>
                      <Input.TextArea
                        rows={3}
                        value={agent.draft.prompt || ''}
                        placeholder={t('ohMyPi.subagents.promptPlaceholder')}
                        onChange={(e) =>
                          updateCustomAgentDraft(agent.key, (d) => ({
                            ...d,
                            prompt: e.target.value,
                          }))
                        }
                      />
                    </div>

                    <div style={{ marginTop: 4 }}>
                      <Button
                        type="link"
                        size="small"
                        icon={<SettingOutlined />}
                        onClick={() =>
                          setCustomAgents((prev) =>
                            prev.map((c) =>
                              c.key === agent.key
                                ? { ...c, advancedExpanded: !c.advancedExpanded }
                                : c,
                            ),
                          )
                        }
                        style={{ paddingLeft: 0 }}
                      >
                        {t('ohMyPi.subagents.advanced')}
                      </Button>
                      {agent.advancedExpanded && (
                        <div style={{ marginTop: 8 }}>
                          <JsonEditor
                            value={agent.advancedRaw}
                            height={120}
                            placeholder="{}"
                            onRawChange={(raw) => {
                              setCustomAgents((prev) =>
                                prev.map((c) =>
                                  c.key === agent.key ? { ...c, advancedRaw: raw } : c,
                                ),
                              );
                            }}
                            onChange={(_val, valid) => {
                              setCustomAgents((prev) =>
                                prev.map((c) =>
                                  c.key === agent.key ? { ...c, advancedValid: valid } : c,
                                ),
                              );
                            }}
                          />
                        </div>
                      )}
                    </div>
                  </div>
                ),
              }))}
            />
          )}
        </div>
      </div>
    </Modal>
  );
};

export default OmpAgentsConfigModal;