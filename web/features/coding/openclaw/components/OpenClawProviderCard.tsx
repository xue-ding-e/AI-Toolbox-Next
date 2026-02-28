import React from 'react';
import { Button, Card, Space, Table, Tag, Typography, Popconfirm, Tooltip } from 'antd';
import {
  EditOutlined,
  DeleteOutlined,
  PlusOutlined,
  ApiOutlined,
  CloudDownloadOutlined,
} from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { OpenClawProviderConfig, OpenClawModel } from '@/types/openclaw';

const { Text } = Typography;

interface Props {
  providerId: string;
  config: OpenClawProviderConfig;
  onEdit: () => void;
  onDelete: () => void;
  onAddModel: () => void;
  onEditModel: (model: OpenClawModel) => void;
  onDeleteModel: (modelId: string) => void;
  onConnectivityTest: () => void;
  onFetchModels: () => void;
}

const OpenClawProviderCard: React.FC<Props> = ({
  providerId,
  config,
  onEdit,
  onDelete,
  onAddModel,
  onEditModel,
  onDeleteModel,
  onConnectivityTest,
  onFetchModels,
}) => {
  const { t } = useTranslation();

  const isAuthReady = Boolean(config.baseUrl?.trim() && config.apiKey?.trim());
  const authTooltip = !isAuthReady ? t('openclaw.providers.completeUrlAndKey') : '';

  const modelColumns = [
    {
      title: t('openclaw.providers.modelId'),
      dataIndex: 'id',
      key: 'id',
      width: '30%',
      render: (text: string) => <Text code>{text}</Text>,
    },
    {
      title: t('openclaw.providers.modelName'),
      dataIndex: 'name',
      key: 'name',
      width: '20%',
      render: (text: string | undefined) => text || '-',
    },
    {
      title: t('openclaw.providers.contextLimit'),
      dataIndex: 'contextWindow',
      key: 'contextWindow',
      width: '15%',
      render: (val: number | undefined) => (val ? val.toLocaleString() : '-'),
    },
    {
      title: t('openclaw.providers.outputLimit'),
      dataIndex: 'maxTokens',
      key: 'maxTokens',
      width: '12%',
      render: (val: number | undefined) => (val ? val.toLocaleString() : '-'),
    },
    {
      title: 'Cost',
      key: 'cost',
      width: '13%',
      render: (_: unknown, record: OpenClawModel) => {
        if (!record.cost) return '-';
        return `$${record.cost.input}/$${record.cost.output}`;
      },
    },
    {
      title: '',
      key: 'actions',
      width: '10%',
      render: (_: unknown, record: OpenClawModel) => (
        <Space size={0}>
          <Button
            type="link"
            size="small"
            icon={<EditOutlined />}
            onClick={() => onEditModel(record)}
          />
          <Popconfirm
            title={t('openclaw.providers.deleteModel') + '?'}
            onConfirm={() => onDeleteModel(record.id)}
            okText={t('common.confirm')}
            cancelText={t('common.cancel')}
          >
            <Button type="link" size="small" danger icon={<DeleteOutlined />} />
          </Popconfirm>
        </Space>
      ),
    },
  ];

  return (
    <Card
      size="small"
      style={{ marginBottom: 12 }}
      title={
        <Space>
          <Text strong>{providerId}</Text>
          {config.api && <Tag color="blue">{config.api}</Tag>}
        </Space>
      }
      extra={
        <Space size={0}>
          <Button type="link" size="small" icon={<EditOutlined />} onClick={onEdit}>
            {t('common.edit')}
          </Button>
          <Popconfirm
            title={t('openclaw.providers.confirmDelete', { name: providerId })}
            onConfirm={onDelete}
            okText={t('common.confirm')}
            cancelText={t('common.cancel')}
          >
            <Button type="link" size="small" danger icon={<DeleteOutlined />}>
              {t('common.delete')}
            </Button>
          </Popconfirm>
        </Space>
      }
    >
      {config.baseUrl && (
        <div style={{ marginBottom: 8 }}>
          <Text type="secondary">Base URL: </Text>
          <Text code>{config.baseUrl}</Text>
        </div>
      )}

      <Table
        dataSource={config.models || []}
        columns={modelColumns}
        rowKey="id"
        size="small"
        pagination={false}
        locale={{ emptyText: t('openclaw.providers.emptyText') }}
      />

      <div style={{ marginTop: 8, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <Space size={0}>
          <Tooltip title={authTooltip}>
            <span>
              <Button
                size="small"
                type="text"
                style={{ fontSize: 12 }}
                onClick={onConnectivityTest}
                disabled={!isAuthReady || (config.models || []).length === 0}
              >
                <ApiOutlined style={{ marginRight: 4 }} />
                {t('openclaw.providers.connectivityTest')}
              </Button>
            </span>
          </Tooltip>
          <Tooltip title={authTooltip}>
            <span>
              <Button
                size="small"
                type="text"
                style={{ fontSize: 12 }}
                onClick={onFetchModels}
                disabled={!isAuthReady}
              >
                <CloudDownloadOutlined style={{ marginRight: 4 }} />
                {t('openclaw.providers.fetchModels')}
              </Button>
            </span>
          </Tooltip>
        </Space>
        <Button type="dashed" size="small" icon={<PlusOutlined />} onClick={onAddModel}>
          {t('openclaw.providers.addModel')}
        </Button>
      </div>
    </Card>
  );
};

export default OpenClawProviderCard;
