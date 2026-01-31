import React from 'react';
import { Tooltip } from 'antd';
import { Sparkles } from 'lucide-react';
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
        <Sparkles className={styles.icon} size={14} />
        <span className={styles.text}>Skills</span>
      </div>
    </Tooltip>
  );
};
