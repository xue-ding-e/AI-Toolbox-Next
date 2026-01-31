import React from 'react';
import { Button, Tooltip, Dropdown, Tag } from 'antd';
import {
  DeleteOutlined,
  EditOutlined,
  PlusOutlined,
  HolderOutlined,
  CodeOutlined,
  GlobalOutlined,
} from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { McpServer, McpTool } from '../types';
import styles from './McpCard.module.less';

interface McpCardProps {
  server: McpServer;
  tools: McpTool[];
  loading: boolean;
  onEdit: (server: McpServer) => void;
  onDelete: (serverId: string) => void;
  onToggleTool: (serverId: string, toolKey: string) => void;
}

export const McpCard: React.FC<McpCardProps> = ({
  server,
  tools,
  loading,
  onEdit,
  onDelete,
  onToggleTool,
}) => {
  const { t } = useTranslation();

  // Drag-and-drop sortable
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: server.id });

  const sortableStyle = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
  };

  // Type icon
  const iconNode = server.server_type === 'stdio' ? (
    <CodeOutlined className={styles.icon} />
  ) : (
    <GlobalOutlined className={styles.icon} />
  );

  // Get server config summary
  const getConfigSummary = () => {
    if (server.server_type === 'stdio') {
      const config = server.server_config as { command?: string };
      return config.command || 'stdio';
    } else {
      const config = server.server_config as { url?: string };
      return config.url || 'http';
    }
  };

  // Enabled tools vs available tools
  const enabledToolKeys = new Set(server.enabled_tools);
  const enabledTools = tools.filter((t) => enabledToolKeys.has(t.key));
  const availableTools = tools.filter((t) => !enabledToolKeys.has(t.key));

  // Sort available tools: installed first
  const sortedAvailableTools = [...availableTools].sort((a, b) => {
    if (a.installed === b.installed) return 0;
    return a.installed ? -1 : 1;
  });

  const dropdownItems = sortedAvailableTools.map((tool) => ({
    key: tool.key,
    label: (
      <span>
        {tool.display_name}
        {!tool.installed && (
          <span className={styles.notInstalledTag}>{t('mcp.notInstalled')}</span>
        )}
      </span>
    ),
    onClick: () => onToggleTool(server.id, tool.key),
  }));

  return (
    <div ref={setNodeRef} style={sortableStyle}>
      <div className={styles.card}>
        <div
          className={styles.dragHandle}
          {...attributes}
          {...listeners}
        >
          <HolderOutlined />
        </div>
        <div className={styles.iconArea}>{iconNode}</div>
        <div className={styles.main}>
          <div className={styles.headerRow}>
            <div className={styles.name}>{server.name}</div>
            <Tag className={styles.typeTag}>{server.server_type}</Tag>
            <span className={styles.configSummary}>{getConfigSummary()}</span>
          </div>
          {server.description && (
            <div className={styles.description}>{server.description}</div>
          )}
          <div className={styles.toolMatrix}>
            {enabledTools.map((tool) => {
              const syncDetail = server.sync_details.find((d) => d.tool === tool.key);
              const status = syncDetail?.status || 'pending';
              return (
                <Tooltip
                  key={`${server.id}-${tool.key}`}
                  title={`${tool.display_name} - ${status}`}
                >
                  <button
                    type="button"
                    className={`${styles.toolPill} ${styles.active} ${status === 'error' ? styles.error : ''}`}
                    onClick={() => onToggleTool(server.id, tool.key)}
                  >
                    <span className={`${styles.statusBadge} ${styles[status]}`} />
                    {tool.display_name}
                  </button>
                </Tooltip>
              );
            })}
            {dropdownItems.length > 0 && (
              <Dropdown
                menu={{ items: dropdownItems }}
                trigger={['click']}
                disabled={loading}
              >
                <button type="button" className={styles.addToolBtn}>
                  <PlusOutlined />
                </button>
              </Dropdown>
            )}
          </div>
        </div>
        <div className={styles.actions}>
          <Button
            type="text"
            icon={<EditOutlined />}
            onClick={() => onEdit(server)}
            disabled={loading}
            title={t('mcp.edit')}
          />
          <Button
            type="text"
            danger
            icon={<DeleteOutlined />}
            onClick={() => onDelete(server.id)}
            disabled={loading}
            title={t('mcp.delete')}
          />
        </div>
      </div>
    </div>
  );
};

export default McpCard;
