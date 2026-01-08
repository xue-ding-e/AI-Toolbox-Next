import React from 'react';
import { Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import GeneralSettingsPage from './GeneralSettingsPage';

const { Title } = Typography;

const SettingsPage: React.FC = () => {
  const { t } = useTranslation();

  return (
    <div>
      <Title level={4} style={{ marginBottom: 16 }}>
        {t('settings.title')}
      </Title>
      <GeneralSettingsPage />
    </div>
  );
};

export default SettingsPage;
