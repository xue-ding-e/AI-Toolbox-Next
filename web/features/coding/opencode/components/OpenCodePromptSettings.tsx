import React from 'react';
import { Button, Collapse, Empty, Modal, Space, Spin, Typography, message } from 'antd';
import { FileTextOutlined, PlusOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import {
  DndContext,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from '@dnd-kit/core';
import {
  SortableContext,
  arrayMove,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { restrictToVerticalAxis } from '@dnd-kit/modifiers';
import type {
  OpenCodePromptConfig,
  OpenCodePromptConfigInput,
} from '@/types/openCodePrompt';
import {
  applyOpenCodePromptConfig,
  createOpenCodePromptConfig,
  deleteOpenCodePromptConfig,
  listOpenCodePromptConfigs,
  reorderOpenCodePromptConfigs,
  saveOpenCodeLocalPromptConfig,
  updateOpenCodePromptConfig,
} from '@/services/openCodePromptApi';
import { refreshTrayMenu } from '@/services/appApi';
import OpenCodePromptConfigCard from './OpenCodePromptConfigCard';
import OpenCodePromptConfigModal, { type OpenCodePromptConfigFormValues } from './OpenCodePromptConfigModal';
import styles from './OpenCodePromptSettings.module.less';

const { Text } = Typography;

interface OpenCodePromptSettingsProps {
  refreshKey?: number;
  onUpdated?: () => void;
}

const OpenCodePromptSettings: React.FC<OpenCodePromptSettingsProps> = ({
  refreshKey = 0,
  onUpdated,
}) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [configs, setConfigs] = React.useState<OpenCodePromptConfig[]>([]);
  const [configModalOpen, setConfigModalOpen] = React.useState(false);
  const [editingConfig, setEditingConfig] = React.useState<OpenCodePromptConfig | null>(null);

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8,
      },
    })
  );

  const loadConfigs = React.useCallback(async () => {
    setLoading(true);
    try {
      const configList = await listOpenCodePromptConfigs();
      setConfigs(configList);
    } catch (error) {
      console.error('Failed to load OpenCode prompt configs:', error);
      message.error(t('common.error'));
    } finally {
      setLoading(false);
    }
  }, [t]);

  React.useEffect(() => {
    loadConfigs();
  }, [loadConfigs, refreshKey]);

  const notifyUpdated = async () => {
    await refreshTrayMenu();
    onUpdated?.();
  };

  const handleAddConfig = () => {
    setEditingConfig(null);
    setConfigModalOpen(true);
  };

  const handleEditConfig = (config: OpenCodePromptConfig) => {
    setEditingConfig({ ...config });
    setConfigModalOpen(true);
  };

  const handleDeleteConfig = (config: OpenCodePromptConfig) => {
    Modal.confirm({
      title: t('common.confirm'),
      content: t('opencode.prompt.confirmDelete', { name: config.name }),
      onOk: async () => {
        try {
          await deleteOpenCodePromptConfig(config.id);
          message.success(t('common.success'));
          await loadConfigs();
          await notifyUpdated();
        } catch (error) {
          console.error('Failed to delete OpenCode prompt config:', error);
          message.error(t('common.error'));
        }
      },
    });
  };

  const handleApplyConfig = async (config: OpenCodePromptConfig) => {
    try {
      await applyOpenCodePromptConfig(config.id);
      message.success(t('opencode.prompt.applySuccess'));
      await loadConfigs();
      await notifyUpdated();
    } catch (error) {
      console.error('Failed to apply OpenCode prompt config:', error);
      message.error(t('common.error'));
    }
  };

  const handleConfigSuccess = async (values: OpenCodePromptConfigFormValues) => {
    const payload: OpenCodePromptConfigInput = {
      id: editingConfig?.id !== '__local__' ? editingConfig?.id : undefined,
      name: values.name,
      content: values.content,
    };

    try {
      if (editingConfig?.id === '__local__') {
        await saveOpenCodeLocalPromptConfig(payload);
      } else if (editingConfig?.id) {
        await updateOpenCodePromptConfig(payload);
      } else {
        await createOpenCodePromptConfig(payload);
      }

      message.success(t('common.success'));
      setConfigModalOpen(false);
      setEditingConfig(null);
      await loadConfigs();
      await notifyUpdated();
    } catch (error) {
      console.error('Failed to save OpenCode prompt config:', error);
      message.error(t('common.error'));
    }
  };

  const handleDragEnd = async (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) {
      return;
    }

    if (configs.some((config) => config.id === '__local__')) {
      return;
    }

    const oldIndex = configs.findIndex((config) => config.id === active.id);
    const newIndex = configs.findIndex((config) => config.id === over.id);

    if (oldIndex === -1 || newIndex === -1) {
      return;
    }

    const oldConfigs = [...configs];
    const newConfigs = arrayMove(configs, oldIndex, newIndex);
    setConfigs(newConfigs);

    try {
      await reorderOpenCodePromptConfigs(newConfigs.map((config) => config.id));
      await notifyUpdated();
    } catch (error) {
      console.error('Failed to reorder OpenCode prompt configs:', error);
      setConfigs(oldConfigs);
      message.error(t('common.error'));
    }
  };

  const content = (
    <Spin spinning={loading}>
      <div className={styles.hintBlock}>
        <div>{t('opencode.prompt.sectionHint')}</div>
        <div>{t('opencode.prompt.sectionWarning')}</div>
      </div>

      {configs.length === 0 ? (
        <Empty description={t('opencode.prompt.emptyText')} style={{ margin: '24px 0' }} />
      ) : (
        <DndContext
          sensors={sensors}
          collisionDetection={closestCenter}
          modifiers={[restrictToVerticalAxis]}
          onDragEnd={handleDragEnd}
        >
          <SortableContext items={configs.map((config) => config.id)} strategy={verticalListSortingStrategy}>
            <div>
              {configs.map((config) => (
                <OpenCodePromptConfigCard
                  key={config.id}
                  config={config}
                  onEdit={handleEditConfig}
                  onDelete={handleDeleteConfig}
                  onApply={handleApplyConfig}
                />
              ))}
            </div>
          </SortableContext>
        </DndContext>
      )}
    </Spin>
  );

  const appliedConfig = configs.find((config) => config.isApplied);

  return (
    <>
      <Collapse
        style={{ marginBottom: 16 }}
        defaultActiveKey={['opencode-prompt']}
        items={[
          {
            key: 'opencode-prompt',
            label: (
              <Space>
                <Text strong>
                  <FileTextOutlined style={{ marginRight: 8 }} />
                  {t('opencode.prompt.title')}
                </Text>
                {appliedConfig && (
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    {t('opencode.prompt.current')}: {appliedConfig.name}
                  </Text>
                )}
              </Space>
            ),
            extra: (
              <Button
                type="link"
                size="small"
                style={{ fontSize: 12 }}
                icon={<PlusOutlined />}
                onClick={(e) => {
                  e.stopPropagation();
                  handleAddConfig();
                }}
              >
                {t('opencode.prompt.addConfig')}
              </Button>
            ),
            children: content,
          },
        ]}
      />

      <OpenCodePromptConfigModal
        open={configModalOpen}
        initialValues={editingConfig || undefined}
        onCancel={() => {
          setConfigModalOpen(false);
          setEditingConfig(null);
        }}
        onSuccess={handleConfigSuccess}
      />
    </>
  );
};

export default OpenCodePromptSettings;
