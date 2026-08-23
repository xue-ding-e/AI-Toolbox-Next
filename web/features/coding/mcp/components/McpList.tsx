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
import { ManagementEmpty, ManagementLoading, useAutoGridColumns, VirtualGrid } from '@/features/coding/shared/management';
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
  resolvedPackageVersions?: Record<string, string>;
  preferredToolKeysForAddMore?: string[];
  limitAddMoreToPreferredTools?: boolean;
  onOpenDetail?: (server: McpServer) => void;
  onEdit: (server: McpServer) => void;
  onEditMetadata: (server: McpServer) => void;
  onDelete: (serverId: string) => void;
  onToggleTool: (serverId: string, toolKey: string) => void;
  onSetManagementEnabled?: (server: McpServer, enabled: boolean) => void;
  onRefresh?: () => void;
  onDragEnd: (event: DragEndEvent) => void;
}

export const McpList: React.FC<McpListProps> = ({
  servers,
  tools,
  loading,
  columns,
  dragDisabled,
  resolvedPackageVersions,
  preferredToolKeysForAddMore,
  limitAddMoreToPreferredTools,
  onOpenDetail,
  onEdit,
  onEditMetadata,
  onDelete,
  onToggleTool,
  onSetManagementEnabled,
  onRefresh,
  onDragEnd,
}) => {
  const { t } = useTranslation();

  // Measure the non-virtualized grid container width when on "auto" columns so
  // drag-sort mode renders the same adaptive column count as browse mode.
  // Disabled when the caller forces a fixed column count.
  const { containerRef, columnCount: autoColumnCount } = useAutoGridColumns<HTMLDivElement>({
    minColumnWidth: 350,
    maxColumns: 3,
    gap: 10,
    enabled: columns === undefined,
  });
  const effectiveColumns = columns === undefined ? autoColumnCount : columns;

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
      ref={containerRef}
      className={[
        styles.list,
        columns === undefined ? styles.listAuto : styles.listFixed,
      ].filter(Boolean).join(' ')}
      style={effectiveColumns === undefined ? undefined : ({
        '--management-grid-columns': `repeat(${effectiveColumns}, minmax(0, 1fr))`,
      } as React.CSSProperties)}
    >
      {servers.map((server) => (
        <McpCard
          key={server.id}
          server={server}
          tools={tools}
          loading={loading}
          dragDisabled={dragDisabled}
          resolvedPackageVersions={resolvedPackageVersions}
          preferredToolKeysForAddMore={preferredToolKeysForAddMore}
          limitAddMoreToPreferredTools={limitAddMoreToPreferredTools}
          onOpenDetail={onOpenDetail}
          onEdit={onEdit}
          onEditMetadata={onEditMetadata}
          onDelete={onDelete}
          onToggleTool={onToggleTool}
          onSetManagementEnabled={onSetManagementEnabled}
          onRefresh={onRefresh}
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
        minColumnWidth={350}
        maxColumns={3}
        defaultRowHeight={78}
        renderItem={(server) => (
          <McpCard
            server={server}
            tools={tools}
            loading={loading}
            dragDisabled
            resolvedPackageVersions={resolvedPackageVersions}
            preferredToolKeysForAddMore={preferredToolKeysForAddMore}
            limitAddMoreToPreferredTools={limitAddMoreToPreferredTools}
            onOpenDetail={onOpenDetail}
            onEdit={onEdit}
            onEditMetadata={onEditMetadata}
            onDelete={onDelete}
            onToggleTool={onToggleTool}
            onSetManagementEnabled={onSetManagementEnabled}
            onRefresh={onRefresh}
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
