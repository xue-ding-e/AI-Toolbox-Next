import React from 'react';
import { Card, Space, Button, Dropdown, Tag, Typography } from 'antd';
import {
  EditOutlined,
  DeleteOutlined,
  CopyOutlined,
  MoreOutlined,
  DownOutlined,
  UpOutlined,
  CheckCircleOutlined,
} from '@ant-design/icons';
import type { MenuProps } from 'antd';
import { useTranslation } from 'react-i18next';
import type { ClaudeCodeProvider } from '@/types/claudecode';

const { Text } = Typography;

interface ClaudeProviderCardProps {
  provider: ClaudeCodeProvider;
  isCurrent: boolean;
  isApplied: boolean;
  onEdit: (provider: ClaudeCodeProvider) => void;
  onDelete: (provider: ClaudeCodeProvider) => void;
  onCopy: (provider: ClaudeCodeProvider) => void;
  onSelect: (provider: ClaudeCodeProvider) => void;
}

const ClaudeProviderCard: React.FC<ClaudeProviderCardProps> = ({
  provider,
  isCurrent,
  isApplied,
  onEdit,
  onDelete,
  onCopy,
  onSelect,
}) => {
  const { t } = useTranslation();
  const [modelsExpanded, setModelsExpanded] = React.useState(false);

  const menuItems: MenuProps['items'] = [
    {
      key: 'edit',
      label: t('claudecode.provider.editProvider'),
      icon: <EditOutlined />,
      onClick: () => onEdit(provider),
    },
    {
      key: 'copy',
      label: t('claudecode.provider.copyProvider'),
      icon: <CopyOutlined />,
      onClick: () => onCopy(provider),
    },
    {
      type: 'divider',
    },
    {
      key: 'delete',
      label: t('claudecode.provider.deleteProvider'),
      icon: <DeleteOutlined />,
      danger: true,
      onClick: () => onDelete(provider),
    },
  ];

  const hasModels =
    provider.settingsConfig.haikuModel ||
    provider.settingsConfig.sonnetModel ||
    provider.settingsConfig.opusModel;

  return (
    <Card
      size="small"
      style={{
        marginBottom: 12,
        borderColor: isCurrent ? '#1890ff' : undefined,
        backgroundColor: isCurrent ? '#f0f5ff' : undefined,
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
        <div style={{ flex: 1 }}>
          <Space direction="vertical" size={4} style={{ width: '100%' }}>
            {/* 供应商名称和状态 */}
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Text strong style={{ fontSize: 14 }}>
                {provider.name}
              </Text>
              {isCurrent && (
                <Tag color="blue" icon={<CheckCircleOutlined />}>
                  {t('claudecode.provider.enabled')}
                </Tag>
              )}
              {isApplied && (
                <Tag color="success">{t('claudecode.provider.applied')}</Tag>
              )}
            </div>

            {/* Base URL */}
            {provider.settingsConfig.env?.ANTHROPIC_BASE_URL && (
              <Text type="secondary" style={{ fontSize: 12 }}>
                {provider.settingsConfig.env.ANTHROPIC_BASE_URL}
              </Text>
            )}

            {/* 默认模型 */}
            {provider.settingsConfig.model && (
              <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                <Text type="secondary" style={{ fontSize: 12 }}>
                  {t('claudecode.provider.defaultModel')}:
                </Text>
                <Text code style={{ fontSize: 12 }}>
                  {provider.settingsConfig.model}
                </Text>
              </div>
            )}

            {/* 备注 */}
            {provider.notes && (
              <Text type="secondary" style={{ fontSize: 12 }}>
                {provider.notes}
              </Text>
            )}

            {/* 展开的模型配置 */}
            {hasModels && modelsExpanded && (
              <div
                style={{
                  marginTop: 8,
                  padding: 8,
                  backgroundColor: '#fafafa',
                  borderRadius: 4,
                }}
              >
                <Space direction="vertical" size={4} style={{ width: '100%' }}>
                  {provider.settingsConfig.haikuModel && (
                    <div>
                      <Text type="secondary" style={{ fontSize: 12 }}>
                        Haiku:
                      </Text>{' '}
                      <Text code style={{ fontSize: 12 }}>
                        {provider.settingsConfig.haikuModel}
                      </Text>
                    </div>
                  )}
                  {provider.settingsConfig.sonnetModel && (
                    <div>
                      <Text type="secondary" style={{ fontSize: 12 }}>
                        Sonnet:
                      </Text>{' '}
                      <Text code style={{ fontSize: 12 }}>
                        {provider.settingsConfig.sonnetModel}
                      </Text>
                    </div>
                  )}
                  {provider.settingsConfig.opusModel && (
                    <div>
                      <Text type="secondary" style={{ fontSize: 12 }}>
                        Opus:
                      </Text>{' '}
                      <Text code style={{ fontSize: 12 }}>
                        {provider.settingsConfig.opusModel}
                      </Text>
                    </div>
                  )}
                </Space>
              </div>
            )}
          </Space>
        </div>

        {/* 操作按钮 */}
        <Space>
          {hasModels && (
            <Button
              type="text"
              size="small"
              icon={modelsExpanded ? <UpOutlined /> : <DownOutlined />}
              onClick={() => setModelsExpanded(!modelsExpanded)}
            >
              {modelsExpanded
                ? t('claudecode.provider.collapseModels')
                : t('claudecode.provider.expandModels')}
            </Button>
          )}
          {!isCurrent && (
            <Button type="primary" size="small" onClick={() => onSelect(provider)}>
              {t('claudecode.provider.enable')}
            </Button>
          )}
          <Dropdown menu={{ items: menuItems }} trigger={['click']}>
            <Button type="text" size="small" icon={<MoreOutlined />} />
          </Dropdown>
        </Space>
      </div>
    </Card>
  );
};

export default ClaudeProviderCard;
