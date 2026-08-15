import React from 'react';
import { Alert, Button, Space, Typography } from 'antd';
import { ReloadOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { OpenClawHealthWarning } from '@/types/openclaw';
import { buildHealthBannerItem } from '../healthBanner';

const { Text } = Typography;

interface Props {
  warnings: OpenClawHealthWarning[];
  onReload?: () => void;
}

const OpenClawHealthBanner: React.FC<Props> = ({ warnings, onReload }) => {
  const { t } = useTranslation();

  if (!warnings || warnings.length === 0) {
    return null;
  }

  return (
    <Alert
      type="warning"
      showIcon
      message={t('openclaw.healthBanner.title')}
      description={
        <ul style={{ margin: 0, paddingLeft: 18 }}>
          {warnings.map((warning, index) => (
            <li key={`${warning.code}-${index}`}>
              <Text>{buildHealthBannerItem(warning, t)}</Text>
            </li>
          ))}
        </ul>
      }
      action={
        onReload ? (
          <Space direction="vertical">
            <Button size="small" icon={<ReloadOutlined />} onClick={onReload}>
              {t('openclaw.healthBanner.refresh')}
            </Button>
          </Space>
        ) : undefined
      }
      style={{ marginBottom: 16 }}
    />
  );
};

export default OpenClawHealthBanner;