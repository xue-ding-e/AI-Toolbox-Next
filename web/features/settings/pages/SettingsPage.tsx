import React from 'react';
import { Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import GeneralSettingsPage from './GeneralSettingsPage';

const { Title } = Typography;

const SettingsPage: React.FC = () => {
  const { t } = useTranslation();

  return (
    <div>
      <div style={{ marginBottom: 24 }}>
        <Title level={3} style={{ margin: 0 }}>
          {t('settings.title')}
        </Title>
      </div>
      <GeneralSettingsPage />
    </div>
  );
};

export default SettingsPage;
