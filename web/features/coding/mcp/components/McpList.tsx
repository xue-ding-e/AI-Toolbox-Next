import React from 'react';
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
import { restrictToWindowEdges } from '@dnd-kit/modifiers';
import { ManagementEmpty, ManagementLoading, VirtualGrid } from '@/features/coding/shared/management';
import type { DragEndEvent } from '@dnd-kit/core';
import type { McpServer, McpTool } from '../types';
import { McpCard } from './McpCard';
import styles from './McpList.module.less';

interface McpListProps {
  servers: McpServer[];
  tools: McpTool[];
  loading: boolean;
  columns?: number;
  dragDisabled?: boolean;
  onEdit: (server: McpServer) => void;
  onEditMetadata: (server: McpServer) => void;
  onDelete: (serverId: string) => void;
  onToggleTool: (serverId: string, toolKey: string) => void;
  onDragEnd: (event: DragEndEvent) => void;
}

export const McpList: React.FC<McpListProps> = ({
  servers,
  tools,
  loading,
  columns,
  dragDisabled,
  onEdit,
  onEditMetadata,
  onDelete,
  onToggleTool,
  onDragEnd,
}) => {
  const { t } = useTranslation();

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8,
      },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    })
  );

  if (loading && servers.length === 0) {
    return (
      <div className={styles.loading}>
        <ManagementLoading label={t('common.loading')} />
      </div>
    );
  }

  if (servers.length === 0) {
    return (
      <div className={styles.empty}>
        <ManagementEmpty description={t('mcp.noServers')} />
      </div>
    );
  }

  const cardList = (
    <div
      className={[
        styles.list,
        columns === undefined ? styles.listAuto : styles.listFixed,
      ].filter(Boolean).join(' ')}
      style={columns === undefined ? undefined : ({
        '--management-grid-columns': `repeat(${columns}, minmax(0, 1fr))`,
      } as React.CSSProperties)}
    >
      {servers.map((server) => (
        <McpCard
          key={server.id}
          server={server}
          tools={tools}
          loading={loading}
          dragDisabled={dragDisabled}
          onEdit={onEdit}
          onEditMetadata={onEditMetadata}
          onDelete={onDelete}
          onToggleTool={onToggleTool}
        />
      ))}
    </div>
  );

  if (dragDisabled) {
    return (
      <VirtualGrid
        items={servers}
        getKey={(server) => server.id}
        columns={columns}
        defaultRowHeight={78}
        renderItem={(server) => (
          <McpCard
            server={server}
            tools={tools}
            loading={loading}
            dragDisabled
            onEdit={onEdit}
            onEditMetadata={onEditMetadata}
            onDelete={onDelete}
            onToggleTool={onToggleTool}
          />
        )}
      />
    );
  }

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      modifiers={[restrictToWindowEdges]}
      onDragEnd={onDragEnd}
    >
      <SortableContext
        items={servers.map((s) => s.id)}
        strategy={rectSortingStrategy}
      >
        {cardList}
      </SortableContext>
    </DndContext>
  );
};

export default McpList;
