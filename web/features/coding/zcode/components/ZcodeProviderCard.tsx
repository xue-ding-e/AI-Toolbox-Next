import React from 'react';
import { Button, Card, Dropdown, Empty, Space, Tag, Tooltip, Typography } from 'antd';
import {
  DeleteOutlined,
  EditOutlined,
  MoreOutlined,
  ThunderboltOutlined,
} from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { ZcodeProvider } from '@/types/zcode';
import { describeZcodeModel, parseZcodeProviderSettings } from '../utils/zcodeSettingsConfig';

const { Text } = Typography;

interface ZcodeProviderCardProps {
  provider: ZcodeProvider;
  onEdit: () => void;
  onApply: () => void;
  onDelete: () => void;
}

const ZcodeProviderCard: React.FC<ZcodeProviderCardProps> = ({
  provider,
  onEdit,
  onApply,
  onDelete,
}) => {
  const { t } = useTranslation();
  const settings = parseZcodeProviderSettings(provider.settingsConfig);
  const apiType = settings?.config?.api?.type;
  const baseUrl = settings?.config?.api?.baseUrl;
  const models = settings?.models ?? [];
  const defaultModelId = models.find((model) => model.isDefault)?.modelId;

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
    <Card size="small" styles={{ body: { padding: 12 } }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
        <div style={{ minWidth: 0, flex: 1 }}>
          <Space size="small" wrap>
            <Text strong>{provider.name}</Text>
            {provider.isApplied && (
              <Tag color="green">{t('zcode.provider.applied', { defaultValue: '默认' })}</Tag>
            )}
            {provider.isDisabled && (
              <Tag>{t('zcode.provider.disabled', { defaultValue: '已禁用' })}</Tag>
            )}
            {apiType && <Tag color="blue">{apiType}</Tag>}
          </Space>
          <div style={{ marginTop: 4 }}>
            <Text type="secondary" style={{ fontSize: 12 }}>
              {provider.id}
            </Text>
          </div>
          {baseUrl && (
            <div style={{ marginTop: 2 }}>
              <Text code style={{ fontSize: 12 }}>
                {baseUrl}
              </Text>
            </div>
          )}
          {provider.notes && (
            <div style={{ marginTop: 4 }}>
              <Text type="secondary" style={{ fontSize: 12 }}>
                {provider.notes}
              </Text>
            </div>
          )}
        </div>
        <Space size="small">
          <Tooltip
            title={t('zcode.provider.applyHint', {
              defaultValue: '设为 ZCode 新建会话的默认供应商与模型',
            })}
          >
            <Button size="small" icon={<ThunderboltOutlined />} onClick={onApply}>
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

      <div style={{ marginTop: 10 }}>
        {models.length === 0 ? (
          <Empty
            image={Empty.PRESENTED_IMAGE_SIMPLE}
            description={t('zcode.model.empty', { defaultValue: '暂无模型' })}
          />
        ) : (
          <Space orientation="vertical" size={2} style={{ width: '100%' }}>
            {models.map((model) => (
              <div key={model.modelId}>
                <Space size="small">
                  <Text style={{ fontSize: 12 }}>{model.displayName || model.modelId}</Text>
                  {model.modelId === defaultModelId && (
                    <Tag color="green" style={{ fontSize: 10 }}>
                      {t('zcode.model.default', { defaultValue: '默认' })}
                    </Tag>
                  )}
                  <Text type="secondary" style={{ fontSize: 11 }}>
                    {describeZcodeModel(model)}
                  </Text>
                </Space>
              </div>
            ))}
          </Space>
        )}
      </div>
    </Card>
  );
};

export default ZcodeProviderCard;
