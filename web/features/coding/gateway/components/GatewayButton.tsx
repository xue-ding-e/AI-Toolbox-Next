import React from 'react';
import { Tooltip } from 'antd';
import { Network } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useLocation, useNavigate } from 'react-router-dom';
import { DEFAULT_GATEWAY_PATH } from '../utils/gatewayNavigation';
import styles from './GatewayButton.module.less';

const GatewayButton: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const isActive = location.pathname.startsWith('/gateway');

  return (
    <Tooltip title={t('gateway.tooltip')}>
      <div
        className={`${styles.gatewayButton} ${isActive ? styles.active : ''}`}
        onClick={() => navigate(DEFAULT_GATEWAY_PATH)}
      >
        <Network className={styles.icon} size={14} />
        <span className={styles.text}>{t('gateway.navLabel')}</span>
      </div>
    </Tooltip>
  );
};

export default GatewayButton;
