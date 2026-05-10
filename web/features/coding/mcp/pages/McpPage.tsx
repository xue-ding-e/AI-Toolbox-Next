import React, { useState, useCallback } from 'react';
import { Typography, Button, Space, Modal, Tooltip, Input, Segmented, message } from 'antd';
import {
  PlusOutlined,
  EllipsisOutlined,
  ImportOutlined,
  FileTextOutlined,
  LinkOutlined,
  DragOutlined,
  AppstoreOutlined,
  BarsOutlined,
  DownOutlined,
  UpOutlined,
  ToolOutlined,
} from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { openUrl } from '@tauri-apps/plugin-opener';
import { arrayMove } from '@dnd-kit/sortable';
import type { DragEndEvent } from '@dnd-kit/core';
import { useMcp } from '../hooks/useMcp';
import { useMcpActions } from '../hooks/useMcpActions';
import { useMcpTools } from '../hooks/useMcpTools';
import { useMcpStore } from '../stores/mcpStore';
import { McpList } from '../components/McpList';
import { McpGroupedList } from '../components/McpGroupedList';
import { AddMcpModal } from '../components/modals/AddMcpModal';
import { McpSettingsModal } from '../components/modals/McpSettingsModal';
import { ImportMcpModal } from '../components/modals/ImportMcpModal';
import { ImportJsonModal } from '../components/modals/ImportJsonModal';
import { McpMetadataModal } from '../components/modals/McpMetadataModal';
import * as mcpApi from '../services/mcpApi';
import {
  buildMcpGroups,
  filterMcpServersBySearch,
  getMcpGroupToolKeys,
  getMcpGroupOptions,
  getMcpServerIdsMissingTool,
  getMcpServerIdsWithTool,
  isMcpGroupToolsAligned,
  isMcpUngroupedCustomGroup,
} from '../utils/mcpGrouping';
import type { McpGroup, McpServer, CreateMcpServerInput, UpdateMcpServerInput } from '../types';
import styles from './McpPage.module.less';

const { Title, Text, Link } = Typography;
const AUTO_EXPAND_MCP_THRESHOLD = 20;

function getMcpConfigSummary(server: McpServer): string {
  if (server.server_type === 'stdio') {
    const config = server.server_config as { command?: string };
    return config.command || 'stdio';
  }

  const config = server.server_config as { url?: string };
  return config.url || 'http';
}

const McpPage: React.FC = () => {
  const { t } = useTranslation();
  const { servers, loading, refresh } = useMcp();
  const { tools } = useMcpTools();
  const { setServers, isSettingsModalOpen, setSettingsModalOpen, isImportModalOpen, setImportModalOpen, isImportJsonModalOpen, setImportJsonModalOpen, loadScanResult } = useMcpStore();
  const {
    createServer,
    editServer,
    deleteServer,
    toggleTool,
    reorderServers,
    syncAll,
  } = useMcpActions();

  const [isAddModalOpen, setAddModalOpen] = useState(false);
  const [editingServer, setEditingServer] = useState<McpServer | null>(null);
  const [actionLoading, setActionLoading] = useState(false);
  const [reorderMode, setReorderMode] = useState(false);
  const [searchText, setSearchText] = useState('');
  const [viewMode, setViewMode] = useState<'flat' | 'grouped'>('flat');
  const [groupActiveKeys, setGroupActiveKeys] = useState<string[]>([]);
  const [metadataServer, setMetadataServer] = useState<McpServer | null>(null);
  const [groupToolMode, setGroupToolMode] = useState(false);
  const previousViewModeRef = React.useRef<'flat' | 'grouped'>('flat');
  const previousAutoExpandRef = React.useRef(false);

  const filteredServers = React.useMemo(() => {
    return filterMcpServersBySearch(servers, searchText, getMcpConfigSummary);
  }, [servers, searchText]);

  const isSearchActive = !!searchText.trim();
  const isFlatReorderEnabled = viewMode === 'flat' && reorderMode && !isSearchActive;
  const canUseGroupToolMode = viewMode === 'grouped' && !isSearchActive;
  const groupOptions = React.useMemo(() => getMcpGroupOptions(servers), [servers]);
  const groupedServers = React.useMemo<McpGroup[]>(() => {
    if (viewMode !== 'grouped') return [];

    return buildMcpGroups(filteredServers, {
      groupUngrouped: t('mcp.groupUngrouped'),
    });
  }, [filteredServers, t, viewMode]);

  React.useEffect(() => {
    if (viewMode !== 'flat' || isSearchActive) {
      setReorderMode(false);
    }
  }, [isSearchActive, viewMode]);

  React.useEffect(() => {
    if (!canUseGroupToolMode) {
      setGroupToolMode(false);
    }
  }, [canUseGroupToolMode]);

  const shouldAutoExpandGroups =
    filteredServers.length > 0 && filteredServers.length < AUTO_EXPAND_MCP_THRESHOLD;

  React.useEffect(() => {
    if (viewMode !== 'grouped') {
      previousViewModeRef.current = viewMode;
      previousAutoExpandRef.current = false;
      return;
    }

    const enteredGroupedView = previousViewModeRef.current !== 'grouped';
    const autoExpandChanged = previousAutoExpandRef.current !== shouldAutoExpandGroups;
    previousViewModeRef.current = viewMode;
    previousAutoExpandRef.current = shouldAutoExpandGroups;
    if (!enteredGroupedView && !autoExpandChanged) {
      return;
    }

    if (shouldAutoExpandGroups) {
      setGroupActiveKeys(groupedServers.map((group) => group.key));
      return;
    }

    setGroupActiveKeys([]);
  }, [groupedServers, shouldAutoExpandGroups, viewMode]);

  React.useEffect(() => {
    if (viewMode !== 'grouped') {
      return;
    }

    const validGroupKeys = new Set(groupedServers.map((group) => group.key));
    setGroupActiveKeys((previousKeys) => {
      const nextKeys = previousKeys.filter((key) => validGroupKeys.has(key));
      return nextKeys.length === previousKeys.length ? previousKeys : nextKeys;
    });
  }, [groupedServers, viewMode]);

  const groupToolTargetGroups = React.useMemo(
    () => groupedServers.filter((group) => !isMcpUngroupedCustomGroup(group)),
    [groupedServers],
  );

  const groupsNeedingToolNormalization = React.useMemo(
    () => groupToolTargetGroups.filter((group) => !isMcpGroupToolsAligned(group)),
    [groupToolTargetGroups],
  );

  const getToolLabel = React.useCallback((toolKey: string) => {
    return tools.find((tool) => tool.key === toolKey)?.display_name ?? toolKey;
  }, [tools]);

  const applyMcpToolState = React.useCallback(async (
    serverIds: string[],
    toolKey: string,
    enabled: boolean,
    quiet = false,
  ) => {
    if (serverIds.length === 0) {
      return true;
    }

    setActionLoading(true);
    try {
      for (const serverId of serverIds) {
        await mcpApi.toggleMcpTool(serverId, toolKey);
      }
      await refresh();
      if (!quiet) {
        message.success(t(
          enabled ? 'mcp.groupTools.addSuccess' : 'mcp.groupTools.removeSuccess',
          { count: serverIds.length, tool: getToolLabel(toolKey) },
        ));
      }
      return true;
    } catch (error) {
      message.error(t('mcp.toggleToolFailed') + ': ' + String(error));
      await refresh();
      return false;
    } finally {
      setActionLoading(false);
    }
  }, [getToolLabel, refresh, t]);

  const normalizeMcpGroupTools = React.useCallback(async () => {
    let updatedCount = 0;
    for (const group of groupToolTargetGroups) {
      for (const toolKey of getMcpGroupToolKeys(group)) {
        const missingServerIds = getMcpServerIdsMissingTool(group, toolKey);
        if (missingServerIds.length === 0) {
          continue;
        }

        const saved = await applyMcpToolState(missingServerIds, toolKey, true, true);
        if (!saved) {
          return false;
        }
        updatedCount += missingServerIds.length;
      }
    }

    if (updatedCount > 0) {
      message.success(t('mcp.groupTools.normalizedSuccess', { count: updatedCount }));
    }
    return true;
  }, [applyMcpToolState, groupToolTargetGroups, t]);

  const handleToggleGroupToolMode = React.useCallback((nextEnabled: boolean) => {
    if (!nextEnabled) {
      setGroupToolMode(false);
      return;
    }

    if (!canUseGroupToolMode) {
      return;
    }

    if (groupsNeedingToolNormalization.length === 0) {
      setGroupToolMode(true);
      return;
    }

    Modal.confirm({
      title: t('mcp.groupTools.confirmTitle'),
      content: t('mcp.groupTools.confirmContent', {
        count: groupsNeedingToolNormalization.length,
      }),
      okText: t('mcp.groupTools.confirmOk'),
      cancelText: t('common.cancel'),
      onOk: async () => {
        const normalized = await normalizeMcpGroupTools();
        if (normalized) {
          setGroupToolMode(true);
        }
      },
    });
  }, [canUseGroupToolMode, groupsNeedingToolNormalization.length, normalizeMcpGroupTools, t]);

  const handleAddGroupTool = React.useCallback(async (group: McpGroup, toolKey: string) => {
    if (isMcpUngroupedCustomGroup(group)) {
      return;
    }

    const missingServerIds = getMcpServerIdsMissingTool(group, toolKey);
    await applyMcpToolState(missingServerIds, toolKey, true);
  }, [applyMcpToolState]);

  const handleRemoveGroupTool = React.useCallback(async (group: McpGroup, toolKey: string) => {
    if (isMcpUngroupedCustomGroup(group)) {
      return;
    }

    const enabledServerIds = getMcpServerIdsWithTool(group, toolKey);
    await applyMcpToolState(enabledServerIds, toolKey, false);
  }, [applyMcpToolState]);

  const handleAddServer = async (input: CreateMcpServerInput) => {
    setActionLoading(true);
    try {
      await createServer(input);
      setAddModalOpen(false);
    } finally {
      setActionLoading(false);
    }
  };

  const handleUpdateServer = async (serverId: string, input: UpdateMcpServerInput) => {
    setActionLoading(true);
    try {
      await editServer(serverId, input);
      setEditingServer(null);
      setAddModalOpen(false);
    } finally {
      setActionLoading(false);
    }
  };

  const handleEdit = (server: McpServer) => {
    setEditingServer(server);
    setAddModalOpen(true);
  };

  const handleCloseModal = () => {
    setAddModalOpen(false);
    setEditingServer(null);
  };

  const handleDelete = (serverId: string) => {
    const serverToDelete = servers.find((s) => s.id === serverId);
    Modal.confirm({
      title: t('mcp.deleteConfirm'),
      content: t('mcp.deleteConfirmContent', { name: serverToDelete?.name }),
      okText: t('common.delete'),
      okType: 'danger',
      cancelText: t('common.cancel'),
      onOk: async () => {
        setActionLoading(true);
        try {
          await deleteServer(serverId);
        } finally {
          setActionLoading(false);
        }
      },
    });
  };

  const handleToggleTool = async (serverId: string, toolKey: string) => {
    setActionLoading(true);
    try {
      await toggleTool(serverId, toolKey);
    } finally {
      setActionLoading(false);
    }
  };

  const handleDragEnd = useCallback(
    async (event: DragEndEvent) => {
      const { active, over } = event;
      if (!over || active.id === over.id) return;

      const oldIndex = servers.findIndex((s) => s.id === active.id);
      const newIndex = servers.findIndex((s) => s.id === over.id);

      if (oldIndex !== -1 && newIndex !== -1) {
        const newServers = arrayMove(servers, oldIndex, newIndex);
        setServers(newServers);
        const ids = newServers.map((s) => s.id);
        await reorderServers(ids);
      }
    },
    [servers, setServers, reorderServers]
  );

  return (
    <div className={styles.mcpPage}>
      <div className={styles.pageHeader}>
        <div>
          <Title level={4} style={{ margin: 0, display: 'inline-block', marginRight: 8 }}>
            {t('mcp.title')}
          </Title>
          <Link
            type="secondary"
            style={{ fontSize: 12 }}
            onClick={(e) => {
              e.stopPropagation();
              openUrl('https://code.claude.com/docs/en/mcp#installing-mcp-servers');
            }}
          >
            <LinkOutlined /> {t('mcp.viewDocs')}
          </Link>
        </div>
        <Button
          type="text"
          icon={<EllipsisOutlined />}
          onClick={() => setSettingsModalOpen(true)}
        >
          {t('mcp.settings')}
        </Button>
      </div>

      <Text type="secondary" style={{ fontSize: 12, marginBottom: 16, marginTop: -16 }}>
        {t('mcp.pageHint')}
      </Text>

      <div className={styles.toolbar}>
        <Space size={8}>
          <Input.Search
            placeholder={t('mcp.searchPlaceholder')}
            allowClear
            style={{ width: 200 }}
            value={searchText}
            onChange={(e) => setSearchText(e.target.value)}
          />
          <Button
            type="text"
            icon={<ImportOutlined />}
            onClick={() => setImportModalOpen(true)}
            style={{ color: 'var(--color-text-tertiary)' }}
          >
            {t('mcp.importExisting')}
          </Button>
          <Button
            type="text"
            icon={<FileTextOutlined />}
            onClick={() => setImportJsonModalOpen(true)}
            style={{ color: 'var(--color-text-tertiary)' }}
          >
            {t('mcp.importJson.button')}
          </Button>
          <Button
            type="link"
            icon={<PlusOutlined />}
            onClick={() => setAddModalOpen(true)}
          >
            {t('mcp.addServer')}
          </Button>
        </Space>
        <Space size={4}>
          {viewMode === 'flat' && (
            <Tooltip
              title={
                isSearchActive
                  ? t('mcp.reorderDisabledWhileSearching')
                  : t('mcp.reorderHint')
              }
            >
              <Button
                type={reorderMode ? 'primary' : 'text'}
                size="small"
                icon={<DragOutlined />}
                className={styles.reorderButton}
                onClick={() => setReorderMode((prev) => !prev)}
                disabled={loading || actionLoading || isSearchActive}
              >
                {t('mcp.reorder')}
              </Button>
            </Tooltip>
          )}
          {viewMode === 'grouped' && (
            <>
              <Tooltip title={t('mcp.expandAll')}>
                <Button
                  type="text"
                  size="small"
                  icon={<DownOutlined />}
                  onClick={() => setGroupActiveKeys(groupedServers.map((g) => g.key))}
                />
              </Tooltip>
              <Tooltip title={t('mcp.collapseAll')}>
                <Button
                  type="text"
                  size="small"
                  icon={<UpOutlined />}
                  onClick={() => setGroupActiveKeys([])}
                />
              </Tooltip>
            </>
          )}
          {viewMode === 'grouped' && (
            <Tooltip
              title={
                isSearchActive
                  ? t('mcp.groupTools.disabledWhileSearching')
                  : t('mcp.groupTools.tip')
              }
            >
              <Button
                type={groupToolMode ? 'primary' : 'text'}
                size="small"
                icon={<ToolOutlined />}
                disabled={loading || actionLoading || isSearchActive}
                onClick={() => handleToggleGroupToolMode(!groupToolMode)}
              >
                {t('mcp.groupTools.mode')}
              </Button>
            </Tooltip>
          )}
          <Tooltip title={t('mcp.groupedViewTip')}>
            <Segmented
              size="small"
              value={viewMode}
              onChange={(v) => setViewMode(v as 'flat' | 'grouped')}
              options={[
                { value: 'flat', icon: <AppstoreOutlined />, label: t('mcp.viewFlat') },
                { value: 'grouped', icon: <BarsOutlined />, label: t('mcp.viewGrouped') },
              ]}
            />
          </Tooltip>
        </Space>
      </div>

      <div className={styles.content}>
        {viewMode === 'flat' ? (
          <McpList
            servers={filteredServers}
            tools={tools}
            loading={loading || actionLoading}
            dragDisabled={!isFlatReorderEnabled}
            onEdit={handleEdit}
            onEditMetadata={setMetadataServer}
            onDelete={handleDelete}
            onToggleTool={handleToggleTool}
            onDragEnd={handleDragEnd}
          />
        ) : (
          <McpGroupedList
            groups={groupedServers}
            tools={tools}
            loading={loading || actionLoading}
            activeKeys={groupActiveKeys}
            onActiveKeysChange={setGroupActiveKeys}
            onEdit={handleEdit}
            onEditMetadata={setMetadataServer}
            onDelete={handleDelete}
            onToggleTool={handleToggleTool}
            groupToolMode={groupToolMode}
            onAddGroupTool={handleAddGroupTool}
            onRemoveGroupTool={handleRemoveGroupTool}
          />
        )}
      </div>

      {isAddModalOpen && (
        <AddMcpModal
          open={isAddModalOpen}
          tools={tools}
          servers={servers}
          editingServer={editingServer}
          onClose={handleCloseModal}
          onSubmit={handleAddServer}
          onUpdate={handleUpdateServer}
          onSyncAll={syncAll}
        />
      )}

      {isSettingsModalOpen && (
        <McpSettingsModal
          open={isSettingsModalOpen}
          onClose={() => setSettingsModalOpen(false)}
        />
      )}

      {isImportModalOpen && (
        <ImportMcpModal
          open={isImportModalOpen}
          onClose={() => setImportModalOpen(false)}
          onSuccess={() => {
            setImportModalOpen(false);
            loadScanResult();
          }}
        />
      )}

      {isImportJsonModalOpen && (
        <ImportJsonModal
          open={isImportJsonModalOpen}
          servers={servers}
          onClose={() => setImportJsonModalOpen(false)}
          onSuccess={() => {
            setImportJsonModalOpen(false);
            loadScanResult();
          }}
          onSyncAll={syncAll}
        />
      )}

      <McpMetadataModal
        open={!!metadataServer}
        server={metadataServer}
        groupOptions={groupOptions}
        onClose={() => setMetadataServer(null)}
        onSuccess={() => {
          setMetadataServer(null);
          refresh();
        }}
      />
    </div>
  );
};

export default McpPage;
