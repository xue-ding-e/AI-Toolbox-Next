import React from 'react';
import { Card, Typography, Space, Button, Tag, Tooltip } from 'antd';
import { EditOutlined, CopyOutlined, DeleteOutlined, CheckCircleOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { OhMyOpenCodeConfig, OhMyOpenCodeAgentType } from '@/types/ohMyOpenCode';
import { getAgentDisplayName } from '@/services/ohMyOpenCodeApi';

const { Text, Paragraph } = Typography;

// Standard agent types count
const STANDARD_AGENT_COUNT = 7; // Sisyphus, oracle, librarian, explore, frontend-ui-ux-engineer, document-writer, multimodal-looker

interface OhMyOpenCodeConfigCardProps {
  config: OhMyOpenCodeConfig;
  isSelected?: boolean;
  onEdit: (config: OhMyOpenCodeConfig) => void;
  onCopy: (config: OhMyOpenCodeConfig) => void;
  onDelete: (config: OhMyOpenCodeConfig) => void;
  onApply: (config: OhMyOpenCodeConfig) => void;
}

const OhMyOpenCodeConfigCard: React.FC<OhMyOpenCodeConfigCardProps> = ({
  config,
  isSelected = false,
  onEdit,
  onCopy,
  onDelete,
  onApply,
}) => {
  const { t } = useTranslation();

  // Get configured agents summary
  const getAgentsSummary = (): string => {
    const summaries: string[] = [];
    const agentTypes = Object.keys(config.agents) as OhMyOpenCodeAgentType[];
    
    agentTypes.forEach((agentType) => {
      const agent = config.agents[agentType];
      if (agent && agent.model) {
        const displayName = getAgentDisplayName(agentType).split(' ')[0]; // Get short name
        summaries.push(`${displayName}: ${agent.model}`);
      }
    });

    return summaries.join(' | ');
  };

  // Get configured count
  const configuredCount = Object.values(config.agents).filter((a) => a && a.model).length;
  const totalAgents = STANDARD_AGENT_COUNT; // Use standard agent count instead of actual keys

  return (
    <Card
      size="small"
      style={{
        marginBottom: 8,
        borderColor: isSelected ? '#1890ff' : undefined,
        backgroundColor: isSelected ? '#e6f7ff' : undefined,
      }}
      bodyStyle={{ padding: '8px 12px' }}
    >
      {/* 第一行：配置名称、标签和操作按钮 */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 16 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <Text strong style={{ fontSize: 14, whiteSpace: 'nowrap' }}>{config.name}</Text>
          
          <Tag color="blue" style={{ margin: 0 }}>
            {configuredCount}/{totalAgents} Agent
          </Tag>
          
          {isSelected && (
            <Tag color="blue" icon={<CheckCircleOutlined />} style={{ margin: 0 }}>
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
            >
              {t('opencode.ohMyOpenCode.apply')}
            </Button>
          )}
          <Tooltip title={t('common.edit')}>
            <Button
              type="text"
              size="small"
              icon={<EditOutlined />}
              onClick={() => onEdit(config)}
            />
          </Tooltip>
          <Tooltip title={t('common.copy')}>
            <Button
              type="text"
              size="small"
              icon={<CopyOutlined />}
              onClick={() => onCopy(config)}
            />
          </Tooltip>
          <Tooltip title={t('common.delete')}>
            <Button
              type="text"
              size="small"
              danger
              icon={<DeleteOutlined />}
              onClick={() => onDelete(config)}
            />
          </Tooltip>
        </Space>
      </div>

      {/* 第二行：Agent 详情（支持换行） */}
      {getAgentsSummary() && (
        <div style={{ marginTop: 4 }}>
          <Text 
            type="secondary" 
            style={{ 
              fontSize: 12, 
              wordBreak: 'break-word',
              lineHeight: '1.5'
            }}
          >
            {getAgentsSummary()}
          </Text>
        </div>
      )}
      
      {!getAgentsSummary() && (
        <div style={{ marginTop: 4 }}>
          <Text type="secondary" style={{ fontSize: 12 }}>
            {t('opencode.ohMyOpenCode.noAgentsConfigured')}
          </Text>
        </div>
      )}
    </Card>
  );
};

export default OhMyOpenCodeConfigCard;
