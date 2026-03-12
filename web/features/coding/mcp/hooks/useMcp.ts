import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useMcpStore } from '../stores/mcpStore';

export const useMcp = () => {
  const { servers, tools, loading, showInTray, scanResult, fetchServers, fetchTools, fetchShowInTray, loadScanResult } = useMcpStore();
  const hasLoadedRef = useRef(false);

  useEffect(() => {
    if (hasLoadedRef.current) return;
    hasLoadedRef.current = true;

    const loadData = async () => {
      await fetchServers();
      await fetchTools();
      await fetchShowInTray();
    };
    loadData();
  }, [fetchServers, fetchTools, fetchShowInTray]);

  // 监听外部 MCP 配置变更（如从其他页面或 tray 修改）
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const setup = async () => {
      unlisten = await listen('mcp-changed', () => {
        fetchServers();
        fetchTools();
      });
    };
    setup();
    return () => { unlisten?.(); };
  }, [fetchServers, fetchTools]);

  return {
    servers,
    tools,
    loading,
    showInTray,
    scanResult,
    refresh: fetchServers,
    refreshTools: fetchTools,
    triggerScan: loadScanResult,
  };
};

export default useMcp;
