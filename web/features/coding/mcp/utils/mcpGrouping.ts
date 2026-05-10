import type { McpGroup, McpServer } from '../types';

export const CUSTOM_UNGROUPED_GROUP_KEY = 'custom:__ungrouped__';

export function normalizeMcpMetadataText(value: string | null | undefined): string | null {
  const trimmed = value?.trim() ?? '';
  return trimmed ? trimmed : null;
}

export function getMcpGroupOptions(servers: McpServer[]): string[] {
  const groups = new Set<string>();
  for (const server of servers) {
    const group = normalizeMcpMetadataText(server.user_group);
    if (group) {
      groups.add(group);
    }
  }
  return [...groups].sort((left, right) => left.localeCompare(right));
}

export function getMcpDisplayNote(server: McpServer): string | null {
  return normalizeMcpMetadataText(server.user_note)
    ?? normalizeMcpMetadataText(server.description);
}

export function filterMcpServersBySearch(
  servers: McpServer[],
  searchText: string,
  getConfigSummary: (server: McpServer) => string,
): McpServer[] {
  const keyword = searchText.trim().toLowerCase();
  if (!keyword) {
    return servers;
  }

  return servers.filter((server) => {
    const searchableValues = [
      server.name,
      server.server_type,
      getConfigSummary(server),
      server.description,
      server.user_group,
      server.user_note,
    ];

    return searchableValues.some((value) => value?.toLowerCase().includes(keyword));
  });
}

export function buildMcpGroups(
  servers: McpServer[],
  labels: { groupUngrouped: string },
): McpGroup[] {
  const groupMap = new Map<string, McpGroup>();

  for (const server of servers) {
    const userGroup = normalizeMcpMetadataText(server.user_group);
    const key = userGroup ? `custom:${userGroup}` : CUSTOM_UNGROUPED_GROUP_KEY;
    const label = userGroup ?? labels.groupUngrouped;
    const existing = groupMap.get(key);

    if (existing) {
      existing.servers.push(server);
    } else {
      groupMap.set(key, {
        key,
        label,
        servers: [server],
      });
    }
  }

  return Array.from(groupMap.values());
}

export function isMcpUngroupedCustomGroup(group: McpGroup): boolean {
  return group.key === CUSTOM_UNGROUPED_GROUP_KEY;
}

export function getMcpGroupToolKeys(group: McpGroup): string[] {
  const toolKeys = new Set<string>();
  for (const server of group.servers) {
    for (const toolKey of server.enabled_tools) {
      toolKeys.add(toolKey);
    }
  }
  return [...toolKeys];
}

export function isMcpGroupToolsAligned(group: McpGroup): boolean {
  if (group.servers.length <= 1) {
    return true;
  }

  const [firstServer, ...restServers] = group.servers;
  const firstToolKey = createToolSetKey(firstServer.enabled_tools);
  return restServers.every((server) => createToolSetKey(server.enabled_tools) === firstToolKey);
}

export function getMcpServerIdsMissingTool(group: McpGroup, toolKey: string): string[] {
  return group.servers
    .filter((server) => !server.enabled_tools.includes(toolKey))
    .map((server) => server.id);
}

export function getMcpServerIdsWithTool(group: McpGroup, toolKey: string): string[] {
  return group.servers
    .filter((server) => server.enabled_tools.includes(toolKey))
    .map((server) => server.id);
}

function createToolSetKey(toolKeys: string[]): string {
  return [...new Set(toolKeys)].sort().join('\u0000');
}
