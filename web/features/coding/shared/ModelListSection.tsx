import React from 'react';
import { Button, Collapse, Empty, Space, Tooltip, Typography } from 'antd';
import {
  CloudDownloadOutlined,
  DeleteOutlined,
  PlusOutlined,
  ApiOutlined,
} from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from '@dnd-kit/core';
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { restrictToVerticalAxis } from '@dnd-kit/modifiers';
import ModelItem from '@/components/common/ModelItem';
import type { ModelDisplayData } from '@/components/common/ProviderCard/types';
import styles from './ModelListSection.module.less';

const { Text } = Typography;

export interface ModelListSectionProps {
  /** i18n prefix for `model.*` labels. */

  /** Models rendered as rows. Callers map their own shape to `ModelDisplayData`. */
  models: ModelDisplayData[];

  /**
   * Row identity for selection / reorder / drag. Defaults to `model.id`.
   * Providers whose rows are keyed by more than the model id (Codex keys on
   * `model + displayName`) pass a resolver so callbacks receive their own key.
   */
  rowKeyOf?: (model: ModelDisplayData) => string;

  /** Collapse key; must be unique per provider when several lists coexist. */
  sectionKey: string;

  /** Extra class on the Collapse, e.g. to scope a transparent-background override. */
  className?: string;

  /** Style applied to the content wrapper inside the Collapse body. */
  bodyStyle?: React.CSSProperties;

  /** Rows render with a transparent fill so a tinted parent card shows through. */
  transparentRows?: boolean;

  /** Batch-delete selection mode. */
  selectionMode?: boolean;
  selectedIds?: string[];
  onToggleSelection?: (modelId: string, selected: boolean) => void;
  onToggleBatchDeleteMode?: () => void;
  onBatchDelete?: () => void;

  onTest?: () => void;
  testDisabled?: boolean;
  testDisabledTooltip?: string;
  onFetchModels?: () => void;
  fetchDisabled?: boolean;
  fetchDisabledTooltip?: string;
  onAddModel?: () => void;

  onEditModel?: (modelId: string) => void;
  onCopyModel?: (modelId: string) => void;
  onDeleteModel?: (modelId: string) => void;
  onSetPrimaryModel?: (modelId: string) => void;
  /**
   * Flips a model's enabled flag. Omit for CLIs that cannot disable a single
   * model — the row then shows no switch rather than a dead one.
   */
  onToggleModelDisabled?: (modelId: string, isDisabled: boolean) => void;
  /**
   * Per-row extra action (e.g. Codex "set as auto-review model").
   *
   * `rowKey` is the same key the row was rendered with, so callers can hand it
   * straight back to their own handlers. Do not try to re-derive it from
   * `model` — a provider whose display name falls back to the upstream id makes
   * that ambiguous, and the wrong row gets acted on.
   */
  renderModelExtraActions?: (model: ModelDisplayData, rowKey: string) => React.ReactNode;

  modelsDraggable?: boolean;
  onReorderModels?: (modelIds: string[]) => void;

  /** Rendered directly under the toolbar, above the rows (e.g. the auto-review line). */
  aboveList?: React.ReactNode;
}

/**
 * Shared model-list block for providers that own a model catalog (the Codex
 * shape). Locks the Collapse header with its batch-delete / test / fetch / add
 * actions and the row rendering through `ModelItem` (edit / copy / delete).
 *
 * Providers without a catalog (the Claude Code shape) simply do not render it.
 */
const ModelListSection: React.FC<ModelListSectionProps> = ({
  models,
  rowKeyOf = (model) => model.id,
  sectionKey,
  className,
  bodyStyle,
  transparentRows = false,
  selectionMode = false,
  selectedIds = [],
  onToggleSelection,
  onToggleBatchDeleteMode,
  onBatchDelete,
  onTest,
  testDisabled = false,
  testDisabledTooltip,
  onFetchModels,
  fetchDisabled = false,
  fetchDisabledTooltip,
  onAddModel,
  onEditModel,
  onCopyModel,
  onDeleteModel,
  onSetPrimaryModel,
  onToggleModelDisabled,
  renderModelExtraActions,
  modelsDraggable = false,
  onReorderModels,
  aboveList,
}) => {
  const { t } = useTranslation();

  const actionButtonStyle: React.CSSProperties = { fontSize: 12 };

  const sensors = useSensors(
    useSensor(PointerSensor),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) {
      return;
    }
    const oldIndex = models.findIndex((model) => rowKeyOf(model) === active.id);
    const newIndex = models.findIndex((model) => rowKeyOf(model) === over.id);
    if (oldIndex < 0 || newIndex < 0) {
      return;
    }
    onReorderModels?.(arrayMove(models, oldIndex, newIndex).map((model) => rowKeyOf(model)));
  };

  const canReorder = modelsDraggable && !selectionMode && Boolean(onReorderModels);

  const modelRows = models.map((model) => {
    const rowKey = rowKeyOf(model);
    return (
      <ModelItem
        key={rowKey}
        model={model}
        draggable={canReorder}
        sortableId={rowKey}
        transparentBackground={transparentRows}
        onEdit={onEditModel ? () => onEditModel(rowKey) : undefined}
        onCopy={onCopyModel ? () => onCopyModel(rowKey) : undefined}
        onDelete={onDeleteModel ? () => onDeleteModel(rowKey) : undefined}
        onSetPrimary={onSetPrimaryModel ? () => onSetPrimaryModel(rowKey) : undefined}
        isDisabled={model.isDisabled}
        onToggleDisabled={
          onToggleModelDisabled
            ? () => onToggleModelDisabled(rowKey, !model.isDisabled)
            : undefined
        }
        selectionMode={selectionMode}
        selected={selectedIds.includes(rowKey)}
        onSelectChange={
          onToggleSelection ? (selected) => onToggleSelection(rowKey, selected) : undefined
        }
        extraActions={renderModelExtraActions?.(model, rowKey)}
      />
    );
  });

  return (
    <Collapse
      ghost
      className={[className, transparentRows ? styles.transparentCollapse : undefined]
        .filter(Boolean)
        .join(' ')}
      defaultActiveKey={[]}
      style={{ marginTop: 12, background: 'transparent' }}
      items={[
        {
          key: sectionKey,
          label: (
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                width: '100%',
                background: 'transparent',
              }}
            >
              <Text strong style={{ fontSize: 13 }}>
                {t(`common.model.title`)} ({models.length})
              </Text>
              <Space size={0} onClick={(event) => event.stopPropagation()}>
                {onToggleBatchDeleteMode && (
                  <Button
                    size="small"
                    type="text"
                    icon={<DeleteOutlined />}
                    style={actionButtonStyle}
                    onClick={onToggleBatchDeleteMode}
                  >
                    {selectionMode
                      ? t(`common.model.cancelBatchDelete`)
                      : t(`common.model.batchDelete`)}
                  </Button>
                )}
                {selectionMode && onBatchDelete && (
                  <Button
                    size="small"
                    type="text"
                    danger
                    style={actionButtonStyle}
                    disabled={selectedIds.length === 0}
                    onClick={onBatchDelete}
                  >
                    {t(`common.model.deleteSelected`, { count: selectedIds.length })}
                  </Button>
                )}
                {onTest && (
                  <Tooltip title={testDisabled ? testDisabledTooltip : ''}>
                    <span>
                      <Button
                        size="small"
                        type="text"
                        style={actionButtonStyle}
                        onClick={onTest}
                        disabled={testDisabled}
                      >
                        <ApiOutlined style={{ marginRight: 4 }} />
                        {t('opencode.connectivity.button')}
                      </Button>
                    </span>
                  </Tooltip>
                )}
                {onFetchModels && (
                  <Tooltip title={fetchDisabled ? fetchDisabledTooltip : ''}>
                    <span>
                      <Button
                        size="small"
                        type="text"
                        style={actionButtonStyle}
                        onClick={onFetchModels}
                        disabled={fetchDisabled}
                      >
                        <CloudDownloadOutlined style={{ marginRight: 4 }} />
                        {t(`common.model.fetchModels`)}
                      </Button>
                    </span>
                  </Tooltip>
                )}
                {onAddModel && (
                  <Button
                    size="small"
                    type="text"
                    style={actionButtonStyle}
                    onClick={onAddModel}
                  >
                    <PlusOutlined style={{ marginRight: 4 }} />
                    {t(`common.model.addModel`)}
                  </Button>
                )}
              </Space>
            </div>
          ),
          children: (
            <div style={bodyStyle}>
              {aboveList}
              {models.length === 0 ? (
                <Empty
                  image={Empty.PRESENTED_IMAGE_SIMPLE}
                  description={t(`common.model.emptyText`)}
                  style={{ margin: '8px 0' }}
                />
              ) : canReorder ? (
                <DndContext
                  sensors={sensors}
                  collisionDetection={closestCenter}
                  modifiers={[restrictToVerticalAxis]}
                  onDragEnd={handleDragEnd}
                >
                  <SortableContext
                    items={models.map((model) => rowKeyOf(model))}
                    strategy={verticalListSortingStrategy}
                  >
                    <Space orientation="vertical" style={{ width: '100%' }} size={4}>
                      {modelRows}
                    </Space>
                  </SortableContext>
                </DndContext>
              ) : (
                <Space orientation="vertical" style={{ width: '100%' }} size={4}>
                  {modelRows}
                </Space>
              )}
            </div>
          ),
        },
      ]}
    />
  );
};

export default ModelListSection;
