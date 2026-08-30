import React from 'react';
import { Empty, Table, Tag, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import type { KimiPlugin } from '@/types/kimi';

const { Text } = Typography;

interface KimiPluginsPanelProps {
  plugins: KimiPlugin[];
  loading?: boolean;
}

export const KimiPluginsPanel: React.FC<KimiPluginsPanelProps> = ({
  plugins,
  loading = false,
}) => {
  const { t } = useTranslation();

  const columns = [
    {
      title: t('kimi.plugins.name'),
      dataIndex: 'name',
      key: 'name',
      render: (text: string) => <Text strong>{text}</Text>,
    },
    {
      title: t('kimi.plugins.version'),
      dataIndex: 'version',
      key: 'version',
      width: 120,
      render: (text: string) => text || '-',
    },
    {
      title: t('kimi.plugins.status'),
      dataIndex: 'enabled',
      key: 'enabled',
      width: 90,
      render: (enabled: boolean | undefined) =>
        enabled === false ? (
          <Tag>{t('kimi.plugins.disabled')}</Tag>
        ) : (
          <Tag color="success">{t('kimi.plugins.enabled')}</Tag>
        ),
    },
    {
      title: t('kimi.plugins.description'),
      dataIndex: 'description',
      key: 'description',
      render: (text: string) => text || '-',
    },
  ];

  return (
    <div>
      <div
        style={{
          fontSize: 12,
          color: 'var(--color-text-secondary)',
          borderLeft: '2px solid var(--color-border)',
          paddingLeft: 8,
          marginBottom: 12,
        }}
      >
        {t('kimi.plugins.globalHint')}
      </div>
      <Table
        dataSource={plugins}
        columns={columns}
        rowKey="name"
        loading={loading}
        pagination={false}
        size="small"
        locale={{
          emptyText: (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={t('kimi.plugins.empty')}
            />
          ),
        }}
      />
    </div>
  );
};

export default KimiPluginsPanel;
