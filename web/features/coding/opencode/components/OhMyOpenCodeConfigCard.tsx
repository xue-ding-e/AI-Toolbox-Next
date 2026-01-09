import React from 'react';
import { Card, Typography, Space, Button, Tag, Tooltip } from 'antd';
import { EditOutlined, CopyOutlined, DeleteOutlined, CheckCircleOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { OhMyOpenCodeConfig, OhMyOpenCodeAgentType } from '@/types/ohMyOpenCode';
import { getAgentDisplayName } from '@/services/ohMyOpenCodeApi';

const { Text, Paragraph } = Typography;

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
  const totalAgents = Object.keys(config.agents).length;

  return (
    <Card
      size="small"
      style={{
        marginBottom: 8,
        borderColor: isSelected ? '#1890ff' : undefined,
        backgroundColor: isSelected ? '#e6f7ff' : undefined,
      }}
      extra={
        <Space>
          {isSelected ? (
            <Tag color="blue" icon={<CheckCircleOutlined />}>
              {t('opencode.ohMyOpenCode.applied')}
            </Tag>
          ) : (
            <Button
              type="link"
              size="small"
              onClick={() => onApply(config)}
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
      }
    >
      <div>
        <Text strong style={{ fontSize: 14 }}>{config.name}</Text>
      </div>
      
      <div style={{ marginTop: 8 }}>
        <Space wrap size={4}>
          <Tag color="blue">
            {configuredCount}/{totalAgents} {t('opencode.ohMyOpenCode.agentsConfigured')}
          </Tag>
        </Space>
      </div>

      <Paragraph
        type="secondary"
        style={{ fontSize: 12, marginTop: 8, marginBottom: 0 }}
        ellipsis={{ rows: 2 }}
      >
        {getAgentsSummary() || t('opencode.ohMyOpenCode.noAgentsConfigured')}
      </Paragraph>
    </Card>
  );
};

export default OhMyOpenCodeConfigCard;
