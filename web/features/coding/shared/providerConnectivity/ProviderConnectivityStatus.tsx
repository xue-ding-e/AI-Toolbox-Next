import React from 'react';
import { Tooltip, theme } from 'antd';
import { LoadingOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { ProviderConnectivityStatusItem } from '@/components/common/ProviderCard/types';

interface ProviderConnectivityStatusProps {
  item?: ProviderConnectivityStatusItem;
}

const ProviderConnectivityStatus: React.FC<ProviderConnectivityStatusProps> = ({ item }) => {
  const { token } = theme.useToken();
  const { t } = useTranslation();

  if (!item || item.status === 'idle') {
    return null;
  }

  const ariaLabel = (() => {
    if (item.status === 'running') {
      return t('common.connectivityStatusRunning');
    }

    if (item.status === 'success') {
      if (item.modelId && typeof item.totalMs === 'number') {
        return t('common.connectivityStatusSuccessWithTiming', {
          model: item.modelId,
          totalMs: item.totalMs,
        });
      }

      if (item.modelId) {
        return t('common.connectivityStatusSuccessWithModel', {
          model: item.modelId,
        });
      }

      return t('common.connectivityStatusSuccess');
    }

    const message = item.tooltipMessage || item.errorMessage;
    if (message) {
      return t('common.connectivityStatusErrorWithMessage', {
        message,
      });
    }

    return t('common.connectivityStatusError');
  })();

  if (item.status === 'running') {
    return (
      <span
        role="status"
        aria-live="polite"
        aria-atomic="true"
        aria-label={ariaLabel}
        style={{
          display: 'inline-flex',
          alignItems: 'center',
          justifyContent: 'center',
          width: 14,
          height: 14,
          flexShrink: 0,
        }}
      >
        <LoadingOutlined spin style={{ fontSize: 12, color: token.colorPrimary }} />
      </span>
    );
  }

  const dot = (
    <span
      role="status"
      aria-live="polite"
      aria-atomic="true"
      aria-label={ariaLabel}
      style={{
        display: 'inline-block',
        width: 8,
        height: 8,
        borderRadius: '50%',
        backgroundColor: item.status === 'success' ? token.colorSuccess : token.colorError,
        flexShrink: 0,
      }}
    />
  );

  const tooltipTitle = item.tooltipMessage || (item.status === 'error' ? item.errorMessage || '' : '');

  if (tooltipTitle) {
    return <Tooltip title={tooltipTitle}>{dot}</Tooltip>;
  }

  return dot;
};

export default ProviderConnectivityStatus;
