import React from 'react';
import { Card, Typography, Space, Button, Tag, Switch, Dropdown, message } from 'antd';
import { EditOutlined, CopyOutlined, DeleteOutlined, CheckCircleOutlined, MoreOutlined, HolderOutlined } from '@ant-design/icons';
import type { MenuProps } from 'antd';
import { useTranslation } from 'react-i18next';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { SLIM_AGENT_TYPES, SLIM_AGENT_DISPLAY_NAMES, type OhMyOpenCodeSlimConfig } from '@/types/ohMyOpenCodeSlim';

const { Text } = Typography;

// Standard agent types count
const STANDARD_AGENT_COUNT = SLIM_AGENT_TYPES.length;

interface OhMyOpenCodeSlimConfigCardProps {
  config: OhMyOpenCodeSlimConfig;
  isSelected?: boolean;
  disabled?: boolean;
  onEdit: (config: OhMyOpenCodeSlimConfig) => void;
  onCopy: (config: OhMyOpenCodeSlimConfig) => void;
  onDelete: (config: OhMyOpenCodeSlimConfig) => void;
  onApply: (config: OhMyOpenCodeSlimConfig) => void;
  onToggleDisabled: (config: OhMyOpenCodeSlimConfig, isDisabled: boolean) => void;
}

const OhMyOpenCodeSlimConfigCard: React.FC<OhMyOpenCodeSlimConfigCardProps> = ({
  config,
  isSelected = false,
  disabled = false,
  onEdit,
  onCopy,
  onDelete,
  onApply,
  onToggleDisabled,
}) => {
  const { t } = useTranslation();

  // 拖拽排序
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
    opacity: isDragging ? 0.5 : (config.isDisabled ? 0.6 : 1),
  };

  const handleToggleDisabled = (checked: boolean) => {
    if (isSelected && !checked) {
      message.warning(t('common.disableAppliedConfigWarning'));
      return;
    }
    onToggleDisabled(config, !checked);
  };

  // Get configured agents as structured data (sorted)
  const getAgentsData = (): { name: string; model: string }[] => {
    const result: { name: string; model: string }[] = [];

    // Handle null agents
    if (!config.agents) {
      return result;
    }

    // Iterate in the predefined order
    SLIM_AGENT_TYPES.forEach((agentType) => {
      const agent = config.agents?.[agentType];
      if (agent && typeof agent.model === 'string' && agent.model) {
        const displayName = SLIM_AGENT_DISPLAY_NAMES[agentType].split(' ')[0]; // Get short name
        result.push({ name: displayName, model: agent.model });
      }
    });

    return result;
  };

  const agentsData = getAgentsData();

  // Get configured count
  const configuredCount = config.agents
    ? Object.values(config.agents).filter((a) => !!a && typeof a.model === 'string' && !!a.model).length
    : 0;
  const totalAgents = STANDARD_AGENT_COUNT;

  const menuItems: MenuProps['items'] = [
    {
      key: 'toggle',
      label: (
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12 }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
            <span>{t('common.enable')}</span>
            <Text type="secondary" style={{ fontSize: 11 }}>
              {config.isDisabled
                ? t('opencode.ohMyOpenCodeSlim.configDisabled')
                : t('opencode.ohMyOpenCodeSlim.configEnabled')}
            </Text>
          </div>
          <Switch
            checked={!config.isDisabled}
            onChange={handleToggleDisabled}
            size="small"
            disabled={disabled}
          />
        </div>
      ),
    },
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
    {
      type: 'divider',
    },
    {
      key: 'delete',
      label: t('common.delete'),
      icon: <DeleteOutlined />,
      danger: true,
      onClick: () => onDelete(config),
    },
  ].filter(Boolean) as MenuProps['items'];

  return (
    <div ref={setNodeRef} style={sortableStyle}>
      <Card
        size="small"
        style={{
          marginBottom: 8,
          borderColor: isSelected ? '#1890ff' : undefined,
          backgroundColor: isSelected ? '#e6f7ff' : undefined,
          transition: 'opacity 0.3s ease, border-color 0.2s ease',
        }}
        styles={{ body: { padding: '8px 12px' } }}
      >
        <div style={{ display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          {/* 拖拽手柄 */}
          <div
            {...attributes}
            {...listeners}
            style={{
              cursor: isDragging ? 'grabbing' : 'grab',
              color: '#999',
              touchAction: 'none',
              padding: '4px 0',
            }}
          >
            <HolderOutlined />
          </div>

          {/* 右侧内容区 */}
          <div style={{ flex: 1 }}>
            {/* 第一行：配置名称、标签和操作按钮 */}
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 16 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                <Text strong style={{ fontSize: 14, whiteSpace: 'nowrap' }}>{config.name}</Text>

                <Tag color="blue" style={{ margin: 0 }}>
                  {configuredCount}/{totalAgents} Agent
                </Tag>

                {isSelected && (
                  <Tag color="green" icon={<CheckCircleOutlined />} style={{ margin: 0 }}>
                    {t('opencode.ohMyOpenCode.applied')}
                  </Tag>
                )}
              </div>

              {/* 右侧：操作按钮 */}
              <Space size={4}>
                {!isSelected && (
                  <Button
                    type="link"
                    size="small"
                    onClick={() => onApply(config)}
                    style={{ padding: '0 8px' }}
                    disabled={disabled || config.isDisabled}
                  >
                    {t('opencode.ohMyOpenCode.apply')}
                  </Button>
                )}
                <Dropdown menu={{ items: menuItems }} trigger={['click']}>
                  <Button
                    type="text"
                    size="small"
                    icon={<MoreOutlined />}
                    disabled={disabled}
                  />
                </Dropdown>
              </Space>
            </div>

            {/* 第二行：Agent 详情（结构化展示） */}
            {agentsData.length > 0 && (
              <div style={{ marginTop: 6 }}>
                <div style={{
                  display: 'flex',
                  flexWrap: 'wrap',
                  gap: '4px 12px',
                  lineHeight: '1.6'
                }}>
                  {agentsData.map((item, index) => (
                    <span key={index} style={{ fontSize: 12, whiteSpace: 'nowrap' }}>
                      <Text strong style={{ color: '#1890ff', fontSize: 12 }}>{item.name}</Text>
                      <Text type="secondary" style={{ fontSize: 12 }}>: </Text>
                      <Text type="secondary" style={{ fontSize: 12 }}>{item.model}</Text>
                    </span>
                  ))}
                </div>
              </div>
            )}

            {agentsData.length === 0 && (
              <div style={{ marginTop: 4 }}>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  {t('opencode.ohMyOpenCode.noAgentsConfigured')}
                </Text>
              </div>
            )}
          </div>
        </div>
    </Card>
    </div>
  );
};

export default OhMyOpenCodeSlimConfigCard;
