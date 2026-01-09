import React from 'react';
import { Select, Typography, Spin, Empty, Button, Space, message } from 'antd';
import { SyncOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { OhMyOpenCodeConfig } from '@/types/ohMyOpenCode';
import { listOhMyOpenCodeConfigs, applyOhMyOpenCodeConfig } from '@/services/ohMyOpenCodeApi';

const { Text } = Typography;

interface OhMyOpenCodeConfigSelectorProps {
  modelOptions: { label: string; value: string }[];
  onConfigSelected?: (configId: string) => void;
}

const OhMyOpenCodeConfigSelector: React.FC<OhMyOpenCodeConfigSelectorProps> = ({
  modelOptions,
  onConfigSelected,
}) => {
  const { t } = useTranslation();
  const [loading, setLoading] = React.useState(false);
  const [configs, setConfigs] = React.useState<OhMyOpenCodeConfig[]>([]);
  const [selectedConfigId, setSelectedConfigId] = React.useState<string>('');

  // Load configs on mount
  React.useEffect(() => {
    loadConfigs();
  }, []);

  const loadConfigs = async () => {
    setLoading(true);
    try {
      const data = await listOhMyOpenCodeConfigs();
      setConfigs(data);
      const applied = data.find((c) => c.isApplied);
      if (applied) {
        setSelectedConfigId(applied.id);
      }
    } catch (error) {
      console.error('Failed to load configs:', error);
    } finally {
      setLoading(false);
    }
  };

  const handleChange = async (configId: string | undefined) => {
    if (!configId) {
      setSelectedConfigId('');
      return;
    }

    try {
      await applyOhMyOpenCodeConfig(configId);
      setSelectedConfigId(configId);
      message.success(t('opencode.ohMyOpenCode.applySuccess'));
      loadConfigs();
      if (onConfigSelected) {
        onConfigSelected(configId);
      }
    } catch {
      message.error(t('common.error'));
    }
  };

  const options = configs.map((config) => ({
    label: config.isApplied 
      ? `${config.name} ✓` 
      : config.name,
    value: config.id,
  }));

  if (loading) {
    return <Spin size="small" />;
  }

  if (configs.length === 0) {
    return (
      <Empty 
        description={t('opencode.ohMyOpenCode.noConfigs')} 
        style={{ margin: '8px 0' }}
      >
        <Button
          type="link"
          size="small"
          icon={<SyncOutlined />}
          onClick={loadConfigs}
        >
          {t('opencode.ohMyOpenCode.refresh')}
        </Button>
      </Empty>
    );
  }

  return (
    <Space.Compact style={{ width: '100%' }}>
      <Select
        value={selectedConfigId || undefined}
        onChange={handleChange}
        placeholder={t('opencode.ohMyOpenCode.selectConfig')}
        options={options}
        style={{ flex: 1 }}
        allowClear
      />
      <Button
        icon={<SyncOutlined />}
        onClick={loadConfigs}
        loading={loading}
      />
    </Space.Compact>
  );
};

export default OhMyOpenCodeConfigSelector;
