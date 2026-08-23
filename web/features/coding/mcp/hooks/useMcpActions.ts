import { useState } from 'react';
import { message } from 'antd';
import { useTranslation } from 'react-i18next';
import { useMcpStore } from '../stores/mcpStore';
import * as mcpApi from '../services/mcpApi';
import type { CreateMcpServerInput, UpdateMcpServerInput } from '../types';

export const useMcpActions = () => {
  const { t } = useTranslation();
  const { servers, addServer, updateServer, removeServer, fetchServers } = useMcpStore();
  const [actionLoading, setActionLoading] = useState(false);

  const createServer = async (input: CreateMcpServerInput) => {
    try {
      const server = await mcpApi.createMcpServer(input);
      addServer(server);
      message.success(t('mcp.serverCreated'));
      return server;
    } catch (error) {
      message.error(t('mcp.serverCreateFailed') + ': ' + String(error));
      throw error;
    }
  };

  const editServer = async (serverId: string, input: UpdateMcpServerInput) => {
    try {
      const server = await mcpApi.updateMcpServer(serverId, input);
      updateServer(server);
      message.success(t('mcp.serverUpdated'));
      return server;
    } catch (error) {
      message.error(t('mcp.serverUpdateFailed') + ': ' + String(error));
      throw error;
    }
  };

  const deleteServer = async (serverId: string) => {
    try {
      await mcpApi.deleteMcpServer(serverId);
      removeServer(serverId);
      message.success(t('mcp.serverDeleted'));
    } catch (error) {
      message.error(t('mcp.serverDeleteFailed') + ': ' + String(error));
      throw error;
    }
  };

  const toggleTool = async (serverId: string, toolKey: string) => {
    try {
      const isEnabled = await mcpApi.toggleMcpTool(serverId, toolKey);
      // Refresh servers to get updated state
      await fetchServers();
      return isEnabled;
    } catch (error) {
      message.error(t('mcp.toggleToolFailed') + ': ' + String(error));
      throw error;
    }
  };

  const reorderServers = async (ids: string[]) => {
    try {
      await mcpApi.reorderMcpServers(ids);
      // Refresh to get updated order
      await fetchServers();
    } catch (error) {
      message.error(t('mcp.reorderFailed') + ': ' + String(error));
      throw error;
    }
  };

  const syncToTool = async (toolKey: string) => {
    try {
      const results = await mcpApi.syncMcpToTool(toolKey);
      const failed = results.filter((r) => !r.success);
      if (failed.length > 0) {
        message.warning(t('mcp.syncPartialFailed', { count: failed.length }));
      } else {
        message.success(t('mcp.syncSuccess'));
      }
      await fetchServers();
      return results;
    } catch (error) {
      message.error(t('mcp.syncFailed') + ': ' + String(error));
      throw error;
    }
  };

  const syncAll = async () => {
    try {
      const results = await mcpApi.syncMcpAll();
      const failed = results.filter((r) => !r.success);
      if (failed.length > 0) {
        message.warning(t('mcp.syncPartialFailed', { count: failed.length }));
      } else {
        message.success(t('mcp.syncAllSuccess'));
      }
      await fetchServers();
      return results;
    } catch (error) {
      message.error(t('mcp.syncFailed') + ': ' + String(error));
      throw error;
    }
  };

  const importFromTool = async (toolKey: string) => {
    try {
      const result = await mcpApi.importMcpFromTool(toolKey);
      if (result.servers_imported > 0) {
        message.success(t('mcp.importSuccess', { count: result.servers_imported }));
      } else if (result.servers_skipped > 0) {
        message.info(t('mcp.importSkipped', { count: result.servers_skipped }));
      } else {
        message.info(t('mcp.importNoServers'));
      }
      await fetchServers();
      return result;
    } catch (error) {
      message.error(t('mcp.importFailed') + ': ' + String(error));
      throw error;
    }
  };

  // Disable a server: mark management_enabled=false and cancel sync from all tools
  // (backend records disabled_previous_tools and removes tool config best-effort).
  const disableServer = async (serverId: string) => {
    setActionLoading(true);
    try {
      await mcpApi.setMcpManagementEnabled(serverId, false);
      await fetchServers();
      message.success(t('mcp.serverDisabled'));
    } catch (error) {
      message.error(t('mcp.serverDisableFailed') + ': ' + String(error));
      throw error;
    } finally {
      setActionLoading(false);
    }
  };

  // Enable a server (management flag only). Returns the previously recorded tool
  // bindings so the caller can confirm which tools to restore via syncMcpToTool.
  const enableServer = async (serverId: string): Promise<string[]> => {
    setActionLoading(true);
    try {
      const previousTools = await mcpApi.setMcpManagementEnabled(serverId, true);
      await fetchServers();
      return previousTools;
    } catch (error) {
      message.error(t('mcp.serverEnableFailed') + ': ' + String(error));
      throw error;
    } finally {
      setActionLoading(false);
    }
  };

  // Restore a just-enabled server's tool bindings: the backend writes the chosen
  // tools back into enabled_tools and re-syncs this server into each tool config.
  const restoreTools = async (serverId: string, tools: string[]) => {
    setActionLoading(true);
    try {
      const results = await mcpApi.restoreMcpTools(serverId, tools);
      const failed = results.filter((r) => !r.success);
      if (failed.length > 0) {
        message.warning(t('mcp.restorePartialFailed', { count: failed.length }));
      } else {
        message.success(t('mcp.serverRestored', { count: tools.length }));
      }
      await fetchServers();
      return results;
    } catch (error) {
      message.error(t('mcp.restoreFailed') + ': ' + String(error));
      throw error;
    } finally {
      setActionLoading(false);
    }
  };

  // Batch enable/disable. Given the desired target state, skip servers already in
  // that state. On enable, collect restore tools per server for a unified confirm.
  const batchSetManagementEnabled = async (
    serverIds: string[],
    enabled: boolean,
  ): Promise<{ restored: Record<string, string[]>; succeeded: number; failed: number }> => {
    const targetServers = servers.filter(
      (s) => serverIds.includes(s.id) && s.management_enabled !== enabled,
    );
    if (targetServers.length === 0) {
      return { restored: {}, succeeded: 0, failed: 0 };
    }
    setActionLoading(true);
    const restored: Record<string, string[]> = {};
    let succeeded = 0;
    let failed = 0;
    try {
      for (const server of targetServers) {
        try {
          const previous = await mcpApi.setMcpManagementEnabled(server.id, enabled);
          if (enabled) {
            restored[server.id] = previous;
          }
          succeeded++;
        } catch {
          failed++;
        }
      }
      await fetchServers();
      return { restored, succeeded, failed };
    } finally {
      setActionLoading(false);
    }
  };

  return {
    actionLoading,
    createServer,
    editServer,
    deleteServer,
    toggleTool,
    reorderServers,
    syncToTool,
    syncAll,
    importFromTool,
    disableServer,
    enableServer,
    restoreTools,
    batchSetManagementEnabled,
  };
};

export default useMcpActions;
