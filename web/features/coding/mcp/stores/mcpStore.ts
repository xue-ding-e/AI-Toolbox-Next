import { create } from 'zustand';
import type { McpServer, McpTool, McpScanResult } from '../types';
import * as mcpApi from '../services/mcpApi';

interface McpState {
  servers: McpServer[];
  tools: McpTool[];
  loading: boolean;
  showInTray: boolean;
  scanResult: McpScanResult | null;

  // Modal states
  isSettingsModalOpen: boolean;
  isImportModalOpen: boolean;
  isImportJsonModalOpen: boolean;

  // Actions
  fetchServers: () => Promise<void>;
  fetchTools: () => Promise<void>;
  fetchShowInTray: () => Promise<void>;
  loadScanResult: () => Promise<void>;
  setServers: (servers: McpServer[]) => void;
  addServer: (server: McpServer) => void;
  updateServer: (server: McpServer) => void;
  removeServer: (serverId: string) => void;
  setShowInTray: (enabled: boolean) => Promise<void>;
  setSettingsModalOpen: (open: boolean) => void;
  setImportModalOpen: (open: boolean) => void;
  setImportJsonModalOpen: (open: boolean) => void;
}

export const useMcpStore = create<McpState>()((set) => ({
  servers: [],
  tools: [],
  loading: false,
  showInTray: false,
  scanResult: null,
  isSettingsModalOpen: false,
  isImportModalOpen: false,
  isImportJsonModalOpen: false,

  fetchServers: async () => {
    set({ loading: true });
    try {
      const servers = await mcpApi.listMcpServers();
      set({ servers });
    } catch (error) {
      console.error('Failed to fetch MCP servers:', error);
    } finally {
      set({ loading: false });
    }
  },

  fetchTools: async () => {
    try {
      const tools = await mcpApi.getMcpTools();
      set({ tools });
    } catch (error) {
      console.error('Failed to fetch MCP tools:', error);
    }
  },

  fetchShowInTray: async () => {
    try {
      const showInTray = await mcpApi.getMcpShowInTray();
      set({ showInTray });
    } catch (error) {
      console.error('Failed to fetch MCP show in tray:', error);
    }
  },

  loadScanResult: async () => {
    try {
      const scanResult = await mcpApi.scanMcpServers();
      set({ scanResult });
    } catch (error) {
      console.error('Failed to scan MCP servers:', error);
    }
  },

  setServers: (servers) => set({ servers }),

  addServer: (server) => set((state) => ({ servers: [...state.servers, server] })),

  updateServer: (server) => set((state) => ({
    servers: state.servers.map((s) => (s.id === server.id ? server : s)),
  })),

  removeServer: (serverId) => set((state) => ({
    servers: state.servers.filter((s) => s.id !== serverId),
  })),

  setShowInTray: async (enabled) => {
    try {
      await mcpApi.setMcpShowInTray(enabled);
      set({ showInTray: enabled });
    } catch (error) {
      console.error('Failed to set MCP show in tray:', error);
    }
  },

  setSettingsModalOpen: (open) => set({ isSettingsModalOpen: open }),

  setImportModalOpen: (open) => set({ isImportModalOpen: open }),

  setImportJsonModalOpen: (open) => set({ isImportJsonModalOpen: open }),
}));
