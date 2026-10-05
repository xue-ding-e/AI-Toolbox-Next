import React from 'react';
import { Button, Card, Space, Switch, Tag, Tooltip, Typography } from 'antd';
import {
  DeleteOutlined,
  EditOutlined,
  HolderOutlined,
  ThunderboltOutlined,
} from '@ant-design/icons';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { useTranslation } from 'react-i18next';
import type { OmoNativeAgentsConfig } from '@/types/omoNative';

const { Text } = Typography;

interface OmoNativeConfigCardProps {
  config: OmoNativeAgentsConfig;
  onEdit: (config: OmoNativeAgentsConfig) => void;
  onDelete: (config: OmoNativeAgentsConfig) => void;
  onApply: (config: OmoNativeAgentsConfig) => void;
  onToggleDisabled: (config: OmoNativeAgentsConfig, isDisabled: boolean) => void;
  /** `__local__` 桥接态不可当已应用方案管理，只保留本地来源提示。 */
  isLocal?: boolean;
}

/** 摘要：生效的模型档 + 已配置的 agent/category 数量。 */
const buildSummary = (config: OmoNativeAgentsConfig): string => {
  const parts: string[] = [];
  const agentCount = config.agents ? Object.keys(config.agents).length : 0;
  const categoryCount = config.categories ? Object.keys(config.categories).length : 0;
  if (agentCount > 0) parts.push(`${agentCount} agents`);
  if (categoryCount > 0) parts.push(`${categoryCount} categories`);
  if (config.modelProfile) parts.push(config.modelProfile);
  return parts.join(' · ');
};

const OmoNativeConfigCard: React.FC<OmoNativeConfigCardProps> = ({
  config,
  onEdit,
  onDelete,
  onApply,
  onToggleDisabled,
  isLocal = false,
}) => {
  const { t } = useTranslation();
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: config.id, disabled: isLocal });

  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
    marginBottom: 12,
  };

  const summary = buildSummary(config);

  return (
    <div ref={setNodeRef} style={style}>
      <Card
        size="small"
        style={{
          borderColor: config.isApplied ? 'var(--ant-color-primary)' : undefined,
          opacity: config.isDisabled ? 0.6 : 1,
        }}
        styles={{ body: { padding: 12 } }}
      >
        <Space style={{ width: '100%', justifyContent: 'space-between' }} align="start">
          <Space align="start">
            {!isLocal && (
              <Button
                type="text"
                size="small"
                icon={<HolderOutlined />}
                style={{ cursor: 'grab' }}
                {...attributes}
                {...listeners}
              />
            )}
            <div>
              <Space size={4} wrap>
                <Text strong>{config.name}</Text>
                {config.isApplied && <Tag color="processing">{t('omoNative.applied')}</Tag>}
                {config.isDisabled && <Tag>{t('omoNative.disabled')}</Tag>}
                {isLocal && <Tag color="default">{t('omoNative.localTag')}</Tag>}
              </Space>
              {summary && (
                <div>
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    {summary}
                  </Text>
                </div>
              )}
            </div>
          </Space>

          <Space>
            {!isLocal && (
              <Tooltip title={t('omoNative.toggleDisabledHint')}>
                <Switch
                  size="small"
                  checked={!config.isDisabled}
                  onChange={(checked) => onToggleDisabled(config, !checked)}
                />
              </Tooltip>
            )}
            <Button
              size="small"
              icon={<EditOutlined />}
              onClick={() => onEdit(config)}
            >
              {t('common.edit')}
            </Button>
            {!isLocal && (
              <>
                <Button
                  size="small"
                  type={config.isApplied ? 'default' : 'primary'}
                  icon={<ThunderboltOutlined />}
                  disabled={config.isDisabled}
                  onClick={() => onApply(config)}
                >
                  {config.isApplied ? t('omoNative.reapply') : t('omoNative.apply')}
                </Button>
                <Button
                  size="small"
                  danger
                  icon={<DeleteOutlined />}
                  onClick={() => onDelete(config)}
                />
              </>
            )}
          </Space>
        </Space>
      </Card>
    </div>
  );
};

export default OmoNativeConfigCard;
