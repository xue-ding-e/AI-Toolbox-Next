import React from 'react';
import { Button, Card, Dropdown, Space, Tag, Typography, theme } from 'antd';
import {
  CheckCircleOutlined,
  DeleteOutlined,
  DownOutlined,
  EditOutlined,
  HolderOutlined,
  MoreOutlined,
  UpOutlined,
} from '@ant-design/icons';
import type { MenuProps } from 'antd';
import { useTranslation } from 'react-i18next';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import MarkdownPreview from '@/components/common/MarkdownPreview';
import type { GlobalPromptConfig } from '@/types/globalPrompt';
import styles from './GlobalPromptSettings.module.less';

const { Text } = Typography;

interface GlobalPromptConfigCardProps {
  config: GlobalPromptConfig;
  translationKeyPrefix: string;
  onEdit: (config: GlobalPromptConfig) => void;
  onDelete: (config: GlobalPromptConfig) => void;
  onApply: (config: GlobalPromptConfig) => void;
}

const GlobalPromptConfigCard: React.FC<GlobalPromptConfigCardProps> = ({
  config,
  translationKeyPrefix,
  onEdit,
  onDelete,
  onApply,
}) => {
  const { t } = useTranslation();
  const { token } = theme.useToken();
  const [expanded, setExpanded] = React.useState(false);
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: config.id, disabled: config.id === '__local__' });

  const sortableStyle: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
  };

  const menuItems: MenuProps['items'] = [
    {
      key: 'edit',
      label: t('common.edit'),
      icon: <EditOutlined />,
      onClick: () => onEdit(config),
    },
    ...(config.id !== '__local__'
      ? [{
          type: 'divider' as const,
        }, {
          key: 'delete',
          label: t('common.delete'),
          icon: <DeleteOutlined />,
          danger: true,
          onClick: () => onDelete(config),
        }]
      : []),
  ];

  return (
    <div ref={setNodeRef} style={sortableStyle}>
      <Card
        size="small"
        className={styles.card}
        style={{
          marginBottom: 8,
          borderColor: config.isApplied ? token.colorPrimary : 'var(--color-border-secondary)',
          backgroundColor: config.isApplied ? 'var(--color-bg-selected)' : 'var(--color-bg-container)',
          opacity: config.id === '__local__' ? 0.95 : 1,
          transition: 'opacity 0.3s ease, border-color 0.2s ease',
        }}
        styles={{ body: { padding: '8px 12px' } }}
      >
        <div style={{ display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          <div
            className={styles.dragHandle}
            {...(config.id !== '__local__' ? attributes : {})}
            {...(config.id !== '__local__' ? listeners : {})}
            style={{
              cursor: config.id === '__local__' ? 'default' : (isDragging ? 'grabbing' : 'grab'),
              touchAction: 'none',
              padding: '4px 0',
            }}
          >
            <HolderOutlined />
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div className={styles.cardHeader}>
              <div className={styles.cardTitleRow}>
                <Text strong className={styles.cardName}>{config.name}</Text>
                {config.id === '__local__' && (
                  <Text type="secondary" className={styles.cardHint}>
                    ({t(`${translationKeyPrefix}.localConfigHint`)})
                  </Text>
                )}
                {config.isApplied && (
                  <Tag color="green" icon={<CheckCircleOutlined />} style={{ margin: 0 }}>
                    {t(`${translationKeyPrefix}.applied`)}
                  </Tag>
                )}
              </div>
              <Space size={4}>
                {!config.isApplied && (
                  <Button type="link" size="small" onClick={() => onApply(config)}>
                    {t(`${translationKeyPrefix}.apply`)}
                  </Button>
                )}
                <Dropdown menu={{ items: menuItems }} trigger={['click']}>
                  <Button type="text" size="small" icon={<MoreOutlined />} />
                </Dropdown>
              </Space>
            </div>
            <div className={styles.cardContent}>
              {expanded ? (
                <>
                  <MarkdownPreview
                    content={config.content}
                    className={styles.cardPreview}
                  />
                  <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
                    <Button
                      type="text"
                      size="small"
                      className={styles.expandToggle}
                      icon={<UpOutlined />}
                      aria-label={t(`${translationKeyPrefix}.collapse`)}
                      title={t(`${translationKeyPrefix}.collapse`)}
                      onClick={() => setExpanded(false)}
                    />
                  </div>
                </>
              ) : (
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 0 }}>
                  <Text
                    ellipsis
                    style={{ flex: 1, minWidth: 0, color: 'inherit' }}
                  >
                    {config.content}
                  </Text>
                  <Button
                    type="text"
                    size="small"
                    className={styles.expandToggle}
                    icon={<DownOutlined />}
                    aria-label={t(`${translationKeyPrefix}.expand`)}
                    title={t(`${translationKeyPrefix}.expand`)}
                    onClick={() => setExpanded(true)}
                  />
                </div>
              )}
            </div>
          </div>
        </div>
      </Card>
    </div>
  );
};

export default GlobalPromptConfigCard;
