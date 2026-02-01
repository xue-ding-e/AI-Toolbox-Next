import React from 'react';
import { Empty, Spin } from 'antd';
import { useTranslation } from 'react-i18next';
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
} from '@dnd-kit/core';
import {
  SortableContext,
  sortableKeyboardCoordinates,
  rectSortingStrategy,
} from '@dnd-kit/sortable';
import type { DragEndEvent } from '@dnd-kit/core';
import type { McpServer, McpTool } from '../types';
import { McpCard } from './McpCard';
import styles from './McpList.module.less';

interface McpListProps {
  servers: McpServer[];
  tools: McpTool[];
  loading: boolean;
  onEdit: (server: McpServer) => void;
  onDelete: (serverId: string) => void;
  onToggleTool: (serverId: string, toolKey: string) => void;
  onDragEnd: (event: DragEndEvent) => void;
}

export const McpList: React.FC<McpListProps> = ({
  servers,
  tools,
  loading,
  onEdit,
  onDelete,
  onToggleTool,
  onDragEnd,
}) => {
  const { t } = useTranslation();

  const sensors = useSensors(
    useSensor(PointerSensor),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    })
  );

  if (loading && servers.length === 0) {
    return (
      <div className={styles.loading}>
        <Spin />
      </div>
    );
  }

  if (servers.length === 0) {
    return (
      <div className={styles.empty}>
        <Empty description={t('mcp.noServers')} />
      </div>
    );
  }

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      onDragEnd={onDragEnd}
    >
      <SortableContext
        items={servers.map((s) => s.id)}
        strategy={rectSortingStrategy}
      >
        <div className={styles.list}>
          {servers.map((server) => (
            <McpCard
              key={server.id}
              server={server}
              tools={tools}
              loading={loading}
              onEdit={onEdit}
              onDelete={onDelete}
              onToggleTool={onToggleTool}
            />
          ))}
        </div>
      </SortableContext>
    </DndContext>
  );
};

export default McpList;
