import React from 'react';
import { Collapse, Empty, Dropdown, Tooltip } from 'antd';
import { PlusOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { McpGroup, McpServer, McpTool } from '../types';
import { getMcpGroupToolKeys, isMcpUngroupedCustomGroup } from '../utils/mcpGrouping';
import { McpCard } from './McpCard';
import styles from './McpGroupedList.module.less';

interface McpGroupedListProps {
  groups: McpGroup[];
  tools: McpTool[];
  loading: boolean;
  activeKeys: string[];
  onActiveKeysChange: (keys: string[]) => void;
  onEdit: (server: McpServer) => void;
  onEditMetadata: (server: McpServer) => void;
  onDelete: (serverId: string) => void;
  onToggleTool: (serverId: string, toolKey: string) => void;
  groupToolMode?: boolean;
  onAddGroupTool?: (group: McpGroup, toolKey: string) => void;
  onRemoveGroupTool?: (group: McpGroup, toolKey: string) => void;
}

export const McpGroupedList: React.FC<McpGroupedListProps> = ({
  groups,
  tools,
  loading,
  activeKeys,
  onActiveKeysChange,
  onEdit,
  onEditMetadata,
  onDelete,
  onToggleTool,
  groupToolMode = false,
  onAddGroupTool,
  onRemoveGroupTool,
}) => {
  const { t } = useTranslation();

  if (groups.length === 0) {
    return (
      <div className={styles.empty}>
        <Empty description={t('mcp.noServers')} />
      </div>
    );
  }

  const renderGroupTools = (group: McpGroup) => {
    const activeToolKeys = new Set(getMcpGroupToolKeys(group));
    const activeTools = tools.filter((tool) => activeToolKeys.has(tool.key));
    const availableTools = tools.filter((tool) => tool.installed && !activeToolKeys.has(tool.key));

    return (
      <div className={styles.groupTools} onClick={(e) => e.stopPropagation()}>
        {activeTools.map((tool) => (
          <Tooltip
            key={tool.key}
            title={t('mcp.groupTools.removeTool', { tool: tool.display_name })}
          >
            <button
              type="button"
              className={styles.groupToolPill}
              disabled={loading}
              onClick={() => onRemoveGroupTool?.(group, tool.key)}
            >
              <span className={styles.statusBadge} />
              {tool.display_name}
            </button>
          </Tooltip>
        ))}
        {availableTools.length > 0 && (
          <Dropdown
            menu={{
              items: availableTools.map((tool) => ({
                key: tool.key,
                label: tool.display_name,
                onClick: () => onAddGroupTool?.(group, tool.key),
              })),
            }}
            trigger={['click']}
            disabled={loading}
          >
            <button type="button" className={styles.groupToolAdd} disabled={loading}>
              <PlusOutlined />
            </button>
          </Dropdown>
        )}
      </div>
    );
  };

  const items = groups.map((group) => {
    const groupToolsEnabled = groupToolMode && !isMcpUngroupedCustomGroup(group);

    return {
      key: group.key,
      label: (
        <div className={styles.groupHeader}>
          <div className={styles.groupTitle}>
            <span className={styles.groupLabel}>
              {group.label}
              <span className={styles.groupCount}>
                ({t('mcp.serverCount', { count: group.servers.length })})
              </span>
            </span>
          </div>
          {groupToolsEnabled && renderGroupTools(group)}
        </div>
      ),
      children: (
        <div className={styles.groupGrid}>
          {group.servers.map((server) => (
            <McpCard
              key={server.id}
              server={server}
              tools={tools}
              loading={loading}
              dragDisabled
              toolsReadOnly={groupToolsEnabled}
              onEdit={onEdit}
              onEditMetadata={onEditMetadata}
              onDelete={onDelete}
              onToggleTool={onToggleTool}
            />
          ))}
        </div>
      ),
    };
  });

  return (
    <div className={styles.groupedList}>
      <Collapse
        activeKey={activeKeys}
        onChange={(keys) => onActiveKeysChange(keys as string[])}
        items={items}
      />
    </div>
  );
};

export default McpGroupedList;
