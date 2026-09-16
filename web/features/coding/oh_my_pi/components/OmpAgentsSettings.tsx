import React from 'react';
import {
  Alert,
  App,
  Button,
  Card,
  Collapse,
  Dropdown,
  Empty,
  Modal,
  Select,
  Space,
  Spin,
  Switch,
  Tag,
  Typography,
} from 'antd';
import {
  AppstoreOutlined,
  CheckOutlined,
  CopyOutlined,
  DeleteOutlined,
  EditOutlined,
  HolderOutlined,
  MoreOutlined,
  PlusOutlined,
} from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import {
  DndContext,
  closestCenter,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from '@dnd-kit/core';
import {
  SortableContext,
  arrayMove,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { restrictToVerticalAxis } from '@dnd-kit/modifiers';
import { CSS } from '@dnd-kit/utilities';

import AppliedTag from '@/components/common/AppliedTag';
import type { OmpAgentsConfig, OmpAgentConfig, OmpRuntimeProviderView } from '@/types/ohMyPi';
import {
  applyOmpAgentsConfig,
  clearOmpAgentsAppliedConfig,
  createOmpAgentsConfig,
  deleteOmpAgentsConfig,
  listOmpAgentsConfigs,
  reorderOmpAgentsConfigs,
  toggleOmpAgentsConfigDisabled,
  updateOmpAgentsConfig,
} from '@/services/ohMyPiApi';
import { refreshTrayMenu } from '@/services/appApi';
import {
  OMP_CORE_MODEL_ROLES,
  OMP_RESERVED_AGENT_NAMES,
  getOmpModelRoleDisplay,
} from '../utils/ompAgentsUtils';
import OmpAgentsConfigModal, { type GroupedModelOptions } from './OmpAgentsConfigModal';

const { Text } = Typography;

interface OmpAgentsSettingsProps {
  modelOptions: GroupedModelOptions;
  providers?: OmpRuntimeProviderView[];
  /** 拖拽排序回调由页面持有(非必填);用于在 apply 后刷新页面级状态。 */
  onConfigApplied?: (config: OmpAgentsConfig) => void;
  onConfigUpdated?: () => void;
}

/** 排序卡片(包一层 useSortable)。 */
const SortableCard: React.FC<{
  config: OmpAgentsConfig;
  isSelected: boolean;
  disabled: boolean;
  onEdit: (config: OmpAgentsConfig) => void;
  onCopy: (config: OmpAgentsConfig) => void;
  onDelete: (config: OmpAgentsConfig) => void;
  onApply: (config: OmpAgentsConfig) => void;
  onToggleDisabled: (config: OmpAgentsConfig, isDisabled: boolean) => void;
  onClearApplied: (config: OmpAgentsConfig) => void;
}> = ({
  config,
  isSelected,
  disabled,
  onEdit,
  onCopy,
  onDelete,
  onApply,
  onToggleDisabled,
  onClearApplied,
}) => {
  const { t } = useTranslation();
  const isLocalConfig = config.id === '__local__';
  const showAsApplied = isSelected && !isLocalConfig;

  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: config.id });

  const sortableStyle = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : config.isDisabled ? 0.6 : 1,
  };

  const roleEntries = React.useMemo(() => {
    const roles = config.modelRoles ?? {};
    const entries: Array<{
      key: string;
      tag: string;
      color?: string;
      model: string;
      thinkingLevel?: string;
    }> = [];
    OMP_CORE_MODEL_ROLES.forEach((roleDef) => {
      const display = getOmpModelRoleDisplay(roles[roleDef.key]);
      if (display.model) {
        entries.push({
          key: roleDef.key,
          tag: roleDef.tag,
          color: roleDef.color,
          model: display.model,
          thinkingLevel: display.thinkingLevel,
        });
      }
    });
    return entries;
  }, [config.modelRoles]);

  const customAgentEntries = React.useMemo(() => {
    const schemeAgents = config.agents ?? {};
    // 只排除 OMP 保留名(main/sub)。像 `task` 这种与核心 role 同名的 agent 是
    // 合法的 bundled 覆盖,过滤掉会让它在保存后静默消失。
    return Object.entries(schemeAgents)
      .filter(([name]) => !OMP_RESERVED_AGENT_NAMES.has(name))
      .map(([name, agent]) => ({
        name,
        description: agent?.description ? String(agent.description) : '',
        model: agent?.model ? String(agent.model) : undefined,
      }));
  }, [config.agents]);

  const configuredCount = roleEntries.length + customAgentEntries.length;

  const menuItems = [
    ...(!isLocalConfig
      ? [
          {
            key: 'toggle',
            label: (
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12 }}>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                  <span>{t('common.enable')}</span>
                  <Text type="secondary" style={{ fontSize: 11 }}>
                    {config.isDisabled
                      ? t('ohMyPi.subagents.configDisabledDesc')
                      : t('ohMyPi.subagents.configEnabledDesc')}
                  </Text>
                </div>
                <Switch
                  checked={!config.isDisabled}
                  size="small"
                  disabled={disabled}
                  onChange={(checked) => onToggleDisabled(config, !checked)}
                />
              </div>
            ),
          },
        ]
      : []),
    {
      key: 'edit',
      label: t('common.edit'),
      icon: <EditOutlined />,
      onClick: () => onEdit(config),
    },
    {
      key: 'copy',
      label: t('common.copy'),
      icon: <CopyOutlined />,
      onClick: () => onCopy(config),
    },
    ...(!isLocalConfig
      ? [
          { type: 'divider' as const },
          {
            key: 'delete',
            label: t('common.delete'),
            icon: <DeleteOutlined />,
            danger: true,
            onClick: () => onDelete(config),
          },
        ]
      : []),
  ];

  return (
    <div ref={setNodeRef} style={sortableStyle}>
      <Card
        size="small"
        style={{
          marginBottom: 12,
          borderColor: showAsApplied ? 'var(--ant-color-primary)' : 'var(--color-border-card)',
          backgroundColor: showAsApplied ? 'var(--color-bg-selected)' : 'var(--color-bg-container)',
          boxShadow: 'var(--shadow-card-sm)',
        }}
        styles={{ body: { padding: '8px 12px' } }}
      >
        <div style={{ display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          <div
            {...attributes}
            {...listeners}
            style={{
              cursor: isDragging ? 'grabbing' : 'grab',
              color: 'var(--color-text-tertiary)',
              touchAction: 'none',
              padding: '4px 0',
            }}
          >
            <HolderOutlined />
          </div>
          <div style={{ flex: 1 }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 16 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 12, flexWrap: 'wrap' }}>
                <Text strong style={{ fontSize: 14 }}>{config.name}</Text>
                {isLocalConfig && (
                  <Text type="secondary" style={{ fontSize: 11 }}>
                    ({t('ohMyPi.subagents.localConfigHint')})
                  </Text>
                )}
                <Tag color="blue" style={{ margin: 0 }}>
                  {configuredCount}
                </Tag>
                {showAsApplied && (
                  <AppliedTag onClick={showAsApplied ? () => onClearApplied(config) : undefined}>
                    {t('ohMyPi.subagents.applied')}
                  </AppliedTag>
                )}
              </div>
              <Space size={4}>
                {!isLocalConfig && !isSelected && (
                  <Button
                    type="link"
                    size="small"
                    icon={<CheckOutlined />}
                    onClick={() => onApply(config)}
                    disabled={disabled || config.isDisabled}
                  >
                    {t('ohMyPi.subagents.apply')}
                  </Button>
                )}
                <Dropdown menu={{ items: menuItems }} trigger={['click']}>
                  <Button type="text" size="small" icon={<MoreOutlined />} disabled={disabled} />
                </Dropdown>
              </Space>
            </div>

            {/* 角色与代理内容概览 */}
            {configuredCount > 0 ? (
              <div style={{ marginTop: 6, display: 'flex', flexDirection: 'column', gap: 6 }}>
                {roleEntries.length > 0 && (
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: '4px 10px', alignItems: 'center' }}>
                    {roleEntries.map((role) => (
                      <span key={role.key} style={{ fontSize: 12, display: 'inline-flex', alignItems: 'center', gap: 4 }}>
                        <Tag color={role.color} style={{ margin: 0, padding: '0 4px', fontSize: 10, lineHeight: '16px', fontWeight: 600 }}>
                          {role.tag}
                        </Tag>
                        <Text strong style={{ fontSize: 12, color: 'var(--color-text)' }}>
                          {role.model}
                        </Text>
                        {role.thinkingLevel && (
                          <Text type="secondary" style={{ fontSize: 11 }}>
                            :{role.thinkingLevel}
                          </Text>
                        )}
                      </span>
                    ))}
                  </div>
                )}
                {customAgentEntries.length > 0 && (
                  <div style={{ display: 'flex', flexWrap: 'wrap', gap: '4px 8px', alignItems: 'center' }}>
                    <Text type="secondary" style={{ fontSize: 11 }}>
                      {t('ohMyPi.subagents.customBadge')}:
                    </Text>
                    {customAgentEntries.map((agent) => (
                      <Tag key={agent.name} color="cyan" style={{ margin: 0, fontSize: 11 }}>
                        {agent.name}
                      </Tag>
                    ))}
                  </div>
                )}
              </div>
            ) : (
              <div style={{ marginTop: 4 }}>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  {t('ohMyPi.subagents.noAgentsConfigured')}
                </Text>
              </div>
            )}
          </div>
        </div>
      </Card>
    </div>
  );
};

const OmpAgentsSettings: React.FC<OmpAgentsSettingsProps> = ({
  modelOptions,
  providers,
  onConfigApplied,
  onConfigUpdated,
}) => {
  const { t } = useTranslation();
  const { message: appMessage } = App.useApp();
  const [loading, setLoading] = React.useState(false);
  const [configs, setConfigs] = React.useState<OmpAgentsConfig[]>([]);
  const [selectedConfigId, setSelectedConfigId] = React.useState('');
  const [modalOpen, setModalOpen] = React.useState(false);
  const [editingConfig, setEditingConfig] = React.useState<OmpAgentsConfig | null>(null);
  const [isCopyMode, setIsCopyMode] = React.useState(false);

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 8 },
    }),
  );

  const loadConfigs = React.useCallback(async () => {
    setLoading(true);
    try {
      const data = await listOmpAgentsConfigs();
      setConfigs(data);
      const applied = data.find((c) => c.isApplied && c.id !== '__local__');
      setSelectedConfigId(applied?.id ?? '');
    } catch (error) {
      console.error('Failed to load OMP agents configs:', error);
      void appMessage.error(t('common.error'));
    } finally {
      setLoading(false);
    }
  }, [appMessage, t]);

  React.useEffect(() => {
    void loadConfigs();
  }, [loadConfigs]);

  const refreshAfterChange = async () => {
    await loadConfigs();
    await refreshTrayMenu();
  };

  const handleAddConfig = () => {
    setEditingConfig(null);
    setIsCopyMode(false);
    setModalOpen(true);
  };

  const handleEditConfig = (config: OmpAgentsConfig) => {
    setEditingConfig(JSON.parse(JSON.stringify(config)));
    setIsCopyMode(false);
    setModalOpen(true);
  };

  const handleCopyConfig = (config: OmpAgentsConfig) => {
    setEditingConfig(JSON.parse(JSON.stringify(config)));
    setIsCopyMode(true);
    setModalOpen(true);
  };

  const handleDeleteConfig = (config: OmpAgentsConfig) => {
    Modal.confirm({
      title: t('common.confirm'),
      content: t('ohMyPi.subagents.confirmDelete', { name: config.name }),
      okButtonProps: { danger: true },
      onOk: async () => {
        try {
          await deleteOmpAgentsConfig(config.id);
          void appMessage.success(t('common.success'));
          await refreshAfterChange();
          onConfigUpdated?.();
        } catch {
          void appMessage.error(t('common.error'));
        }
      },
    });
  };

  const handleApplyConfig = (config: OmpAgentsConfig) => {
    Modal.confirm({
      title: t('ohMyPi.subagents.applyConfirmTitle'),
      content: t('ohMyPi.subagents.applyConfirmContent', { name: config.name }),
      okText: t('ohMyPi.subagents.apply'),
      onOk: async () => {
        try {
          await applyOmpAgentsConfig(config.id);
          void appMessage.success(t('ohMyPi.subagents.applySuccess'));
          await refreshAfterChange();
          onConfigApplied?.(config);
          onConfigUpdated?.();
        } catch (error) {
          void appMessage.error(error instanceof Error ? error.message : t('common.error'));
        }
      },
    });
  };

  const handleClearApplied = (config: OmpAgentsConfig) => {
    Modal.confirm({
      title: t('ohMyPi.subagents.clearAppliedConfirmTitle'),
      content: t('ohMyPi.subagents.clearAppliedConfirmContent', { name: config.name }),
      okText: t('ohMyPi.subagents.clearAppliedConfirmOk'),
      okButtonProps: { danger: true },
      onOk: async () => {
        try {
          await clearOmpAgentsAppliedConfig(config.id);
          void appMessage.success(t('ohMyPi.subagents.clearAppliedSuccess'));
          await refreshAfterChange();
          onConfigUpdated?.();
        } catch (error) {
          void appMessage.error(error instanceof Error ? error.message : t('common.error'));
        }
      },
    });
  };

  const applyToggleDisabled = async (config: OmpAgentsConfig, isDisabled: boolean) => {
    try {
      await toggleOmpAgentsConfigDisabled(config.id, isDisabled);
      void appMessage.success(
        isDisabled ? t('ohMyPi.subagents.configDisabled') : t('ohMyPi.subagents.configEnabled'),
      );
      await refreshAfterChange();
      onConfigUpdated?.();
    } catch {
      void appMessage.error(t('common.error'));
    }
  };

  const handleToggleDisabled = async (config: OmpAgentsConfig, isDisabled: boolean) => {
    if (!isDisabled || !config.isApplied) {
      await applyToggleDisabled(config, isDisabled);
      return;
    }
    // 禁用「已应用」方案会撤回运行目录(清空 agents/*.md 并重置 modelRoles),
    // 属于破坏性操作:下拉里一个小开关不能一点就删,必须二次确认。
    Modal.confirm({
      title: t('ohMyPi.subagents.disableAppliedConfirmTitle'),
      content: t('ohMyPi.subagents.disableAppliedConfirmContent', { name: config.name }),
      okText: t('ohMyPi.subagents.disableAppliedConfirmOk'),
      okButtonProps: { danger: true },
      onOk: () => applyToggleDisabled(config, isDisabled),
    });
  };

  const handleDragEnd = async (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) {
      return;
    }
    const oldIndex = configs.findIndex((c) => c.id === active.id);
    const newIndex = configs.findIndex((c) => c.id === over.id);
    if (oldIndex === -1 || newIndex === -1) {
      return;
    }
    const oldConfigs = [...configs];
    const newConfigs = arrayMove(configs, oldIndex, newIndex);
    setConfigs(newConfigs);
    try {
      await reorderOmpAgentsConfigs(newConfigs.map((c) => c.id));
    } catch {
      setConfigs(oldConfigs);
      void appMessage.error(t('common.error'));
    }
  };

  const handleModalSuccess = async (values: {
    id?: string;
    name: string;
    modelRoles?: Record<string, import('@/types/ohMyPi').OmpModelRoleConfig> | null;
    agents: Record<string, OmpAgentConfig>;
    otherFields?: Record<string, unknown>;
  }): Promise<void> => {
    const agentsForApi = Object.keys(values.agents).length > 0 ? values.agents : null;
    const modelRolesForApi = values.modelRoles ?? null;
    if (editingConfig && editingConfig.id === '__local__') {
      // 保存本地桥接态:新建并应用
      const created = await createOmpAgentsConfig({
        name: values.name,
        modelRoles: modelRolesForApi,
        agents: agentsForApi,
        otherFields: values.otherFields,
      });
      await applyOmpAgentsConfig(created.id);
    } else if (editingConfig && !isCopyMode) {
      await updateOmpAgentsConfig({
        id: editingConfig.id,
        name: values.name,
        modelRoles: modelRolesForApi,
        agents: agentsForApi,
        otherFields: values.otherFields,
      });
    } else {
      await createOmpAgentsConfig({
        name: values.name,
        modelRoles: modelRolesForApi,
        agents: agentsForApi,
        otherFields: values.otherFields,
      });
    }
    void appMessage.success(t('common.success'));
    setModalOpen(false);
    await refreshAfterChange();
    onConfigUpdated?.();
  };

  // 弹窗的初始化 effect 以 `initialValues` 的对象身份为依赖:必须 memo,否则每次
  // 渲染都新建对象,父页面任何重渲染都会把弹窗里正在编辑的内容重置掉。
  const modalInitialValues = React.useMemo(
    () =>
      editingConfig
        ? {
            ...editingConfig,
            id: isCopyMode ? undefined : editingConfig.id,
            name: isCopyMode ? `${editingConfig.name}_copy` : editingConfig.name,
          }
        : undefined,
    [editingConfig, isCopyMode],
  );

  const managedConfigs = configs.filter((c) => c.id !== '__local__');
  const appliedConfig = configs.find((c) => c.isApplied && c.id !== '__local__');
  const selectOptions = managedConfigs.map((config) => ({
    label: config.isApplied ? `${config.name} ✓` : config.name,
    value: config.id,
  }));

  return (
    <>
      <Collapse
        className="omp-agents-settings"
        bordered={false}
        defaultActiveKey={['omp-agents']}
        items={[
          {
            key: 'omp-agents',
            label: (
              <Space>
                <AppstoreOutlined />
                <Text strong>{t('ohMyPi.subagents.title')}</Text>
                {appliedConfig && (
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    {t('ohMyPi.subagents.current')}: {appliedConfig.name}
                  </Text>
                )}
              </Space>
            ),
            extra: (
              <Space>
                <Button
                  type="link"
                  size="small"
                  icon={<PlusOutlined />}
                  onClick={(e) => {
                    e.stopPropagation();
                    handleAddConfig();
                  }}
                >
                  {t('ohMyPi.subagents.addConfig')}
                </Button>
              </Space>
            ),
            children: (
              <Spin spinning={loading}>
                <div style={{ marginBottom: 12 }}>
                  <div style={{ fontSize: 12, color: 'var(--color-text-tertiary)', borderLeft: '2px solid var(--color-border)', paddingLeft: 8, marginBottom: 12 }}>
                    <div>{t('ohMyPi.subagents.sectionHint')}</div>
                    <div>{t('ohMyPi.subagents.sectionWarning')}</div>
                  </div>
                </div>

                {managedConfigs.length > 0 ? (
                  <div style={{ marginBottom: 16 }}>
                    <Space.Compact style={{ width: 360 }}>
                      <Select
                        value={selectedConfigId || undefined}
                        onChange={(configId) => {
                          if (!configId) {
                            return;
                          }
                          void handleApplyConfig(
                            configs.find((c) => c.id === configId) as OmpAgentsConfig,
                          );
                        }}
                        placeholder={t('ohMyPi.subagents.selectConfig')}
                        options={selectOptions}
                        style={{ flex: 1 }}
                        allowClear={false}
                      />
                    </Space.Compact>
                  </div>
                ) : (
                  <Empty
                    image={Empty.PRESENTED_IMAGE_SIMPLE}
                    description={t('ohMyPi.subagents.noConfigs')}
                  />
                )}

                {configs.length === 0 ? (
                  <Empty
                    image={Empty.PRESENTED_IMAGE_SIMPLE}
                    description={t('ohMyPi.subagents.emptyText')}
                    style={{ margin: '24px 0' }}
                  />
                ) : (
                  <DndContext
                    sensors={sensors}
                    collisionDetection={closestCenter}
                    modifiers={[restrictToVerticalAxis]}
                    onDragEnd={handleDragEnd}
                  >
                    <SortableContext
                      items={configs.map((c) => c.id)}
                      strategy={verticalListSortingStrategy}
                    >
                      <div>
                        {configs.map((config) => (
                          <SortableCard
                            key={config.id}
                            config={config}
                            isSelected={config.id === selectedConfigId}
                            disabled={false}
                            onEdit={handleEditConfig}
                            onCopy={handleCopyConfig}
                            onDelete={handleDeleteConfig}
                            onApply={handleApplyConfig}
                            onToggleDisabled={handleToggleDisabled}
                            onClearApplied={handleClearApplied}
                          />
                        ))}
                      </div>
                    </SortableContext>
                  </DndContext>
                )}

                {!managedConfigs.length && (
                  <Alert
                    type="info"
                    showIcon
                    title={t('ohMyPi.subagents.localOnlyHint')}
                    style={{ marginTop: 8 }}
                  />
                )}
              </Spin>
            ),
          },
        ]}
      />

      <OmpAgentsConfigModal
        open={modalOpen}
        isEdit={!isCopyMode && !!editingConfig}
        initialValues={modalInitialValues}
        modelOptions={modelOptions}
        providers={providers}
        onCancel={() => {
          setModalOpen(false);
          setEditingConfig(null);
          setIsCopyMode(false);
        }}
        onSuccess={handleModalSuccess}
      />
    </>
  );
};

export default OmpAgentsSettings;