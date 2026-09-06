import { invoke } from '@tauri-apps/api/core';
import type {
  McpServer,
  CreateMcpServerInput,
  UpdateMcpServerInput,
  McpSyncResult,
  McpImportResult,
  McpGroupInventoryPreview,
  McpGroupRecord,
  McpTool,
  McpScanResult,
  McpPackageVersionResolveRequest,
  McpPackageVersionResolveResult,
} from '../types';

// Server CRUD
export const listMcpServers = async (): Promise<McpServer[]> => {
  return invoke<McpServer[]>('mcp_list_servers');
};

export const resolveMcpPackageVersions = async (
  requests: McpPackageVersionResolveRequest[],
): Promise<McpPackageVersionResolveResult[]> => {
  return invoke<McpPackageVersionResolveResult[]>('mcp_resolve_package_versions', { requests });
};

export const createMcpServer = async (input: CreateMcpServerInput): Promise<McpServer> => {
  return invoke<McpServer>('mcp_create_server', { input });
};

export const updateMcpServer = async (serverId: string, input: UpdateMcpServerInput): Promise<McpServer> => {
  return invoke<McpServer>('mcp_update_server', { serverId, input });
};

export const deleteMcpServer = async (serverId: string): Promise<void> => {
  return invoke('mcp_delete_server', { serverId });
};

export const toggleMcpTool = async (serverId: string, toolKey: string): Promise<boolean> => {
  return invoke<boolean>('mcp_toggle_tool', { serverId, toolKey });
};

/**
 * Set the management enabled/disabled state for an MCP server.
 * Disable removes the server from all tool configs and records prior bindings into the server;
 * enable only flips the flag and returns the recorded previous tools so the caller can confirm
 * which to restore through `syncMcpToTool`.
 */
export const setMcpManagementEnabled = async (
  serverId: string,
  enabled: boolean,
): Promise<string[]> => {
  return invoke<string[]>('mcp_set_management_enabled', { serverId, enabled });
};

/**
 * Restore a just-re-enabled server's tool bindings: the backend writes the chosen
 * tools back into `enabled_tools` and re-syncs this server into each tool config.
 * Returns per-tool sync results (tool/success/error_message).
 */
export const restoreMcpTools = async (
  serverId: string,
  tools: string[],
): Promise<McpSyncResult[]> => {
  return invoke<McpSyncResult[]>('mcp_restore_tools', { serverId, tools });
};

export const reorderMcpServers = async (ids: string[]): Promise<void> => {
  return invoke('mcp_reorder_servers', { ids });
};

export const updateMcpMetadata = async (
  serverId: string,
  userGroup: string | null,
  userNote: string | null,
  tags?: string[] | null,
): Promise<void> => {
  return invoke('mcp_update_metadata', { serverId, userGroup, userNote, tags });
};

/**
 * Export every managed server's group assignment as a JSON inventory file at
 * the given path (chosen via the save dialog). Returns the written path.
 */
export const exportMcpGroupInventory = async (path: string): Promise<string> => {
  return invoke<string>('mcp_export_group_inventory', { path });
};

/** Validate an inventory file and report what would change, without writing. */
export const previewMcpGroupInventoryImport = async (
  path: string,
): Promise<McpGroupInventoryPreview> => {
  return invoke<McpGroupInventoryPreview>('mcp_preview_group_inventory_import', { path });
};

/** Apply a validated inventory file; returns the same preview shape. */
export const applyMcpGroupInventoryImport = async (
  path: string,
): Promise<McpGroupInventoryPreview> => {
  return invoke<McpGroupInventoryPreview>('mcp_apply_group_inventory_import', { path });
};

// Managed groups (group management modal)

export const getMcpGroups = async (): Promise<McpGroupRecord[]> => {
  return invoke<McpGroupRecord[]>('mcp_list_groups');
};

export const saveMcpGroup = async (
  name: string,
  note: string | null,
  sortIndex: number,
  id?: string,
): Promise<McpGroupRecord> => {
  // Backend parameter is `groupId`; passing a bare `id` would deserialize as
  // None and turn every edit into a duplicate-name create.
  return invoke<McpGroupRecord>('mcp_save_group', { groupId: id, name, note, sortIndex });
};

export const deleteMcpGroup = async (groupId: string): Promise<void> => {
  return invoke('mcp_delete_group', { groupId });
};

// Sync operations
export const syncMcpToTool = async (toolKey: string): Promise<McpSyncResult[]> => {
  return invoke<McpSyncResult[]>('mcp_sync_to_tool', { toolKey });
};

export const syncMcpAll = async (): Promise<McpSyncResult[]> => {
  return invoke<McpSyncResult[]>('mcp_sync_all');
};

export const importMcpFromTool = async (
  toolKey: string,
  enabledTools?: string[],
  followCcSwitchMarks?: boolean,
): Promise<McpImportResult> => {
  return invoke<McpImportResult>('mcp_import_from_tool', { toolKey, enabledTools, followCcSwitchMarks });
};

// Tools API
export const getMcpTools = async (): Promise<McpTool[]> => {
  return invoke<McpTool[]>('mcp_get_tools');
};

// Scan for existing MCP servers in tool configs
export const scanMcpServers = async (): Promise<McpScanResult> => {
  return invoke<McpScanResult>('mcp_scan_servers');
};

// Preferences
export const getMcpShowInTray = async (): Promise<boolean> => {
  return invoke<boolean>('mcp_get_show_in_tray');
};

export const setMcpShowInTray = async (enabled: boolean): Promise<void> => {
  return invoke('mcp_set_show_in_tray', { enabled });
};

export const getMcpPreferredTools = async (): Promise<string[]> => {
  return invoke<string[]>('mcp_get_preferred_tools');
};

export const setMcpPreferredTools = async (tools: string[]): Promise<void> => {
  return invoke('mcp_set_preferred_tools', { tools });
};

export const getMcpLimitAddMoreToPreferredTools = async (): Promise<boolean> => {
  return invoke<boolean>('mcp_get_limit_add_more_to_preferred_tools');
};

export const setMcpLimitAddMoreToPreferredTools = async (enabled: boolean): Promise<void> => {
  return invoke('mcp_set_limit_add_more_to_preferred_tools', { enabled });
};

export const getMcpSyncDisabledToOpencode = async (): Promise<boolean> => {
  return invoke<boolean>('mcp_get_sync_disabled_to_opencode');
};

export const setMcpSyncDisabledToOpencode = async (enabled: boolean): Promise<void> => {
  return invoke('mcp_set_sync_disabled_to_opencode', { enabled });
};

// Custom Tool Management
export interface AddMcpCustomToolInput {
  key: string;
  displayName: string;
  relativeDetectDir?: string;
  mcpConfigPath: string;
  mcpConfigFormat: 'json' | 'toml';
  mcpField: string;
  /** Optional brand icon image URL; http(s) only, empty string clears it. */
  iconUrl?: string;
}

export const addMcpCustomTool = async (input: AddMcpCustomToolInput): Promise<void> => {
  return invoke('mcp_add_custom_tool', { ...input });
};

export const removeMcpCustomTool = async (key: string): Promise<void> => {
  return invoke('mcp_remove_custom_tool', { key });
};

// Favorite MCP API
export interface FavoriteMcp {
  id: string;
  name: string;
  server_type: 'stdio' | 'http' | 'sse';
  server_config: Record<string, unknown>;
  description?: string;
  tags: string[];
  is_preset: boolean;
  created_at: number;
  updated_at: number;
}

export interface FavoriteMcpInput {
  name: string;
  server_type: 'stdio' | 'http' | 'sse';
  server_config: Record<string, unknown>;
  description?: string;
  tags?: string[];
}

export const listMcpFavorites = async (): Promise<FavoriteMcp[]> => {
  return invoke<FavoriteMcp[]>('mcp_list_favorites');
};

export const upsertMcpFavorite = async (input: FavoriteMcpInput): Promise<FavoriteMcp> => {
  return invoke<FavoriteMcp>('mcp_upsert_favorite', { input });
};

export const deleteMcpFavorite = async (favoriteId: string): Promise<void> => {
  return invoke('mcp_delete_favorite', { favoriteId });
};

export const initMcpDefaultFavorites = async (): Promise<number> => {
  return invoke<number>('mcp_init_default_favorites');
};
