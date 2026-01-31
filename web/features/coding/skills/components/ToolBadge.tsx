import React from 'react';
import type { ToolOption } from '../types';
import styles from './ToolBadge.module.less';

interface ToolBadgeProps {
  tool: ToolOption;
  synced: boolean;
  mode?: string;
  onClick?: () => void;
}

export const ToolBadge: React.FC<ToolBadgeProps> = ({
  tool,
  synced,
  mode,
  onClick,
}) => {
  return (
    <button
      type="button"
      className={`${styles.badge} ${synced ? styles.active : styles.inactive}`}
      onClick={onClick}
      title={synced ? `${tool.label} (${mode || 'synced'})` : tool.label}
    >
      {synced && <span className={styles.indicator} />}
      {tool.label}
    </button>
  );
};
