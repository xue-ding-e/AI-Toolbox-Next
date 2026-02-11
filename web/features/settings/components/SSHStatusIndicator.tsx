/**
 * SSH Status Indicator Component
 *
 * Displays SSH sync status and allows users to open the settings modal
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
import './SSHStatusIndicator.css';

interface SSHStatusIndicatorProps {
  enabled: boolean;
  status: 'idle' | 'success' | 'error';
  onClick: () => void;
}

export const SSHStatusIndicator: React.FC<SSHStatusIndicatorProps> = ({
  enabled,
  status,
  onClick,
}) => {
  const { t } = useTranslation();

  const getStatusColor = (): string => {
    if (!enabled) return 'gray';
    if (status === 'error') return 'red';
    if (status === 'idle') return 'orange';  // 从未同步或待同步用橙色区分
    return 'green';
  };

  const color = getStatusColor();

  return (
    <div
      className="ssh-status-indicator"
      onClick={onClick}
      title={t('settings.ssh.indicator.tooltip')}
    >
      <span className={`ssh-status-dot ssh-status-dot-${color}`} />
      <span className="ssh-status-label">SSH</span>
    </div>
  );
};
