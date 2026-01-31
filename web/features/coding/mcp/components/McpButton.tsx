import React from 'react';
import { Tooltip } from 'antd';
import { Blocks } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useNavigate, useLocation } from 'react-router-dom';
import styles from './McpButton.module.less';

export const McpButton: React.FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();

  const isActive = location.pathname.startsWith('/mcp');

  const handleClick = () => {
    navigate('/mcp');
  };

  return (
    <Tooltip title={t('mcp.tooltip')}>
      <div
        className={`${styles.mcpButton} ${isActive ? styles.active : ''}`}
        onClick={handleClick}
      >
        <Blocks className={styles.icon} size={14} />
        <span className={styles.text}>MCP</span>
      </div>
    </Tooltip>
  );
};

export default McpButton;
