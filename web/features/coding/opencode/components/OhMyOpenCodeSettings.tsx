import React from 'react';
import { Button, Typography, Collapse, Empty, Spin, Space, message, Modal } from 'antd';
import { PlusOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { OhMyOpenCodeConfig } from '@/types/ohMyOpenCode';
import OhMyOpenCodeConfigCard from './OhMyOpenCodeConfigCard';
import OhMyOpenCodeConfigModal, { OhMyOpenCodeConfigFormValues } from './OhMyOpenCodeConfigModal';
import { 
  listOhMyOpenCodeConfigs, 
  createOhMyOpenCodeConfig, 
  updateOhMyOpenCodeConfig, 
  deleteOhMyOpenCodeConfig,
  applyOhMyOpenCodeConfig,
  generateOhMyOpenCodeConfigId,
} from '@/services/ohMyOpenCodeApi';

const { Text } = Typography;

interface OhMyOpenCodeSettingsProps {
  modelOptions: { label: string; value: string }[];
  onConfigApplied?: (config: OhMyOpenCodeConfig) => void;
}

const OhMyOpenCodeSettings: React.FC<OhMyOpenCodeSettingsProps> = ({
  modelOptions,
  onConfigApplied,
}) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [configs, setConfigs] = React.useState<OhMyOpenCodeConfig[]>([]);
  const [modalOpen, setModalOpen] = React.useState(false);
  const [editingConfig, setEditingConfig] = React.useState<OhMyOpenCodeConfig | null>(null);
  const [isCopyMode, setIsCopyMode] = React.useState(false);

  // Load configs on mount
  React.useEffect(() => {
    loadConfigs();
  }, []);

  const loadConfigs = async () => {
    setLoading(true);
    try {
      const data = await listOhMyOpenCodeConfigs();
      setConfigs(data);
    } catch (error) {
      console.error('Failed to load configs:', error);
      message.error(t('common.error'));
    } finally {
      setLoading(false);
    }
  };

  const handleAddConfig = () => {
    setEditingConfig(null);
    setIsCopyMode(false);
    setModalOpen(true);
  };

  const handleEditConfig = (config: OhMyOpenCodeConfig) => {
    setEditingConfig(config);
    setIsCopyMode(false);
    setModalOpen(true);
  };

  const handleCopyConfig = (config: OhMyOpenCodeConfig) => {
    setEditingConfig(config);
    setIsCopyMode(true);
    setModalOpen(true);
  };

  const handleDeleteConfig = (config: OhMyOpenCodeConfig) => {
    Modal.confirm({
      title: t('common.confirm'),
      content: t('opencode.ohMyOpenCode.confirmDelete', { name: config.name }),
      onOk: async () => {
        try {
          await deleteOhMyOpenCodeConfig(config.id);
          message.success(t('common.success'));
          loadConfigs();
        } catch {
          message.error(t('common.error'));
        }
      },
    });
  };

  const handleApplyConfig = async (config: OhMyOpenCodeConfig) => {
    try {
      await applyOhMyOpenCodeConfig(config.id);
      message.success(t('opencode.ohMyOpenCode.applySuccess'));
      loadConfigs();
      if (onConfigApplied) {
        onConfigApplied(config);
      }
    } catch {
      message.error(t('common.error'));
    }
  };

  const handleModalSuccess = async (values: OhMyOpenCodeConfigFormValues) => {
    try {
      if (isCopyMode && editingConfig) {
        // Create new config with generated ID
        const newValues = {
          ...values,
          id: generateOhMyOpenCodeConfigId(),
        };
        await createOhMyOpenCodeConfig(newValues);
      } else if (editingConfig) {
        // Update existing config
        await updateOhMyOpenCodeConfig(values);
      } else {
        // Create new config
        await createOhMyOpenCodeConfig(values);
      }
      message.success(t('common.success'));
      setModalOpen(false);
      loadConfigs();
    } catch (error) {
      console.error('Failed to save config:', error);
      message.error(t('common.error'));
    }
  };

  const handleDuplicateId = () => {
    message.error(t('opencode.ohMyOpenCode.idExists'));
  };

  const existingIds = configs.map((c) => c.id);
  const appliedConfig = configs.find((c) => c.isApplied);

  const content = (
    <Spin spinning={loading}>
      {configs.length === 0 ? (
        <Empty 
          description={t('opencode.ohMyOpenCode.emptyText')}
          style={{ margin: '24px 0' }}
        />
      ) : (
        <div>
          {configs.map((config) => (
            <OhMyOpenCodeConfigCard
              key={config.id}
              config={config}
              isSelected={config.isApplied}
              onEdit={handleEditConfig}
              onCopy={handleCopyConfig}
              onDelete={handleDeleteConfig}
              onApply={handleApplyConfig}
            />
          ))}
        </div>
      )}
    </Spin>
  );

  return (
    <>
      <Collapse
        style={{ marginBottom: 16 }}
        defaultActiveKey={['oh-my-opencode']}
        items={[
          {
            key: 'oh-my-opencode',
            label: (
              <Space>
                <Text strong>{t('opencode.ohMyOpenCode.title')}</Text>
                {appliedConfig && (
                  <Text type="secondary" style={{ fontSize: 12 }}>
                    {t('opencode.ohMyOpenCode.current')}: {appliedConfig.name}
                  </Text>
                )}
              </Space>
            ),
            extra: (
              <Button
                type="primary"
                size="small"
                icon={<PlusOutlined />}
                onClick={(e) => {
                  e.stopPropagation();
                  handleAddConfig();
                }}
              >
                {t('opencode.ohMyOpenCode.addConfig')}
              </Button>
            ),
            children: content,
          },
        ]}
      />

      <OhMyOpenCodeConfigModal
        open={modalOpen}
        isEdit={!isCopyMode && !!editingConfig}
        initialValues={editingConfig || undefined}
        existingIds={existingIds}
        modelOptions={modelOptions}
        onCancel={() => {
          setModalOpen(false);
          setEditingConfig(null);
          setIsCopyMode(false);
        }}
        onSuccess={handleModalSuccess}
        onDuplicateId={handleDuplicateId}
      />
    </>
  );
};

export default OhMyOpenCodeSettings;
