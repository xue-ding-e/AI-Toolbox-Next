import React, { useState, useCallback } from 'react';
import { Drawer, Modal, Popover, message } from 'antd';
import {
  Check,
  ChevronRight,
  ChevronsDown,
  ChevronsUp,
  ExternalLink,
  FileJson,
  FileText,
  Folders,
  GripVertical,
  Import,
  LayoutGrid,
  ListTree,
  MinusCircle,
  MoreHorizontal,
  Plus,
  PlusCircle,
  Power,
  PowerOff,
  Search,
  SlidersHorizontal,
  Tags,
  Trash2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { openUrl } from '@tauri-apps/plugin-opener';
import { arrayMove } from '@dnd-kit/sortable';
import type { DragEndEvent } from '@dnd-kit/core';
import {
  ManagementButton,
  ManagementIconButton,
  ManagementMenu,
  ManagementSearchInput,
  ManagementSegmented,
  MANAGEMENT_GRID_COLUMN_OPTIONS,
  type ManagementGridColumnSetting,
  type ManagementMenuItem,
} from '@/features/coding/shared/management';
import { useMcp } from '../hooks/useMcp';
import { useMcpActions } from '../hooks/useMcpActions';
import { useMcpTools } from '../hooks/useMcpTools';
import { useMcpStore } from '../stores/mcpStore';
import { McpList } from '../components/McpList';
import { McpGroupedList } from '../components/McpGroupedList';
import { McpDetailPanel } from '../components/McpDetailPanel';
import { AddMcpModal } from '../components/modals/AddMcpModal';
import { McpSettingsModal } from '../components/modals/McpSettingsModal';
import { ImportMcpModal } from '../components/modals/ImportMcpModal';
import { ImportJsonModal } from '../components/modals/ImportJsonModal';
import { McpMetadataModal } from '../components/modals/McpMetadataModal';
import { McpGroupsModal } from '../components/modals/McpGroupsModal';
import { McpInventoryModal } from '../components/modals/McpInventoryModal';
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
  normalizeMcpMetadataText,
} from '../utils/mcpGrouping';
import {
  getMcpCommandPackageVersion,
  getMcpCommandPackageVersionKey,
} from '../utils/mcpCommandPackageVersion';
import {
  collectAllTags,
  matchesTagFilters,
  pruneStaleTagFilters,
  UNTAGGED_FILTER,
} from '../utils/mcpTags';
import type { McpGroup, McpGroupRecord, McpServer, CreateMcpServerInput, UpdateMcpServerInput } from '../types';
import styles from './McpPage.module.less';

const AUTO_EXPAND_MCP_THRESHOLD = 20;

function getMcpConfigSummary(server: McpServer): string {
  if (server.server_type === 'stdio') {
    const config = server.server_config as { command?: string };
    return config.command || 'stdio';
  }

  const config = server.server_config as { url?: string };
  return config.url || 'http';
}

interface ToolbarOptionsPopoverProps {
  title: string;
  active?: boolean;
  activeTitle?: string;
  children: React.ReactNode | ((controls: { close: () => void }) => React.ReactNode);
}

// Options popover built on antd Popover (click trigger, viewport-aware
// placement) so positioning, dismissal and layering stay consistent with the
// rest of the app; only the inner layout comes from module styles.
const ToolbarOptionsPopover: React.FC<ToolbarOptionsPopoverProps> = ({ title, active, activeTitle, children }) => {
  const [open, setOpen] = React.useState(false);
  const close = React.useCallback(() => setOpen(false), []);

  return (
    <Popover
      trigger="click"
      placement="bottomRight"
      arrow={false}
      open={open}
      onOpenChange={setOpen}
      content={(
        <div className={styles.toolbarOptionsPopover} role="dialog" aria-label={title}>
          {typeof children === 'function' ? children({ close }) : children}
        </div>
      )}
    >
      <span className={styles.toolbarOptionsHost}>
        <ManagementIconButton
          icon={<SlidersHorizontal size={14} aria-hidden="true" />}
          title={activeTitle ?? title}
          aria-haspopup="dialog"
          aria-expanded={open}
          aria-label={activeTitle ?? title}
          className={active ? styles.toolbarOptionsTriggerActive : undefined}
          controlSize="compact"
        />
      </span>
    </Popover>
  );
};

interface ToolbarActionItemProps {
  icon: React.ReactNode;
  title: string;
  description: string;
  onClick: () => void;
}

const ToolbarActionItem: React.FC<ToolbarActionItemProps> = ({ icon, title, description, onClick }) => (
  <button type="button" className={styles.toolbarActionItem} onClick={onClick}>
    <span className={styles.toolbarActionIcon} aria-hidden="true">{icon}</span>
    <span className={styles.toolbarActionContent}>
      <span className={styles.toolbarActionTitle}>{title}</span>
      <span className={styles.toolbarActionDescription}>{description}</span>
    </span>
    <ChevronRight size={15} className={styles.toolbarActionArrow} aria-hidden="true" />
  </button>
);

interface TagFilterOption {
  value: string;
  label: string;
  count: number;
}

// Faceted tag filter mirroring the Skills toolbar: dashed trigger with
// selected badges, opening a searchable multi-select option list. Selection
// stays AND; the untagged sentinel stays mutually exclusive with concrete tags.
const TagFilterDropdown: React.FC<{
  options: TagFilterOption[];
  selected: string[];
  onToggle: (value: string) => void;
  onClear: () => void;
}> = ({ options, selected, onToggle, onClear }) => {
  const { t } = useTranslation();
  const [open, setOpen] = React.useState(false);
  const [query, setQuery] = React.useState('');

  const selectedSet = React.useMemo(() => new Set(selected), [selected]);
  const selectedBadges = React.useMemo(
    () => options.filter((option) => selectedSet.has(option.value)),
    [options, selectedSet],
  );
  const visibleOptions = React.useMemo(() => {
    const keyword = query.trim().toLowerCase();
    if (!keyword) return options;
    return options.filter((option) => option.label.toLowerCase().includes(keyword));
  }, [options, query]);

  const handleOpenChange = (nextOpen: boolean) => {
    setOpen(nextOpen);
    if (!nextOpen) {
      setQuery('');
    }
  };

  return (
    <Popover
      trigger="click"
      placement="bottomLeft"
      arrow={false}
      open={open}
      onOpenChange={handleOpenChange}
      content={(
        <div className={styles.tagFilterPopover}>
          <div className={styles.tagFilterSearch}>
            <Search size={13} aria-hidden="true" />
            <input
              value={query}
              placeholder={t('mcp.tags.searchPlaceholder')}
              aria-label={t('mcp.tags.searchPlaceholder')}
              onChange={(event) => setQuery(event.target.value)}
            />
          </div>
          <div className={styles.tagFilterList} role="listbox" aria-multiselectable aria-label={t('mcp.tags.filterLabel')}>
            {visibleOptions.map((option) => {
              const isSelected = selectedSet.has(option.value);
              return (
                <button
                  key={option.value}
                  type="button"
                  role="option"
                  aria-selected={isSelected}
                  className={styles.tagFilterOption}
                  onClick={() => onToggle(option.value)}
                >
                  <span
                    className={`${styles.tagFilterCheckbox}${isSelected ? ` ${styles.tagFilterCheckboxChecked}` : ''}`}
                    aria-hidden="true"
                  >
                    {isSelected && <Check size={12} />}
                  </span>
                  <span className={styles.tagFilterOptionLabel}>{option.label}</span>
                  <span className={styles.tagFilterCount}>{option.count}</span>
                </button>
              );
            })}
            {visibleOptions.length === 0 && (
              <div className={styles.tagFilterEmpty}>{t('mcp.tags.noMatch')}</div>
            )}
            {selected.length > 0 && (
              <>
                {visibleOptions.length > 0 && <div className={styles.tagFilterSeparator} />}
                <button type="button" className={styles.tagFilterClear} onClick={onClear}>
                  {t('mcp.tags.clearFilter')}
                </button>
              </>
            )}
          </div>
        </div>
      )}
    >
      <button
        type="button"
        className={styles.tagFilterTrigger}
        aria-haspopup="dialog"
        aria-expanded={open}
        title={t('mcp.tags.filterLabel')}
      >
        <PlusCircle size={14} aria-hidden="true" />
        <span>{t('mcp.tags.filterLabel')}</span>
        {selectedBadges.length > 0 && (
          <>
            <span className={styles.tagFilterDivider} aria-hidden="true" />
            {selectedBadges.length > 2
              ? (
                <span className={styles.tagFilterBadge}>
                  {t('mcp.tags.selectedCount', { count: selectedBadges.length })}
                </span>
              )
              : selectedBadges.map((badge) => (
                <span key={badge.value} className={styles.tagFilterBadge}>{badge.label}</span>
              ))}
          </>
        )}
      </button>
    </Popover>
  );
};

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
    disableServer,
    enableServer,
    restoreTools,
    batchSetManagementEnabled,
  } = useMcpActions();

  const [isAddModalOpen, setAddModalOpen] = useState(false);
  const [editingServer, setEditingServer] = useState<McpServer | null>(null);
  const [actionLoading, setActionLoading] = useState(false);
  const [reorderMode, setReorderMode] = useState(false);
  const [searchText, setSearchText] = useState('');
  const [viewMode, setViewMode] = useState<'flat' | 'grouped'>('flat');
  const [groupActiveKeys, setGroupActiveKeys] = useState<string[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [selectionMode, setSelectionMode] = useState(false);
  const [metadataServer, setMetadataServer] = useState<McpServer | null>(null);
  const [detailServerId, setDetailServerId] = useState<string | null>(null);
  const [groupsModalOpen, setGroupsModalOpen] = useState(false);
  const [inventoryModalOpen, setInventoryModalOpen] = useState(false);
  const [managedGroups, setManagedGroups] = useState<McpGroupRecord[]>([]);
  const [batchGroupModalOpen, setBatchGroupModalOpen] = useState(false);
  const [batchGroupValue, setBatchGroupValue] = useState('');
  const [groupToolMode, setGroupToolMode] = useState(false);
  const [gridColumnSetting, setGridColumnSetting] = useState<ManagementGridColumnSetting>('auto');
  const [resolvedPackageVersions, setResolvedPackageVersions] = useState<Record<string, string>>({});
  const [preferredToolsForAddMore, setPreferredToolsForAddMore] = useState<string[]>([]);
  const [limitAddMoreToPreferredTools, setLimitAddMoreToPreferredTools] = useState(false);
  const [tagFilter, setTagFilter] = React.useState<string[]>([]);
  const [enabledFilter, setEnabledFilter] = useState<'all' | 'enabled' | 'disabled'>('all');
  const deferredSearchText = React.useDeferredValue(searchText);
  const previousViewModeRef = React.useRef<'flat' | 'grouped'>('flat');
  const previousAutoExpandRef = React.useRef(false);

  const loadMcpToolMenuPreferences = React.useCallback(async () => {
    try {
      const [savedPreferredTools, savedLimitAddMoreToPreferredTools] = await Promise.all([
        mcpApi.getMcpPreferredTools(),
        mcpApi.getMcpLimitAddMoreToPreferredTools(),
      ]);
      setPreferredToolsForAddMore(savedPreferredTools);
      setLimitAddMoreToPreferredTools(savedLimitAddMoreToPreferredTools);
    } catch (error) {
      console.error('Failed to load MCP tool menu preferences:', error);
    }
  }, []);

  React.useEffect(() => {
    void loadMcpToolMenuPreferences();
  }, [loadMcpToolMenuPreferences]);

  const loadManagedGroups = React.useCallback(async () => {
    try {
      setManagedGroups(await mcpApi.getMcpGroups());
    } catch (error) {
      console.error('Failed to load MCP groups:', error);
    }
  }, []);

  React.useEffect(() => {
    void loadManagedGroups();
  }, [loadManagedGroups]);

  const allTags = React.useMemo(() => collectAllTags(servers), [servers]);
  const hasUntaggedServer = React.useMemo(
    () => servers.some((server) => !(server.tags ?? []).length),
    [servers],
  );
  const hasTagFilters = allTags.length > 0 || hasUntaggedServer;

  const tagFilterOptions = React.useMemo<TagFilterOption[]>(() => {
    const options: TagFilterOption[] = [];
    if (hasUntaggedServer) {
      const untaggedCount = servers.filter((server) => !(server.tags ?? []).length).length;
      options.push({ value: UNTAGGED_FILTER, label: t('mcp.tags.untagged'), count: untaggedCount });
    }
    for (const tag of allTags) {
      const count = servers.filter((server) => (server.tags ?? []).includes(tag)).length;
      options.push({ value: tag, label: tag, count });
    }
    return options;
  }, [allTags, hasUntaggedServer, servers, t]);

  // Keep dead tag filters pruned as servers/tags change while the page stays mounted.
  React.useEffect(() => {
    setTagFilter((prev) => pruneStaleTagFilters(prev, allTags, hasUntaggedServer));
  }, [allTags, hasUntaggedServer]);

  const handleToggleTagFilter = React.useCallback((tag: string) => {
    setTagFilter((prev) => {
      if (prev.includes(tag)) {
        return prev.filter((existing) => existing !== tag);
      }
      const next = tag === UNTAGGED_FILTER
        ? prev.filter((existing) => existing === UNTAGGED_FILTER)
        : prev.filter((existing) => existing !== UNTAGGED_FILTER);
      return [...next, tag];
    });
  }, []);

  // Filter chain mirrors the Skills page: management status → tag filter → search text.
  const filteredServers = React.useMemo(() => {
    const byStatus = servers.filter((server) => {
      if (enabledFilter === 'enabled') return server.management_enabled;
      if (enabledFilter === 'disabled') return !server.management_enabled;
      return true;
    });
    const byTags = tagFilter.length > 0
      ? byStatus.filter((server) => matchesTagFilters(server, tagFilter))
      : byStatus;
    return filterMcpServersBySearch(byTags, deferredSearchText, getMcpConfigSummary);
  }, [deferredSearchText, enabledFilter, servers, tagFilter]);

  const isSearchActive = !!searchText.trim();
  const isFlatReorderEnabled = viewMode === 'flat' && reorderMode && !isSearchActive;
  const canUseGroupToolMode = viewMode === 'grouped' && !isSearchActive;

  // Active states surfaced in the toolbar options popover: any non-default
  // option lights up the trigger and shows a summary pill inside the popover.
  const toolbarOptionStates = React.useMemo(() => {
    const states: string[] = [];
    if (enabledFilter !== 'all') {
      states.push(t(`mcp.enabledFilter.${enabledFilter}`));
    }
    if (viewMode === 'flat' && reorderMode) {
      states.push(t('mcp.reorder'));
    }
    if (viewMode === 'grouped' && selectionMode) {
      states.push(t('mcp.toolbar.selectionSelect'));
    }
    if (viewMode === 'grouped' && groupToolMode) {
      states.push(t('mcp.toolbar.groupTools'));
    }
    return states;
  }, [enabledFilter, groupToolMode, reorderMode, selectionMode, t, viewMode]);

  const toolbarOptionsActive = toolbarOptionStates.length > 0;
  const toolbarOptionsTitle = toolbarOptionsActive
    ? t('mcp.toolbar.optionsActive', { states: toolbarOptionStates.join(' / ') })
    : t('mcp.toolbar.options');

  const flatReorderDisabledHint = viewMode === 'flat' && isSearchActive
    ? t('mcp.reorderDisabledWhileSearching')
    : null;
  const groupToolsDisabledHint = viewMode === 'grouped' && !canUseGroupToolMode
    ? t('mcp.groupTools.disabledWhileSearching')
    : null;
  const gridColumns = gridColumnSetting === 'auto' ? undefined : gridColumnSetting;
  const effectivePreferredToolsForAddMore = React.useMemo(() => {
    if (preferredToolsForAddMore.length > 0) {
      return preferredToolsForAddMore;
    }
    return tools
      .filter((tool) => tool.installed && tool.supports_mcp)
      .map((tool) => tool.key);
  }, [preferredToolsForAddMore, tools]);
  const groupOptions = React.useMemo(() => getMcpGroupOptions(servers), [servers]);
  const groupedServers = React.useMemo<McpGroup[]>(() => {
    if (viewMode !== 'grouped') return [];

    return buildMcpGroups(filteredServers, {
      groupUngrouped: t('mcp.groupUngrouped'),
    });
  }, [filteredServers, t, viewMode]);

  const packageVersionRequests = React.useMemo(() => {
    const requestMap = new Map<string, { manager: 'npx' | 'uv'; package_name: string }>();
    for (const server of servers) {
      if (server.server_type !== 'stdio') {
        continue;
      }

      const packageVersion = getMcpCommandPackageVersion(server.server_config);
      if (!packageVersion || packageVersion.versionLabel !== 'latest') {
        continue;
      }

      const key = getMcpCommandPackageVersionKey(packageVersion.manager, packageVersion.packageName);
      requestMap.set(key, {
        manager: packageVersion.manager,
        package_name: packageVersion.packageName,
      });
    }

    return [...requestMap.values()];
  }, [servers]);

  const packageVersionRequestKey = React.useMemo(() => (
    packageVersionRequests
      .map((request) => `${request.manager}:${request.package_name.toLowerCase()}`)
      .sort()
      .join('|')
  ), [packageVersionRequests]);

  React.useEffect(() => {
    if (viewMode !== 'flat' || isSearchActive) {
      setReorderMode(false);
    }
  }, [isSearchActive, viewMode]);

  React.useEffect(() => {
    if (packageVersionRequests.length === 0) {
      return;
    }

    let cancelled = false;
    mcpApi.resolveMcpPackageVersions(packageVersionRequests)
      .then((results) => {
        if (cancelled) {
          return;
        }

        setResolvedPackageVersions((previousVersions) => {
          const nextVersions = { ...previousVersions };
          for (const result of results) {
            if (!result.version) {
              continue;
            }

            nextVersions[getMcpCommandPackageVersionKey(result.manager, result.package_name)] = result.version;
          }
          return nextVersions;
        });
      })
      .catch((error) => {
        console.warn('Failed to resolve MCP package versions:', error);
      });

    return () => {
      cancelled = true;
    };
  }, [packageVersionRequestKey, packageVersionRequests]);

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

  React.useEffect(() => {
    if (viewMode !== 'grouped') {
      setSelectionMode(false);
      setSelectedIds(new Set());
      return;
    }

    setSelectedIds((previousSelectedIds) => {
      const visibleServerIds = new Set(filteredServers.map((server) => server.id));
      const nextSelectedIds = new Set([...previousSelectedIds].filter((id) => visibleServerIds.has(id)));
      return nextSelectedIds.size === previousSelectedIds.size ? previousSelectedIds : nextSelectedIds;
    });
  }, [filteredServers, viewMode]);

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

  const selectedArray = React.useMemo(() => [...selectedIds], [selectedIds]);
  const selectedServers = React.useMemo(
    () => servers.filter((server) => selectedIds.has(server.id)),
    [selectedIds, servers],
  );
  const selectedDisabledServerIds = React.useMemo(
    () => selectedServers.filter((server) => !server.management_enabled).map((server) => server.id),
    [selectedServers],
  );
  const selectedEnabledServerIds = React.useMemo(
    () => selectedServers.filter((server) => server.management_enabled).map((server) => server.id),
    [selectedServers],
  );
  const hasSelection = selectedArray.length > 0;
  const installedTools = React.useMemo(() => tools.filter((tool) => tool.installed), [tools]);
  const detailServer = React.useMemo(
    () => servers.find((server) => server.id === detailServerId) ?? null,
    [detailServerId, servers],
  );

  const handleOpenDetail = React.useCallback((server: McpServer) => {
    setDetailServerId(server.id);
  }, []);

  const handleCloseDetail = React.useCallback(() => {
    setDetailServerId(null);
  }, []);

  const handleToggleSelectionMode = React.useCallback(() => {
    if (selectionMode) {
      setSelectedIds(new Set());
    }
    setSelectionMode((previousSelectionMode) => !previousSelectionMode);
  }, [selectionMode]);

  const handleSelectChange = React.useCallback((serverId: string, checked: boolean) => {
    setSelectedIds((previousSelectedIds) => {
      const nextSelectedIds = new Set(previousSelectedIds);
      if (checked) {
        nextSelectedIds.add(serverId);
      } else {
        nextSelectedIds.delete(serverId);
      }
      return nextSelectedIds;
    });
  }, []);

  const handleSelectAllGroup = React.useCallback((group: McpGroup, checked: boolean) => {
    setSelectedIds((previousSelectedIds) => {
      const nextSelectedIds = new Set(previousSelectedIds);
      for (const server of group.servers) {
        if (checked) {
          nextSelectedIds.add(server.id);
        } else {
          nextSelectedIds.delete(server.id);
        }
      }
      return nextSelectedIds;
    });
  }, []);

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

  const handleBatchAddTool = React.useCallback(async (toolKey: string) => {
    const missingServerIds = selectedServers
      .filter((server) => !server.enabled_tools.includes(toolKey))
      .map((server) => server.id);
    await applyMcpToolState(missingServerIds, toolKey, true);
  }, [applyMcpToolState, selectedServers]);

  const handleBatchRemoveTool = React.useCallback(async (toolKey: string) => {
    const enabledServerIds = selectedServers
      .filter((server) => server.enabled_tools.includes(toolKey))
      .map((server) => server.id);
    await applyMcpToolState(enabledServerIds, toolKey, false);
  }, [applyMcpToolState, selectedServers]);

  const batchAddToolItems = React.useMemo<ManagementMenuItem[]>(
    () => installedTools.map((tool) => ({
      key: `add-${tool.key}`,
      label: tool.display_name,
      onSelect: () => handleBatchAddTool(tool.key),
    })),
    [handleBatchAddTool, installedTools],
  );

  const batchRemoveToolItems = React.useMemo<ManagementMenuItem[]>(
    () => installedTools.map((tool) => ({
      key: `remove-${tool.key}`,
      label: tool.display_name,
      onSelect: () => handleBatchRemoveTool(tool.key),
    })),
    [handleBatchRemoveTool, installedTools],
  );

  const handleConfirmBatchGroup = React.useCallback(async () => {
    if (selectedServers.length === 0) {
      return;
    }

    setActionLoading(true);
    try {
      const nextGroup = normalizeMcpMetadataText(batchGroupValue);
      for (const server of selectedServers) {
        await mcpApi.updateMcpMetadata(
          server.id,
          nextGroup,
          normalizeMcpMetadataText(server.user_note),
        );
      }
      await refresh();
      message.success(t('mcp.batch.setGroupSuccess', { count: selectedServers.length }));
      setBatchGroupModalOpen(false);
      setBatchGroupValue('');
      setSelectedIds(new Set());
    } catch (error) {
      message.error(t('mcp.serverUpdateFailed') + ': ' + String(error));
      await refresh();
    } finally {
      setActionLoading(false);
    }
  }, [batchGroupValue, refresh, selectedServers, t]);

  const handleUpdateMcpTags = React.useCallback(async (serverId: string, nextTags: string[]) => {
    const server = servers.find((s) => s.id === serverId);
    if (!server) return;
    setActionLoading(true);
    try {
      await mcpApi.updateMcpMetadata(
        serverId,
        normalizeMcpMetadataText(server.user_group),
        normalizeMcpMetadataText(server.user_note),
        nextTags,
      );
      await refresh();
      message.success(t('mcp.tags.saveSuccess'));
    } catch (error) {
      message.error(t('mcp.serverUpdateFailed') + ': ' + String(error));
      await refresh();
    } finally {
      setActionLoading(false);
    }
  }, [refresh, servers, t]);

  const handleBatchDelete = React.useCallback(() => {
    if (selectedArray.length === 0) {
      return;
    }

    Modal.confirm({
      title: t('mcp.batch.deleteConfirmTitle'),
      content: t('mcp.batch.deleteConfirmMessage', { count: selectedArray.length }),
      okText: t('common.delete'),
      okType: 'danger',
      cancelText: t('common.cancel'),
      onOk: async () => {
        setActionLoading(true);
        try {
          for (const serverId of selectedArray) {
            await mcpApi.deleteMcpServer(serverId);
          }
          await refresh();
          message.success(t('mcp.batch.deleteSuccess', { count: selectedArray.length }));
          setSelectedIds(new Set());
        } catch (error) {
          message.error(t('mcp.serverDeleteFailed') + ': ' + String(error));
          await refresh();
        } finally {
          setActionLoading(false);
        }
      },
    });
  }, [refresh, selectedArray, t]);

  // Restorable tools are a subset of the still-installed tools that were recorded
  // before the server was disabled. Only these candidates may be asked to confirm.
  const getRestorableToolIds = React.useCallback(
    (server: McpServer): string[] =>
      (server.disabled_previous_tools ?? []).filter((key) =>
        installedTools.some((tool) => tool.key === key),
      ),
    [installedTools],
  );

  const handleSetManagementEnabled = React.useCallback(
    (server: McpServer, enabled: boolean) => {
      if (enabled) {
        // Enable restores the management flag first; the backend returns the list
        // of previously-bound tools. Confirm restore of the still-installed subset,
        // then push through the regular restore path.
        const restorableKeys = getRestorableToolIds(server);
        Modal.confirm({
          title: t('mcp.enableConfirmTitle'),
          content:
            restorableKeys.length === 0
              ? t('mcp.enableConfirmEmpty')
              : t('mcp.enableConfirmContent', { count: restorableKeys.length }),
          okText: t('mcp.enableServer'),
          cancelText: t('common.cancel'),
          onOk: async () => {
            setActionLoading(true);
            try {
              await enableServer(server.id);
              if (restorableKeys.length > 0) {
                await restoreTools(server.id, restorableKeys);
              }
              await refresh();
            } catch (error) {
              message.error(t('mcp.serverEnableFailed') + ': ' + String(error));
              await refresh();
            } finally {
              setActionLoading(false);
            }
          },
        });
        return;
      }

      const count = (server.enabled_tools ?? []).length;
      Modal.confirm({
        title: t('mcp.disableConfirmTitle'),
        content: t('mcp.disableConfirmContent', {
          name: server.name,
          count,
        }),
        okText: t('mcp.disableServer'),
        okType: 'danger',
        cancelText: t('common.cancel'),
        onOk: async () => {
          setActionLoading(true);
          try {
            await disableServer(server.id);
            await refresh();
          } catch (error) {
            message.error(t('mcp.serverDisableFailed') + ': ' + String(error));
            await refresh();
          } finally {
            setActionLoading(false);
          }
        },
      });
    },
    [getRestorableToolIds, enableServer, disableServer, restoreTools, refresh, t],
  );

  const handleBatchSetManagementEnabled = React.useCallback(
    (enabled: boolean) => {
      const ids = (enabled ? selectedDisabledServerIds : selectedEnabledServerIds) ?? selectedArray;
      if (ids.length === 0) {
        return;
      }
      const targetServers = servers.filter((s) => ids.includes(s.id));

      if (enabled) {
        // Aggregate restore candidates for the disabled servers being enabled.
        const restoreMap: Record<string, string[]> = {};
        for (const s of targetServers) {
          const keys = getRestorableToolIds(s);
          if (keys.length > 0) {
            restoreMap[s.id] = keys;
          }
        }
        const toolCount = Object.values(restoreMap).reduce((sum, arr) => sum + arr.length, 0);
        Modal.confirm({
          title: t('mcp.batch.enableConfirmTitle'),
          content:
            toolCount === 0
              ? t('mcp.batch.enableConfirmEmpty', { count: ids.length })
              : t('mcp.batch.enableConfirmContent', {
                  count: ids.length,
                  tools: toolCount,
                }),
          okText: t('mcp.batch.enable'),
          cancelText: t('common.cancel'),
          onOk: async () => {
            setActionLoading(true);
            try {
              await batchSetManagementEnabled(ids, true);
              if (toolCount > 0) {
                for (const [serverId, keys] of Object.entries(restoreMap)) {
                  await restoreTools(serverId, keys);
                }
              }
              await refresh();
              setSelectedIds(new Set());
            } catch (error) {
              message.error(t('mcp.batch.enableFailed') + ': ' + String(error));
              await refresh();
            } finally {
              setActionLoading(false);
            }
          },
        });
        return;
      }

      const toolCount = targetServers.reduce((sum, s) => sum + (s.enabled_tools ?? []).length, 0);
      Modal.confirm({
        title: t('mcp.batch.disableConfirmTitle'),
        content: t('mcp.batch.disableConfirmContent', {
          count: ids.length,
          tools: toolCount,
        }),
        okText: t('mcp.batch.disable'),
        okType: 'danger',
        cancelText: t('common.cancel'),
        onOk: async () => {
          setActionLoading(true);
          try {
            await batchSetManagementEnabled(ids, false);
            await refresh();
            setSelectedIds(new Set());
          } catch (error) {
            message.error(t('mcp.batch.disableFailed') + ': ' + String(error));
            await refresh();
          } finally {
            setActionLoading(false);
          }
        },
      });
    },
    [selectedArray, servers, getRestorableToolIds, batchSetManagementEnabled, restoreTools, refresh, t],
  );

  // Batch enable/disable of management state. Each entry keeps its own
  // disabled guard so a selection with no disabled servers never blocks
  // the re-enable entry and vice versa (the re-enable path must stay usable).
  const batchManagementStateItems = React.useMemo<ManagementMenuItem[]>(
    () => [
      {
        key: 'enable-selected',
        icon: <Power size={14} />,
        label: t('mcp.batch.enable'),
        onSelect: () => handleBatchSetManagementEnabled(true),
        disabled: selectedDisabledServerIds.length === 0,
      },
      {
        key: 'disable-selected',
        icon: <PowerOff size={14} />,
        label: t('mcp.batch.disable'),
        danger: true,
        onSelect: () => handleBatchSetManagementEnabled(false),
        disabled: selectedEnabledServerIds.length === 0,
      },
    ],
    [handleBatchSetManagementEnabled, selectedDisabledServerIds, selectedEnabledServerIds, t],
  );

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

  const handleToolMenuPreferencesChange = React.useCallback((preferences: {
    preferredTools: string[];
    limitAddMoreToPreferredTools: boolean;
  }) => {
    setPreferredToolsForAddMore(preferences.preferredTools);
    setLimitAddMoreToPreferredTools(preferences.limitAddMoreToPreferredTools);
  }, []);

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
        <div className={styles.titleBlock}>
          <div className={styles.titleRow}>
            <h1 className={styles.title}>{t('mcp.title')}</h1>
            <button
              type="button"
              className={styles.docsLink}
              onClick={() => openUrl('https://code.claude.com/docs/en/mcp#installing-mcp-servers')}
            >
              <ExternalLink size={13} aria-hidden="true" />
              {t('mcp.viewDocs')}
            </button>
          </div>
          <p className={styles.pageHint}>{t('mcp.pageHint')}</p>
        </div>
        <ManagementButton
          variant="ghost"
          icon={<MoreHorizontal size={16} aria-hidden="true" />}
          className={styles.moreMenuTrigger}
          onClick={() => setSettingsModalOpen(true)}
        >
          {t('mcp.settings')}
        </ManagementButton>
      </div>

      <div className={styles.toolbar}>
        <div className={styles.toolbarPrimary}>
          <ManagementSearchInput
            placeholder={t('mcp.searchPlaceholder')}
            clearLabel={t('common.clearSearch')}
            value={searchText}
            onChange={setSearchText}
            className={styles.toolbarSearch}
          />
          <span className={styles.resultCount}>
            {filteredServers.length}/{servers.length}
          </span>
          <ManagementButton
            variant="subtle"
            controlSize="compact"
            icon={<Import size={14} aria-hidden="true" />}
            onClick={() => setImportModalOpen(true)}
          >
            {t('mcp.importExisting')}
          </ManagementButton>
          <ManagementButton
            variant="subtle"
            controlSize="compact"
            icon={<FileText size={14} aria-hidden="true" />}
            onClick={() => setImportJsonModalOpen(true)}
          >
            {t('mcp.importJson.button')}
          </ManagementButton>
          <ManagementButton
            variant="primary"
            controlSize="compact"
            icon={<Plus size={14} aria-hidden="true" />}
            onClick={() => setAddModalOpen(true)}
          >
            {t('mcp.addServer')}
          </ManagementButton>
          {hasTagFilters && (
            <TagFilterDropdown
              options={tagFilterOptions}
              selected={tagFilter}
              onToggle={handleToggleTagFilter}
              onClear={() => setTagFilter([])}
            />
          )}
        </div>
        <div className={styles.toolbarActions}>
          <ToolbarOptionsPopover
            title={t('mcp.toolbar.options')}
            active={toolbarOptionsActive}
            activeTitle={toolbarOptionsTitle}
          >
            {({ close }) => (
              <>
                <section className={styles.toolbarOptionsSection} aria-label={t('mcp.toolbar.viewControls')}>
                  <div className={styles.toolbarOptionsSectionTitle}>{t('mcp.toolbar.viewFilters')}</div>
                  {toolbarOptionsActive && (
                    <div className={styles.toolbarActiveSummary} title={toolbarOptionStates.join(' / ')}>
                      <span className={styles.toolbarActiveDot} aria-hidden="true" />
                      <span className={styles.toolbarActiveText}>
                        {t('mcp.toolbar.activeSummary', { states: toolbarOptionStates.join(' / ') })}
                      </span>
                    </div>
                  )}
                  <div className={styles.toolbarOptionRow}>
                    <span className={styles.toolbarOptionLabel}>{t('mcp.enabledFilter.label')}</span>
                    <ManagementSegmented<'all' | 'enabled' | 'disabled'>
                      value={enabledFilter}
                      ariaLabel={t('mcp.enabledFilter.label')}
                      onChange={setEnabledFilter}
                      options={[
                        { value: 'all', label: t('mcp.enabledFilter.all') },
                        { value: 'enabled', label: t('mcp.enabledFilter.enabled') },
                        { value: 'disabled', label: t('mcp.enabledFilter.disabled') },
                      ]}
                    />
                  </div>
                  {viewMode === 'flat' && (
                    <div className={styles.toolbarOptionRow}>
                      <span className={styles.toolbarOptionLabel}>{t('mcp.toolbar.arrange')}</span>
                      <ManagementSegmented<'browse' | 'reorder'>
                        value={reorderMode ? 'reorder' : 'browse'}
                        ariaLabel={t('mcp.reorder')}
                        title={isSearchActive ? t('mcp.reorderDisabledWhileSearching') : t('mcp.reorderHint')}
                        disabled={loading || actionLoading || isSearchActive}
                        onChange={(nextMode) => setReorderMode(nextMode === 'reorder')}
                        options={[
                          { value: 'browse', label: t('mcp.toolbar.browseMode') },
                          {
                            value: 'reorder',
                            icon: <GripVertical size={13} aria-hidden="true" />,
                            label: t('mcp.reorder'),
                            title: isSearchActive ? t('mcp.reorderDisabledWhileSearching') : t('mcp.reorderHint'),
                          },
                        ]}
                      />
                      {flatReorderDisabledHint && (
                        <div className={styles.toolbarOptionHint}>{flatReorderDisabledHint}</div>
                      )}
                    </div>
                  )}
                  {viewMode === 'grouped' && (
                    <>
                      <div className={styles.toolbarOptionRow}>
                        <span className={styles.toolbarOptionLabel}>{t('mcp.toolbar.selectionMode')}</span>
                        <ManagementSegmented<'browse' | 'select'>
                          value={selectionMode ? 'select' : 'browse'}
                          ariaLabel={t('mcp.toolbar.selectionMode')}
                          onChange={(nextMode) => {
                            if ((nextMode === 'select') !== selectionMode) {
                              handleToggleSelectionMode();
                            }
                          }}
                          options={[
                            { value: 'browse', label: t('mcp.toolbar.browseMode') },
                            {
                              value: 'select',
                              label: t('mcp.toolbar.selectionSelect'),
                              title: t('mcp.groupControls.selectionModeTip'),
                            },
                          ]}
                        />
                        <div className={styles.toolbarOptionInfo}>{t('mcp.groupControls.selectionModeTip')}</div>
                      </div>
                      <div className={styles.toolbarOptionRow}>
                        <span className={styles.toolbarOptionLabel}>{t('mcp.toolbar.groupTools')}</span>
                        <ManagementSegmented<'independent' | 'aggregated'>
                          value={groupToolMode ? 'aggregated' : 'independent'}
                          ariaLabel={t('mcp.toolbar.groupTools')}
                          title={
                            isSearchActive
                              ? t('mcp.groupTools.disabledWhileSearching')
                              : t('mcp.groupControls.groupToolsTip')
                          }
                          disabled={loading || actionLoading || isSearchActive}
                          onChange={(nextMode) => handleToggleGroupToolMode(nextMode === 'aggregated')}
                          options={[
                            { value: 'independent', label: t('mcp.toolbar.groupToolsIndependent') },
                            {
                              value: 'aggregated',
                              label: t('mcp.toolbar.groupToolsAggregated'),
                              title: isSearchActive
                                ? t('mcp.groupTools.disabledWhileSearching')
                                : t('mcp.groupControls.groupToolsTip'),
                            },
                          ]}
                        />
                        <div className={styles.toolbarOptionInfo}>{t('mcp.groupControls.groupToolsTip')}</div>
                        {groupToolsDisabledHint && (
                          <div className={styles.toolbarOptionHint}>{groupToolsDisabledHint}</div>
                        )}
                      </div>
                    </>
                  )}
                </section>
                <section className={styles.toolbarOptionsSection} aria-label={t('mcp.toolbar.management')}>
                  <div className={styles.toolbarOptionsSectionTitle}>{t('mcp.toolbar.management')}</div>
                  <div className={styles.toolbarActionList}>
                    <ToolbarActionItem
                      icon={<Folders size={14} aria-hidden="true" />}
                      title={t('mcp.toolbar.groupManagement')}
                      description={t('mcp.toolbar.groupManagementDescription')}
                      onClick={() => {
                        close();
                        setGroupsModalOpen(true);
                      }}
                    />
                    <ToolbarActionItem
                      icon={<FileJson size={14} aria-hidden="true" />}
                      title={t('mcp.inventory.button')}
                      description={t('mcp.toolbar.inventoryDescription')}
                      onClick={() => {
                        close();
                        setInventoryModalOpen(true);
                      }}
                    />
                  </div>
                </section>
              </>
            )}
          </ToolbarOptionsPopover>
          {viewMode === 'grouped' && selectionMode && (
            <>
              <ManagementMenu
                items={batchAddToolItems}
                disabled={!hasSelection || loading || actionLoading}
                title={hasSelection ? t('mcp.batch.addTool') : t('mcp.batch.noneSelected')}
                controlSize="compact"
              >
                <PlusCircle size={14} aria-hidden="true" />
              </ManagementMenu>
              <ManagementMenu
                items={batchRemoveToolItems}
                disabled={!hasSelection || loading || actionLoading}
                title={hasSelection ? t('mcp.batch.removeTool') : t('mcp.batch.noneSelected')}
                controlSize="compact"
              >
                <MinusCircle size={14} aria-hidden="true" />
              </ManagementMenu>
              <ManagementIconButton
                icon={<Tags size={14} aria-hidden="true" />}
                title={hasSelection ? t('mcp.batch.setGroup') : t('mcp.batch.noneSelected')}
                disabled={!hasSelection || loading || actionLoading}
                onClick={() => {
                  setBatchGroupValue('');
                  setBatchGroupModalOpen(true);
                }}
                controlSize="compact"
              />
              <ManagementIconButton
                icon={<Trash2 size={14} aria-hidden="true" />}
                title={hasSelection ? t('mcp.batch.delete') : t('mcp.batch.noneSelected')}
                disabled={!hasSelection || loading || actionLoading}
                onClick={handleBatchDelete}
                danger
                controlSize="compact"
              />
              <ManagementMenu
                items={batchManagementStateItems}
                disabled={!hasSelection || loading || actionLoading}
                title={hasSelection ? t('mcp.batch.managementState') : t('mcp.batch.noneSelected')}
                controlSize="compact"
              >
                <Power size={14} aria-hidden="true" />
              </ManagementMenu>
              <span className={styles.batchDivider} />
            </>
          )}
          {viewMode === 'grouped' && (
            <>
              <ManagementIconButton
                icon={<ChevronsDown size={14} aria-hidden="true" />}
                title={t('mcp.expandAll')}
                onClick={() => setGroupActiveKeys(groupedServers.map((g) => g.key))}
                controlSize="compact"
              />
              <ManagementIconButton
                icon={<ChevronsUp size={14} aria-hidden="true" />}
                title={t('mcp.collapseAll')}
                onClick={() => setGroupActiveKeys([])}
                controlSize="compact"
              />
            </>
          )}
          <ManagementSegmented<'flat' | 'grouped'>
            value={viewMode}
            ariaLabel={t('mcp.groupedViewTip')}
            className={styles.viewModeSegmented}
            onChange={setViewMode}
            options={[
              { value: 'flat', icon: <LayoutGrid size={13} aria-hidden="true" />, label: t('mcp.viewFlat') },
              { value: 'grouped', icon: <ListTree size={13} aria-hidden="true" />, label: t('mcp.viewGrouped') },
            ]}
          />
        </div>
      </div>

      <div className={styles.content}>
        {viewMode === 'flat' ? (
          <McpList
            servers={filteredServers}
            tools={tools}
            loading={loading || actionLoading}
            columns={gridColumns}
            dragDisabled={!isFlatReorderEnabled}
            resolvedPackageVersions={resolvedPackageVersions}
            preferredToolKeysForAddMore={effectivePreferredToolsForAddMore}
            limitAddMoreToPreferredTools={limitAddMoreToPreferredTools}
            onOpenDetail={handleOpenDetail}
            onEdit={handleEdit}
            onEditMetadata={setMetadataServer}
            onDelete={handleDelete}
            onToggleTool={handleToggleTool}
            onRefresh={refresh}
            onDragEnd={handleDragEnd}
            onSetManagementEnabled={handleSetManagementEnabled}
          />
        ) : (
          <McpGroupedList
            groups={groupedServers}
            tools={tools}
            loading={loading || actionLoading}
            columns={gridColumns}
            resolvedPackageVersions={resolvedPackageVersions}
            preferredToolKeysForAddMore={effectivePreferredToolsForAddMore}
            limitAddMoreToPreferredTools={limitAddMoreToPreferredTools}
            activeKeys={groupActiveKeys}
            onActiveKeysChange={setGroupActiveKeys}
            selectionMode={selectionMode}
            selectedIds={selectedIds}
            onSelectChange={handleSelectChange}
            onSelectAllGroup={handleSelectAllGroup}
            onOpenDetail={handleOpenDetail}
            onEdit={handleEdit}
            onEditMetadata={setMetadataServer}
            onDelete={handleDelete}
            onToggleTool={handleToggleTool}
            onRefresh={refresh}
            groupToolMode={groupToolMode}
            onAddGroupTool={handleAddGroupTool}
            onRemoveGroupTool={handleRemoveGroupTool}
            onSetManagementEnabled={handleSetManagementEnabled}
          />
        )}
      </div>

      <Drawer
        placement="right"
        width="min(60vw, 760px)"
        open={!!detailServer}
        onClose={handleCloseDetail}
        destroyOnHidden
        closable={false}
        styles={{
          body: {
            padding: 0,
            overflow: 'hidden',
            display: 'flex',
            flexDirection: 'column',
          },
        }}
      >
        {detailServer && (
          <McpDetailPanel
            server={detailServer}
            tools={tools}
            loading={loading || actionLoading}
            toolsReadOnly={groupToolMode}
            resolvedPackageVersions={resolvedPackageVersions}
            allTags={allTags}
            onUpdateTags={handleUpdateMcpTags}
            onClose={handleCloseDetail}
            onEdit={handleEdit}
            onEditMetadata={setMetadataServer}
            onDelete={(serverId) => {
              setDetailServerId(null);
              handleDelete(serverId);
            }}
            onToggleTool={handleToggleTool}
            onSetManagementEnabled={handleSetManagementEnabled}
          />
        )}
      </Drawer>

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
          cardColumnSetting={gridColumnSetting}
          cardColumnOptions={MANAGEMENT_GRID_COLUMN_OPTIONS}
          onCardColumnSettingChange={setGridColumnSetting}
          onToolMenuPreferencesChange={handleToolMenuPreferencesChange}
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

      <Modal
        open={batchGroupModalOpen}
        title={t('mcp.batch.setGroupTitle')}
        onCancel={() => setBatchGroupModalOpen(false)}
        onOk={handleConfirmBatchGroup}
        okText={t('common.save')}
        cancelText={t('common.cancel')}
        confirmLoading={actionLoading}
        okButtonProps={{ disabled: selectedArray.length === 0 }}
      >
        <div className={styles.batchGroupEditor}>
          <input
            className={styles.batchGroupInput}
            value={batchGroupValue}
            list="mcp-batch-group-options"
            placeholder={t('mcp.metadata.groupPlaceholder')}
            onChange={(event) => setBatchGroupValue(event.target.value)}
          />
          <datalist id="mcp-batch-group-options">
            {groupOptions.map((group) => (
              <option key={group} value={group} />
            ))}
          </datalist>
          <p className={styles.batchGroupHint}>
            {t('mcp.batch.setGroupHint')}
          </p>
        </div>
      </Modal>

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

      <McpGroupsModal
        open={groupsModalOpen}
        groups={managedGroups}
        onClose={() => setGroupsModalOpen(false)}
        onSuccess={() => {
          loadManagedGroups();
          refresh();
        }}
      />

      <McpInventoryModal
        open={inventoryModalOpen}
        onClose={() => setInventoryModalOpen(false)}
        onSuccess={refresh}
      />
    </div>
  );
};

export default McpPage;
