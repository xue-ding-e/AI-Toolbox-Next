import React from 'react';
import { Tooltip } from 'antd';
import { AppstoreOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { useNavigate, useLocation } from 'react-router-dom';
import styles from './SkillsButton.module.less';

export const SkillsButton: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();

  const isActive = location.pathname.startsWith('/skills');

  const handleClick = () => {
    navigate('/skills');
  };

  return (
    <Tooltip title={t('skills.tooltip')}>
      <div
        className={`${styles.skillsButton} ${isActive ? styles.active : ''}`}
        onClick={handleClick}
      >
        <AppstoreOutlined className={styles.icon} />
        <span className={styles.text}>Skills</span>
      </div>
    </Tooltip>
  );
};
