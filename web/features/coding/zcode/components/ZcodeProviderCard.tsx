import React from 'react';
import { Button, Card, Dropdown, Space, Tag, Tooltip, Typography } from 'antd';
import {
  CheckOutlined,
  DeleteOutlined,
  EditOutlined,
  HolderOutlined,
  MoreOutlined,
} from '@ant-design/icons';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { useTranslation } from 'react-i18next';
import { ManagementCheckbox } from '@/features/coding/shared/management/ManagementControls';
import ProviderConnectivityStatus from '@/features/coding/shared/providerConnectivity/ProviderConnectivityStatus';
import ModelListSection from '@/features/coding/shared/ModelListSection';
import type {
  ModelDisplayData,
  ProviderConnectivityStatusItem,
} from '@/components/common/ProviderCard/types';
import type { ZcodeProvider } from '@/types/zcode';
import { parseZcodeProviderSettings } from '../utils/zcodeSettingsConfig';

const { Text } = Typography;

interface ZcodeProviderCardProps {
  provider: ZcodeProvider;
  onEdit: () => void;
  onApply: () => void;
  onDelete: () => void;
  /** Renders a checkbox instead of the drag handle while batch selection is on. */
  selectable?: boolean;
  selected?: boolean;
  onSelectChange?: (selected: boolean) => void;
  /** Latest result of the batch connectivity test for this provider. */
  connectivityStatus?: ProviderConnectivityStatusItem;

  /** Model catalog actions. Omit a handler to hide its button. */
  onAddModel?: () => void;
  onEditModel?: (modelId: string) => void;
  onCopyModel?: (modelId: string) => void;
  onDeleteModel?: (modelId: string) => void;
  onSetPrimaryModel?: (modelId: string) => void;
  onReorderModels?: (orderedModelIds: string[]) => void;
  /** Connectivity test for the whole catalog; hidden while it cannot run. */
  onTestModels?: () => void;
  testModelsDisabled?: boolean;
  testModelsDisabledTooltip?: string;
  onFetchModels?: () => void;
  modelSelectionMode?: boolean;
  selectedModelIds?: string[];
  onToggleModelSelection?: (modelId: string, selected: boolean) => void;
  onToggleBatchDeleteMode?: () => void;
  onBatchDeleteModels?: () => void;
}

const ZcodeProviderCard: React.FC<ZcodeProviderCardProps> = ({
  provider,
  onEdit,
  onApply,
  onDelete,
  selectable = false,
  selected = false,
  onSelectChange,
  connectivityStatus,
  onAddModel,
  onEditModel,
  onCopyModel,
  onDeleteModel,
  onSetPrimaryModel,
  onReorderModels,
  onTestModels,
  testModelsDisabled = false,
  testModelsDisabledTooltip,
  onFetchModels,
  modelSelectionMode = false,
  selectedModelIds = [],
  onToggleModelSelection,
  onToggleBatchDeleteMode,
  onBatchDeleteModels,
}) => {
  const { t } = useTranslation();
  const settings = parseZcodeProviderSettings(provider.settingsConfig);
  const apiType = settings?.config?.api?.type;
  const baseUrl = settings?.config?.api?.baseUrl;
  const models = settings?.models ?? [];
  const defaultModelId = models.find((model) => model.isDefault)?.modelId;

  /**
   * `ModelListSection` renders `ModelDisplayData`, so each ZCode row is mapped
   * here. The row key is the `modelId` — ZCode keys models by it, and the
   * default flag travels separately in `isPrimary`.
   */
  const modelDisplayRows = React.useMemo<ModelDisplayData[]>(
    () =>
      models.map((model) => ({
        id: model.modelId,
        name: model.displayName?.trim() || model.modelId,
        contextLimit: model.properties?.contextWindow,
        outputLimit: model.optionSpecs?.maxOutputTokens?.max,
        isPrimary: model.modelId === defaultModelId,
      })),
    [models, defaultModelId],
  );

  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: provider.id,
  });

  const sortableStyle: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : provider.isDisabled ? 0.6 : 1,
  };

  const menuItems = [
    {
      key: 'edit',
      icon: <EditOutlined />,
      label: t('common.edit', { defaultValue: '编辑' }),
    },
    {
      key: 'delete',
      icon: <DeleteOutlined />,
      danger: true,
      label: t('common.delete', { defaultValue: '删除' }),
    },
  ];

  return (
    <div ref={setNodeRef} style={sortableStyle}>
      <Card size="small" styles={{ body: { padding: 12 } }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
          <div style={{ minWidth: 0, flex: 1, display: 'flex', alignItems: 'flex-start', gap: 8 }}>
            {selectable ? (
              <div style={{ display: 'flex', alignItems: 'center', padding: '4px 0' }}>
                <ManagementCheckbox
                  checked={selected}
                  ariaLabel={t('common.batch.selectItem')}
                  onChange={onSelectChange ?? (() => {})}
                />
              </div>
            ) : (
              <div
                {...attributes}
                {...listeners}
                style={{
                  cursor: isDragging ? 'grabbing' : 'grab',
                  color: '#999',
                  padding: '4px 0',
                  touchAction: 'none',
                }}
              >
                <HolderOutlined />
              </div>
            )}
            <div style={{ minWidth: 0, flex: 1 }}>
              <Space size="small" wrap>
                <ProviderConnectivityStatus item={connectivityStatus} />
                <Text strong>{provider.name}</Text>
                {provider.isApplied && (
                  <Tag color="green">{t('zcode.provider.applied', { defaultValue: '默认' })}</Tag>
                )}
                {provider.isDisabled && (
                  <Tag>{t('zcode.provider.disabled', { defaultValue: '已禁用' })}</Tag>
                )}
              </Space>
              {/* One detail line, as on the Codex card: base URL, format and
                  notes inline. The provider id is not shown — it duplicates the
                  name for auto-derived ids and means nothing to the user. */}
              <div style={{ marginTop: 4, display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
                {baseUrl && (
                  <Text code style={{ fontSize: 12 }}>
                    {baseUrl}
                  </Text>
                )}
                {apiType && <Tag color="blue" style={{ fontSize: 11, margin: 0 }}>{apiType}</Tag>}
                {baseUrl && provider.notes && (
                  <Text type="secondary" style={{ fontSize: 12 }}>|</Text>
                )}
                {provider.notes && (
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    {provider.notes}
                  </Text>
                )}
              </div>
            </div>
          </div>
          <Space size="small">
            <Tooltip
              title={t('zcode.provider.applyHint', {
                defaultValue: '设为 ZCode 新建会话的默认供应商与模型',
              })}
            >
              {/* Link-style action, matching the apply button on every other
                  provider card. */}
              <Button
                type="link"
                size="small"
                icon={<CheckOutlined />}
                onClick={onApply}
                disabled={provider.isDisabled}
              >
                {t('zcode.provider.apply', { defaultValue: '应用' })}
              </Button>
            </Tooltip>
            <Dropdown
              menu={{
                items: menuItems,
                onClick: ({ key }) => {
                  if (key === 'edit') {
                    onEdit();
                  } else if (key === 'delete') {
                    onDelete();
                  }
                },
              }}
            >
              <Button size="small" type="text" icon={<MoreOutlined />} />
            </Dropdown>
          </Space>
        </div>

        <ModelListSection
          models={modelDisplayRows}
          rowKeyOf={(model) => model.id}
          sectionKey={`zcode-models-${provider.id}`}
          transparentRows
          modelsDraggable={!modelSelectionMode && Boolean(onReorderModels)}
          onReorderModels={onReorderModels}
          selectionMode={modelSelectionMode}
          selectedIds={selectedModelIds}
          onToggleSelection={onToggleModelSelection}
          onToggleBatchDeleteMode={onToggleBatchDeleteMode}
          onBatchDelete={onBatchDeleteModels}
          onTest={onTestModels}
          testDisabled={testModelsDisabled}
          testDisabledTooltip={testModelsDisabledTooltip}
          onFetchModels={onFetchModels}
          fetchDisabled={!baseUrl}
          fetchDisabledTooltip={t('opencode.provider.completeUrlAndKey')}
          onAddModel={onAddModel}
          onEditModel={onEditModel}
          onCopyModel={onCopyModel}
          onDeleteModel={onDeleteModel}
          onSetPrimaryModel={onSetPrimaryModel}
        />
      </Card>
    </div>
  );
};

export default ZcodeProviderCard;
