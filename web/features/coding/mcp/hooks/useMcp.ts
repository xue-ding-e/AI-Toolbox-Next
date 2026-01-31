import { useEffect } from 'react';
import { useMcpStore } from '../stores/mcpStore';

export const useMcp = () => {
  const { servers, tools, loading, showInTray, scanResult, fetchServers, fetchTools, fetchShowInTray, loadScanResult } = useMcpStore();

  useEffect(() => {
    fetchServers();
    fetchTools();
    fetchShowInTray();
    loadScanResult();
  }, [fetchServers, fetchTools, fetchShowInTray, loadScanResult]);

  return {
    servers,
    tools,
    loading,
    showInTray,
    scanResult,
    refresh: fetchServers,
    refreshTools: fetchTools,
  };
};

export default useMcp;
