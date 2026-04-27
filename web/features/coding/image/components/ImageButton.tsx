import React from 'react';
import { Tooltip } from 'antd';
import { Image } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useLocation, useNavigate } from 'react-router-dom';
import styles from './ImageButton.module.less';

const ImageButton: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();

  const isActive = location.pathname.startsWith('/images');

  return (
    <Tooltip title={t('image.tooltip')}>
      <div
        className={`${styles.imageButton} ${isActive ? styles.active : ''}`}
        onClick={() => navigate('/images')}
      >
        <Image className={styles.icon} size={14} />
        <span className={styles.text}>{t('image.navLabel')}</span>
      </div>
    </Tooltip>
  );
};

export default ImageButton;
