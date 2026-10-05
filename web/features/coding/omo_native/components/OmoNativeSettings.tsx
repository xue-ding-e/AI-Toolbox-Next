import React from 'react';
import { App, Button, Collapse, Empty, Modal, Space, Spin, Tag, Typography } from 'antd';
import {
  AppstoreOutlined,
  LinkOutlined,
  PlusOutlined,
  ThunderboltOutlined,
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
import { SortableContext, arrayMove, verticalListSortingStrategy } from '@dnd-kit/sortable';
import { restrictToVerticalAxis } from '@dnd-kit/modifiers';
import type { OmoNativeAgentsConfig, OmoNativeAgentsConfigInput } from '@/types/omoNative';
import {
  applyOmoNativeAgentsConfig,
  clearOmoNativeAppliedConfig,
  createOmoNativeAgentsConfig,
  deleteOmoNativeAgentsConfig,
  listOmoNativeAgentsConfigs,
  reorderOmoNativeAgentsConfigs,
  saveOmoNativeLocalConfig,
  toggleOmoNativeAgentsConfigDisabled,
  updateOmoNativeAgentsConfig,
} from '@/services/omoNativeApi';
import { openExternalUrl } from '@/services';
import { refreshTrayMenu } from '@/services/appApi';
import OmoNativeConfigCard from './OmoNativeConfigCard';
import OmoNativeConfigModal, {
  type OmoNativeConfigFormValues,
} from './OmoNativeConfigModal';

const { Text, Link } = Typography;

const OMO_DOCS_URL =
  'https://github.com/code-yeongyu/oh-my-openagent/blob/dev/docs/reference/omo-json.md';

interface OmoNativeSettingsProps {
  /** 刷新外部（页面/托盘）的触发器。 */
  onConfigUpdated?: () => void;
}

const OmoNativeSettings: React.FC<OmoNativeSettingsProps> = ({ onConfigUpdated }) => {
  const { t } = useTranslation();
  const { message } = App.useApp();
  const [loading, setLoading] = React.useState(false);
  const [configs, setConfigs] = React.useState<OmoNativeAgentsConfig[]>([]);
  const [modalOpen, setModalOpen] = React.useState(false);
  const [editingConfig, setEditingConfig] = React.useState<OmoNativeAgentsConfig | null>(null);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
  );

  const loadConfigs = React.useCallback(async () => {
    setLoading(true);
    try {
      setConfigs(await listOmoNativeAgentsConfigs());
    } catch (error) {
      console.error('Failed to load OmO Native configs:', error);
      message.error(t('common.error'));
    } finally {
      setLoading(false);
    }
  }, [t, message]);

  React.useEffect(() => {
    void loadConfigs();
  }, [loadConfigs]);

  const appliedConfig = configs.find((config) => config.isApplied && config.id !== '__local__');
  const hasLocal = configs.some((config) => config.id === '__local__');

  const handleAdd = () => {
    setEditingConfig(null);
    setModalOpen(true);
  };

  const handleEdit = (config: OmoNativeAgentsConfig) => {
    setEditingConfig(JSON.parse(JSON.stringify(config)) as OmoNativeAgentsConfig);
    setModalOpen(true);
  };

  const handleDelete = (config: OmoNativeAgentsConfig) => {
    Modal.confirm({
      title: t('common.confirm'),
      content: t('omoNative.confirmDelete', { name: config.name }),
      onOk: async () => {
        try {
          await deleteOmoNativeAgentsConfig(config.id);
          message.success(t('common.success'));
          await loadConfigs();
          await refreshTrayMenu();
          onConfigUpdated?.();
        } catch (error) {
          console.error('Failed to delete OmO Native config:', error);
          message.error(t('common.error'));
        }
      },
    });
  };

  const handleApply = async (config: OmoNativeAgentsConfig) => {
    try {
      await applyOmoNativeAgentsConfig(config.id);
      message.success(t('omoNative.applySuccess'));
      await loadConfigs();
      await refreshTrayMenu();
      onConfigUpdated?.();
    } catch (error) {
      console.error('Failed to apply OmO Native config:', error);
      message.error((error as Error).message || t('common.error'));
    }
  };

  const handleToggleDisabled = async (
    config: OmoNativeAgentsConfig,
    isDisabled: boolean,
  ) => {
    const run = async () => {
      try {
        await toggleOmoNativeAgentsConfigDisabled(config.id, isDisabled);
        message.success(t('common.success'));
        await loadConfigs();
        await refreshTrayMenu();
        onConfigUpdated?.();
      } catch (error) {
        console.error('Failed to toggle OmO Native config:', error);
        message.error(t('common.error'));
      }
    };

    if (isDisabled && config.isApplied) {
      Modal.confirm({
        title: t('omoNative.disableAppliedTitle'),
        content: t('omoNative.disableAppliedContent', { name: config.name }),
        okButtonProps: { danger: true },
        onOk: run,
      });
      return;
    }
    await run();
  };

  const handleDragEnd = async (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;

    const oldIndex = configs.findIndex((config) => config.id === active.id);
    const newIndex = configs.findIndex((config) => config.id === over.id);
    if (oldIndex === -1 || newIndex === -1) return;

    const previous = [...configs];
    const next = arrayMove(configs, oldIndex, newIndex);
    setConfigs(next);
    try {
      await reorderOmoNativeAgentsConfigs(next.map((config) => config.id));
      await refreshTrayMenu();
    } catch (error) {
      console.error('Failed to reorder OmO Native configs:', error);
      setConfigs(previous);
      message.error(t('common.error'));
    }
  };

  const handleModalSuccess = async (values: OmoNativeConfigFormValues) => {
    const isLocalConfig = editingConfig?.id === '__local__';
    const input: OmoNativeAgentsConfigInput = {
      id: isLocalConfig ? undefined : editingConfig?.id,
      name: values.name,
      isApplied: isLocalConfig ? false : editingConfig?.isApplied ?? false,
      isDisabled: editingConfig?.isDisabled ?? false,
      agents: values.agents,
      categories: values.categories,
      modelProfiles: values.modelProfiles,
      modelProfile: values.modelProfile.trim() === '' ? null : values.modelProfile.trim(),
      task: values.task,
      otherFields: values.otherFields,
    };

    if (isLocalConfig) {
      await saveOmoNativeLocalConfig(input);
    } else if (editingConfig) {
      await updateOmoNativeAgentsConfig(input);
    } else {
      await createOmoNativeAgentsConfig(input);
    }

    message.success(t('common.success'));
    setModalOpen(false);
    setEditingConfig(null);
    await loadConfigs();
    await refreshTrayMenu();
    onConfigUpdated?.();
  };

  const handleClearApplied = () => {
    if (!appliedConfig) return;
    Modal.confirm({
      title: t('omoNative.clearAppliedTitle'),
      content: t('omoNative.clearAppliedContent', { name: appliedConfig.name }),
      okButtonProps: { danger: true },
      onOk: async () => {
        try {
          await clearOmoNativeAppliedConfig(appliedConfig.id);
          message.success(t('omoNative.clearAppliedSuccess'));
          await loadConfigs();
          await refreshTrayMenu();
          onConfigUpdated?.();
        } catch (error) {
          console.error('Failed to clear OmO Native applied config:', error);
          message.error(t('common.error'));
        }
      },
    });
  };

  const content = (
    <Spin spinning={loading}>
      <div
        style={{
          fontSize: 12,
          color: 'rgba(0,0,0,0.45)',
          borderLeft: '2px solid rgba(0,0,0,0.12)',
          paddingLeft: 8,
          marginBottom: 12,
        }}
      >
        <div>{t('omoNative.sectionHint')}</div>
      </div>

      {configs.length === 0 ? (
        <Empty description={t('omoNative.emptyText')} style={{ margin: '24px 0' }} />
      ) : (
        <DndContext
          sensors={sensors}
          collisionDetection={closestCenter}
          modifiers={[restrictToVerticalAxis]}
          onDragEnd={handleDragEnd}
        >
          <SortableContext
            items={configs.map((config) => config.id)}
            strategy={verticalListSortingStrategy}
          >
            <div>
              {configs.map((config) => (
                <OmoNativeConfigCard
                  key={config.id}
                  config={config}
                  isLocal={config.id === '__local__'}
                  onEdit={handleEdit}
                  onDelete={handleDelete}
                  onApply={handleApply}
                  onToggleDisabled={handleToggleDisabled}
                />
              ))}
            </div>
          </SortableContext>
        </DndContext>
      )}
    </Spin>
  );

  return (
    <>
      <Collapse
        style={{ marginBottom: 16 }}
        defaultActiveKey={['omo-native-agents']}
        items={[
          {
            key: 'omo-native-agents',
            label: (
              <Space>
                <Text strong>
                  <ThunderboltOutlined style={{ marginRight: 8 }} />
                  {t('omoNative.title')}
                </Text>
                <Link
                  type="secondary"
                  style={{ fontSize: 12 }}
                  onClick={(event) => {
                    event.stopPropagation();
                    openExternalUrl(OMO_DOCS_URL);
                  }}
                >
                  <LinkOutlined /> {t('omoNative.docs')}
                </Link>
                {appliedConfig && (
                  <Tag color="processing">
                    {t('omoNative.current')}: {appliedConfig.name}
                  </Tag>
                )}
                {hasLocal && <Tag>{t('omoNative.localTag')}</Tag>}
              </Space>
            ),
            extra: (
              <Space>
                {appliedConfig && (
                  <Button
                    type="text"
                    size="small"
                    danger
                    style={{ fontSize: 12 }}
                    icon={<AppstoreOutlined />}
                    onClick={(event) => {
                      event.stopPropagation();
                      handleClearApplied();
                    }}
                  >
                    {t('omoNative.clearApplied')}
                  </Button>
                )}
                <Button
                  type="link"
                  size="small"
                  style={{ fontSize: 12 }}
                  icon={<PlusOutlined />}
                  onClick={(event) => {
                    event.stopPropagation();
                    handleAdd();
                  }}
                >
                  {t('omoNative.addConfig')}
                </Button>
              </Space>
            ),
            children: content,
          },
        ]}
      />

      <OmoNativeConfigModal
        open={modalOpen}
        isEdit={!!editingConfig && editingConfig.id !== '__local__'}
        isLocal={editingConfig?.id === '__local__'}
        initialValues={editingConfig}
        onCancel={() => {
          setModalOpen(false);
          setEditingConfig(null);
        }}
        onSuccess={handleModalSuccess}
      />
    </>
  );
};

export default OmoNativeSettings;
