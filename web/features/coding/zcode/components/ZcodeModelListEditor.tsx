import React from 'react';
import { Button, Empty, Space, Switch, Table, Tag, Tooltip, Typography } from 'antd';
import { DeleteOutlined, EditOutlined, PlusOutlined, StarFilled } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { ZcodeModelRow } from '@/types/zcode';
import { describeZcodeModel } from '../utils/zcodeSettingsConfig';
import ZcodeModelFormModal from './ZcodeModelFormModal';

const { Text } = Typography;

interface ZcodeModelListEditorProps {
  models: ZcodeModelRow[];
  onChange: (models: ZcodeModelRow[]) => void;
}

/**
 * Model table for one provider.
 *
 * Exactly one row carries `isDefault` — the provider's default model, which the
 * backend writes into `defaultModelSelection` when the provider is applied.
 */
const ZcodeModelListEditor: React.FC<ZcodeModelListEditorProps> = ({ models, onChange }) => {
  const { t } = useTranslation();
  const [modalOpen, setModalOpen] = React.useState(false);
  const [editingIndex, setEditingIndex] = React.useState<number | null>(null);

  const handleAdd = () => {
    setEditingIndex(null);
    setModalOpen(true);
  };

  const handleEdit = (index: number) => {
    setEditingIndex(index);
    setModalOpen(true);
  };

  const handleDelete = (index: number) => {
    const next = models.filter((_, itemIndex) => itemIndex !== index);
    // Deleting the default row would leave the provider without a model to
    // select; promote the first remaining row so `apply` still has a target.
    if (models[index]?.isDefault && next.length > 0) {
      next[0] = { ...next[0], isDefault: true };
    }
    onChange(next);
  };

  const handleSetDefault = (index: number) => {
    onChange(
      models.map((model, itemIndex) => ({
        ...model,
        isDefault: itemIndex === index,
      })),
    );
  };

  const handleSubmit = (row: ZcodeModelRow) => {
    if (editingIndex === null) {
      // The first model added becomes the default so a fresh provider is
      // immediately usable.
      const isFirst = models.length === 0;
      onChange([...models, { ...row, isDefault: isFirst }]);
    } else {
      onChange(
        models.map((model, itemIndex) =>
          itemIndex === editingIndex ? { ...row, isDefault: model.isDefault } : model,
        ),
      );
    }
    setModalOpen(false);
    setEditingIndex(null);
  };

  return (
    <Space orientation="vertical" style={{ width: '100%' }} size="small">
      <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
        <Button size="small" type="primary" icon={<PlusOutlined />} onClick={handleAdd}>
          {t('zcode.model.add', { defaultValue: '添加模型' })}
        </Button>
      </div>

      {models.length === 0 ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={t('zcode.model.empty', { defaultValue: '暂无模型' })}
        />
      ) : (
        <Table<ZcodeModelRow>
          size="small"
          rowKey="modelId"
          pagination={false}
          dataSource={models}
          columns={[
            {
              title: t('zcode.model.modelId', { defaultValue: '模型 ID' }),
              dataIndex: 'modelId',
              render: (modelId: string, row) => (
                <Space size="small">
                  <Text>{row.displayName || modelId}</Text>
                  {row.displayName && (
                    <Text type="secondary" style={{ fontSize: 11 }}>
                      {modelId}
                    </Text>
                  )}
                  {row.isDefault && <Tag color="green">{t('zcode.model.default', { defaultValue: '默认' })}</Tag>}
                </Space>
              ),
            },
            {
              title: t('zcode.model.ruleKind', { defaultValue: '配置方式' }),
              dataIndex: 'ruleKind',
              width: 100,
              render: (kind: ZcodeModelRow['ruleKind']) => (
                <Tag color={kind === 'manual' ? 'orange' : 'blue'}>
                  {kind === 'manual'
                    ? t('zcode.model.ruleKindManual', { defaultValue: '手动配置' })
                    : t('zcode.model.ruleKindSmart', { defaultValue: '智能配置' })}
                </Tag>
              ),
            },
            {
              title: t('zcode.model.summary', { defaultValue: '参数摘要' }),
              render: (_, row) => (
                <Text type="secondary" style={{ fontSize: 12 }}>
                  {describeZcodeModel(row) || '-'}
                </Text>
              ),
            },
            {
              title: t('zcode.model.enabledLabel', { defaultValue: '启用' }),
              width: 70,
              render: (_, row, index) => (
                <Switch
                  size="small"
                  checked={row.enabled !== false}
                  onChange={(checked) =>
                    onChange(
                      models.map((model, itemIndex) =>
                        itemIndex === index ? { ...model, enabled: checked } : model,
                      ),
                    )
                  }
                />
              ),
            },
            {
              title: t('zcode.model.actions'),
              width: 120,
              render: (_, row, index) => (
                <Space size="small">
                  <Tooltip title={t('zcode.model.setDefault', { defaultValue: '设为默认模型' })}>
                    <Button
                      size="small"
                      type="text"
                      icon={<StarFilled style={{ color: row.isDefault ? '#faad14' : undefined }} />}
                      disabled={row.isDefault}
                      onClick={() => handleSetDefault(index)}
                    />
                  </Tooltip>
                  <Button
                    size="small"
                    type="text"
                    icon={<EditOutlined />}
                    onClick={() => handleEdit(index)}
                  />
                  <Button
                    size="small"
                    type="text"
                    danger
                    icon={<DeleteOutlined />}
                    onClick={() => handleDelete(index)}
                  />
                </Space>
              ),
            },
          ]}
        />
      )}

      {modalOpen && (
        <ZcodeModelFormModal
          open={modalOpen}
          isEdit={editingIndex !== null}
          initialValues={editingIndex !== null ? models[editingIndex] : undefined}
          onCancel={() => {
            setModalOpen(false);
            setEditingIndex(null);
          }}
          onSubmit={handleSubmit}
        />
      )}
    </Space>
  );
};

export default ZcodeModelListEditor;
