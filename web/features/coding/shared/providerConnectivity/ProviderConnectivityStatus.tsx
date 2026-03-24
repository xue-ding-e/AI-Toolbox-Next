import React from 'react';
import { Tooltip, theme } from 'antd';
import { LoadingOutlined } from '@ant-design/icons';
import type { ProviderConnectivityStatusItem } from '@/components/common/ProviderCard/types';

interface ProviderConnectivityStatusProps {
  item?: ProviderConnectivityStatusItem;
}

const ProviderConnectivityStatus: React.FC<ProviderConnectivityStatusProps> = ({ item }) => {
  const { token } = theme.useToken();

  if (!item || item.status === 'idle') {
    return null;
  }

  if (item.status === 'running') {
    return (
      <span
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
