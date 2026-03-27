import React from 'react';
import { Tag } from 'antd';
import { EnvironmentOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';

type ConfigPathSource = 'env' | 'custom' | 'shell' | 'default';

interface ConfigPathSourceTagProps {
  source: ConfigPathSource | string;
  fontSize?: number;
}

const ConfigPathSourceTag: React.FC<ConfigPathSourceTagProps> = ({
  source,
  fontSize = 12,
}) => {
  const { t } = useTranslation();

  if (source === 'env') {
    return (
      <Tag color="blue" icon={<EnvironmentOutlined />} style={{ fontSize }}>
        {t('opencode.configPathSource.fromEnv')}
      </Tag>
    );
  }

  if (source === 'custom') {
    return (
      <Tag color="green" style={{ fontSize }}>
        {t('opencode.configPathSource.custom')}
      </Tag>
    );
  }

  if (source === 'shell') {
    return (
      <Tag color="cyan" style={{ fontSize }}>
        {t('opencode.configPathSource.fromShell')}
      </Tag>
    );
  }

  return <Tag style={{ fontSize }}>{t('opencode.configPathSource.default')}</Tag>;
};

export default ConfigPathSourceTag;
