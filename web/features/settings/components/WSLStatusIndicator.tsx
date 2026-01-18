/**
 * WSL Status Indicator Component
 *
 * Displays WSL sync status and allows users to open the settings modal
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
import './WSLStatusIndicator.css';

interface WSLStatusIndicatorProps {
  enabled: boolean;
  status: 'idle' | 'success' | 'error';
  wslAvailable: boolean;
  onClick: () => void;
}

export const WSLStatusIndicator: React.FC<WSLStatusIndicatorProps> = ({
  enabled,
  status,
  onClick,
}) => {
  const { t } = useTranslation();

  // Determine the color of the status dot
  // Gray: sync disabled
  // Green: sync enabled and working (success or idle)
  // Red: sync failed
  const getStatusColor = (): string => {
    if (!enabled) return 'gray';
    if (status === 'error') return 'red';
    // enabled and (success or idle) => green
    return 'green';
  };

  const color = getStatusColor();

  return (
    <div
      className="wsl-status-indicator"
      onClick={onClick}
      title={t('settings.wsl.indicator.tooltip')}
    >
      <span className={`wsl-status-dot wsl-status-dot-${color}`} />
      <span className="wsl-status-label">WSL</span>
    </div>
  );
};
