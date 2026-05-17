import React from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate, useSearchParams } from 'react-router-dom';
import GeneralSettingsPage from './GeneralSettingsPage';
import styles from './SettingsPage.module.less';

const SettingsPage: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const requestedSection = searchParams.get('section');

  React.useEffect(() => {
    if (requestedSection === 'gateway') {
      navigate('/gateway/settings', { replace: true });
    }
  }, [navigate, requestedSection]);

  return (
    <div className={styles.page}>
      <div className={styles.header}>
        <h1 className={styles.title}>{t('settings.title')}</h1>
      </div>
      <div className={styles.content}>
        <GeneralSettingsPage />
      </div>
    </div>
  );
};

export default SettingsPage;
