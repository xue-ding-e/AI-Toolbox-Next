import React from 'react';
import {
  CheckOutlined,
  CloseOutlined,
  ClockCircleOutlined,
  CopyOutlined,
  DeleteOutlined,
  ExclamationCircleOutlined,
  ExportOutlined,
  ImportOutlined,
  FolderOpenOutlined,
  MessageOutlined,
  ReloadOutlined,
  SearchOutlined,
} from '@ant-design/icons';
import {
  Button,
  Checkbox,
  Collapse,
  Empty,
  Input,
  Modal,
  Select,
  Spin,
  Tag,
  Typography,
  message,
} from 'antd';
import { useTranslation } from 'react-i18next';
import { useLocation, useNavigate } from 'react-router-dom';
import { open } from '@tauri-apps/plugin-dialog';

import {
  deleteToolSessions,
  deleteToolSession,
  exportToolSessions,
  importToolSession,
  listToolSessions,
} from './sessionManagerApi';
import type {
  DeleteToolSessionsResult,
  ExportToolSessionsResult,
  SessionMeta,
  SessionPathOption,
  SessionSourceMode,
  SessionSourceOption,
  SessionTool,
} from './types';
import {
  buildSessionDetailPath,
  SESSION_MANAGER_REFRESH_EVENT,
  type SessionManagerRefreshEventDetail,
} from './sessionDetailNavigation';
import {
  advanceVisibleContextId,
  formatRelativeTime,
  formatSessionTitle,
  shortSessionId,
  shouldShowVisibleFeedback as shouldShowVisibleFeedbackForContext,
} from './utils';
import { useKeepAlive } from '@/components/layout/KeepAliveOutlet';
import styles from './SessionManagerPanel.module.less';

const { Text } = Typography;

interface SessionManagerPanelProps {
  tool: SessionTool;
  translationKey?: string;
  expandNonce?: number;
  refreshNonce?: number;
  extra?: React.ReactNode;
}

const PAGE_SIZE = 10;
const ALL_PATHS_VALUE = '__all_paths__';
let rememberedSessionSourceMode: SessionSourceMode = 'all';

interface SessionManagerContentProps {
  tool: SessionTool;
  expanded: boolean;
  refreshNonce?: number;
  sourceMode: SessionSourceMode;
  showRuntimeSourceTag: boolean;
  onAvailableSourcesChange: (sources: SessionSourceOption[]) => void;
}

const SessionManagerContent: React.FC<SessionManagerContentProps> = ({
  tool,
  expanded,
  refreshNonce = 0,
  sourceMode,
  showRuntimeSourceTag,
  onAvailableSourcesChange,
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const { isActive, rememberScrollPosition } = useKeepAlive();
  const sentinelRef = React.useRef<HTMLDivElement | null>(null);
  const [query, setQuery] = React.useState('');
  const [debouncedQuery, setDebouncedQuery] = React.useState('');
  const [pathFilter, setPathFilter] = React.useState('');
  const [loading, setLoading] = React.useState(false);
  const [loadingMore, setLoadingMore] = React.useState(false);
  const [pathOptions, setPathOptions] = React.useState<SessionPathOption[]>([]);
  const [pathOptionsLoading, setPathOptionsLoading] = React.useState(false);
  const [items, setItems] = React.useState<SessionMeta[]>([]);
  const [page, setPage] = React.useState(1);
  const [hasMore, setHasMore] = React.useState(false);
  const [total, setTotal] = React.useState(0);
  const [importing, setImporting] = React.useState(false);
  const [selectionMode, setSelectionMode] = React.useState(false);
  const [selectedSourcePaths, setSelectedSourcePaths] = React.useState<string[]>([]);
  const [bulkExporting, setBulkExporting] = React.useState(false);
  const [bulkDeleting, setBulkDeleting] = React.useState(false);
  const listContextIdRef = React.useRef(0);
  const listReplaceRequestIdRef = React.useRef(0);
  const listAppendRequestIdRef = React.useRef(0);
  const activePageRef = React.useRef(isActive);
  const visibleContextIdRef = React.useRef(0);
  const previousSourceModeRef = React.useRef(sourceMode);
  const clearSelection = React.useCallback(() => {
    setSelectedSourcePaths([]);
  }, []);

  // KeepAlive pages stay mounted when hidden, so refs must be synchronized
  // during render to avoid effect timing races with in-flight async callbacks.
  visibleContextIdRef.current = advanceVisibleContextId(
    visibleContextIdRef.current,
    activePageRef.current,
    isActive,
  );
  activePageRef.current = isActive;

  const captureVisibleContextId = React.useCallback(() => visibleContextIdRef.current, []);

  const shouldShowVisibleFeedback = React.useCallback((visibleContextId?: number) => {
    return shouldShowVisibleFeedbackForContext(
      activePageRef.current,
      visibleContextId,
      visibleContextIdRef.current,
    );
  }, []);

  React.useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedQuery(query.trim()), 250);
    return () => window.clearTimeout(timer);
  }, [query]);

  React.useEffect(() => {
    if (expanded) {
      return;
    }

    listContextIdRef.current += 1;
    listReplaceRequestIdRef.current += 1;
    listAppendRequestIdRef.current += 1;
    setLoading(false);
    setLoadingMore(false);
    setPathOptions([]);
    setPathOptionsLoading(false);
    setSelectionMode(false);
    setSelectedSourcePaths([]);
    setBulkExporting(false);
  }, [expanded]);

  React.useEffect(() => {
    if (previousSourceModeRef.current === sourceMode) {
      return;
    }

    previousSourceModeRef.current = sourceMode;
    setPathFilter('');
  }, [sourceMode]);

  const loadSessions = React.useCallback(async (
    nextPage: number,
    append: boolean,
    forceRefresh = false,
  ) => {
    if (!expanded) {
      return;
    }

    const visibleContextId = captureVisibleContextId();
    const requestContextId = append ? listContextIdRef.current : listContextIdRef.current + 1;
    const requestId = append
      ? listAppendRequestIdRef.current + 1
      : listReplaceRequestIdRef.current + 1;

    const isCurrentRequest = () => {
      if (requestContextId !== listContextIdRef.current) {
        return false;
      }
      return append
        ? requestId === listAppendRequestIdRef.current
        : requestId === listReplaceRequestIdRef.current;
    };
    const finishLoadingState = () => {
      if (append) {
        if (requestId === listAppendRequestIdRef.current) {
          setLoadingMore(false);
        }
        return;
      }

      if (requestId === listReplaceRequestIdRef.current) {
        setLoading(false);
        setPathOptionsLoading(false);
      }
    };

    if (append) {
      listAppendRequestIdRef.current = requestId;
      setLoadingMore(true);
    } else {
      listContextIdRef.current = requestContextId;
      listReplaceRequestIdRef.current = requestId;
      listAppendRequestIdRef.current += 1;
      setLoading(true);
      setPathOptionsLoading(true);
      setLoadingMore(false);
      setHasMore(false);
    }

    try {
      const result = await listToolSessions({
        tool,
        query: debouncedQuery || undefined,
        pathFilter: pathFilter || undefined,
        page: nextPage,
        pageSize: PAGE_SIZE,
        forceRefresh,
        sourceMode,
      });

      if (!isCurrentRequest()) {
        return;
      }

      if (!append) {
        clearSelection();
      }

      setItems((current) => (append ? [...current, ...result.items] : result.items));
      setPage(result.page);
      setHasMore(result.hasMore);
      setTotal(result.total);
      onAvailableSourcesChange(result.availableSources ?? []);
      if (!append) {
        setPathOptions([
          {
            label: t('sessionManager.allPaths'),
            value: ALL_PATHS_VALUE,
          },
          ...(result.availablePaths ?? []).map((item) => ({
            label: item,
            value: item,
          })),
        ]);
      }
    } catch (error) {
      if (!isCurrentRequest()) {
        return;
      }
      if (!shouldShowVisibleFeedback(visibleContextId)) {
        return;
      }
      const errorMessage = error instanceof Error ? error.message : String(error);
      message.error(errorMessage || t('common.error'));
    } finally {
      finishLoadingState();
    }
  }, [
    captureVisibleContextId,
    clearSelection,
    debouncedQuery,
    expanded,
    pathFilter,
    shouldShowVisibleFeedback,
    sourceMode,
    t,
    tool,
    onAvailableSourcesChange,
  ]);

  React.useEffect(() => {
    if (!expanded) {
      return;
    }
    void loadSessions(1, false);
  }, [expanded, debouncedQuery, loadSessions, pathFilter, refreshNonce]);

  React.useEffect(() => {
    const handleRefreshEvent = (event: Event) => {
      const detail = (event as CustomEvent<SessionManagerRefreshEventDetail>).detail;
      if (detail?.tool !== tool || !expanded) {
        return;
      }
      void loadSessions(1, false, true);
    };

    window.addEventListener(SESSION_MANAGER_REFRESH_EVENT, handleRefreshEvent);
    return () => window.removeEventListener(SESSION_MANAGER_REFRESH_EVENT, handleRefreshEvent);
  }, [expanded, loadSessions, tool]);

  React.useEffect(() => {
    if (!expanded || !hasMore || loading || loadingMore) {
      return;
    }

    const sentinel = sentinelRef.current;
    if (!sentinel) {
      return;
    }

    const observer = new IntersectionObserver((entries) => {
      const target = entries[0];
      if (target?.isIntersecting) {
        void loadSessions(page + 1, true);
      }
    }, {
      rootMargin: '120px',
    });

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [expanded, hasMore, loadSessions, loading, loadingMore, page]);

  const handleRefresh = async () => {
    await loadSessions(1, false, true);
  };

  const exitSelectionMode = React.useCallback(() => {
    setSelectionMode(false);
    clearSelection();
  }, [clearSelection]);

  React.useEffect(() => {
    clearSelection();
  }, [clearSelection, debouncedQuery, pathFilter]);

  const handleImportSession = async () => {
    let selectedImportPath: string | null = null;

    try {
      const selected = await open({
        multiple: false,
        directory: false,
        title: t('sessionManager.importDialogTitle'),
        filters: [
          {
            name: 'JSON',
            extensions: ['json'],
          },
        ],
      });

      if (!selected || Array.isArray(selected)) {
        return;
      }

      selectedImportPath = selected;
    } catch (error) {
      if (!shouldShowVisibleFeedback()) {
        return;
      }
      const errorMessage = error instanceof Error ? error.message : String(error);
      message.error(errorMessage || t('common.error'));
      return;
    }

    const importPath = selectedImportPath;
    if (!importPath) {
      return;
    }

    const visibleContextId = captureVisibleContextId();

    try {
      setImporting(true);
      await importToolSession(tool, importPath);
      await loadSessions(1, false, true);
      if (shouldShowVisibleFeedback(visibleContextId)) {
        message.success(t('sessionManager.importSuccess'));
      }
    } catch (error) {
      if (!shouldShowVisibleFeedback(visibleContextId)) {
        return;
      }
      const errorMessage = error instanceof Error ? error.message : String(error);
      message.error(errorMessage || t('common.error'));
    } finally {
      setImporting(false);
    }
  };

  const handleOpenDetail = (session: SessionMeta) => {
    const fromScrollTop = rememberScrollPosition();
    navigate(buildSessionDetailPath(tool, session.sourcePath), {
      state: {
        from: location.pathname + location.search,
        fromScrollTop,
      },
    });
  };

  const handleCopyText = async (text: string, successText: string) => {
    try {
      await navigator.clipboard.writeText(text);
      message.success(successText);
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      message.error(errorMessage || t('common.error'));
    }
  };

  const performDeleteSession = async (session: SessionMeta, visibleContextId: number) => {
    await deleteToolSession(tool, session.sourcePath);

    await loadSessions(1, false, true);
    if (shouldShowVisibleFeedback(visibleContextId)) {
      message.success(t('sessionManager.deleteSuccess'));
    }
  };

  const handleSelectionModeToggle = () => {
    if (selectionMode) {
      exitSelectionMode();
      return;
    }

    setSelectionMode(true);
    clearSelection();
  };

  const toggleSessionSelection = (session: SessionMeta) => {
    setSelectedSourcePaths((current) => (
      current.includes(session.sourcePath)
        ? current.filter((path) => path !== session.sourcePath)
        : [...current, session.sourcePath]
    ));
  };

  const handleSelectAllCurrentPage = () => {
    const currentPagePaths = items.map((session) => session.sourcePath);
    const allSelected = currentPagePaths.length > 0
      && currentPagePaths.every((sourcePath) => selectedSourcePaths.includes(sourcePath));

    setSelectedSourcePaths((current) => {
      if (allSelected) {
        return current.filter((sourcePath) => !currentPagePaths.includes(sourcePath));
      }

      const nextSelected = new Set(current);
      currentPagePaths.forEach((sourcePath) => {
        nextSelected.add(sourcePath);
      });
      return Array.from(nextSelected);
    });
  };

  const performBulkDeleteSessions = async (
    visibleContextId: number,
  ): Promise<DeleteToolSessionsResult> => {
    const result = await deleteToolSessions(tool, selectedSourcePaths);
    const failedSourcePathSet = new Set(result.failedItems.map((item) => item.sourcePath));

    await loadSessions(1, false, true);

    if (result.deletedCount > 0 && shouldShowVisibleFeedback(visibleContextId)) {
      message.success(t('sessionManager.bulkDeleteSuccess', { count: result.deletedCount }));
    }

    if (result.failedItems.length > 0 && shouldShowVisibleFeedback(visibleContextId)) {
      const firstFailure = result.failedItems[0];
      const errorSummary = result.failedItems.length === 1
        ? firstFailure.error
        : t('sessionManager.bulkDeletePartialFailure', { count: result.failedItems.length, error: firstFailure.error });
      message.error(errorSummary || t('common.error'));
    }

    if (result.failedItems.length === 0) {
      exitSelectionMode();
      return result;
    }

    setSelectedSourcePaths((current) => current.filter((sourcePath) => failedSourcePathSet.has(sourcePath)));
    return result;
  };

  const performBulkExportSessions = async (
    exportDir: string,
    visibleContextId: number,
  ): Promise<ExportToolSessionsResult> => {
    const result = await exportToolSessions(tool, selectedSourcePaths, exportDir);
    const failedSourcePathSet = new Set(result.failedItems.map((item) => item.sourcePath));

    if (result.exportedCount > 0 && shouldShowVisibleFeedback(visibleContextId)) {
      message.success(t('sessionManager.bulkExportSuccess', { count: result.exportedCount }));
    }

    if (result.failedItems.length > 0 && shouldShowVisibleFeedback(visibleContextId)) {
      const firstFailure = result.failedItems[0];
      const errorSummary = result.failedItems.length === 1
        ? firstFailure.error
        : t('sessionManager.bulkExportPartialFailure', { count: result.failedItems.length, error: firstFailure.error });
      message.error(errorSummary || t('common.error'));
    }

    if (result.failedItems.length === 0) {
      exitSelectionMode();
      return result;
    }

    setSelectedSourcePaths((current) => current.filter((sourcePath) => failedSourcePathSet.has(sourcePath)));
    return result;
  };

  const handleBulkExportSessions = async () => {
    if (selectedSourcePaths.length === 0) {
      return;
    }

    let selectedExportDir: string | null = null;

    try {
      const selected = await open({
        multiple: false,
        directory: true,
        title: t('sessionManager.bulkExportDialogTitle'),
      });

      if (!selected || Array.isArray(selected)) {
        return;
      }

      selectedExportDir = selected;
    } catch (error) {
      if (!shouldShowVisibleFeedback()) {
        return;
      }
      const errorMessage = error instanceof Error ? error.message : String(error);
      message.error(errorMessage || t('common.error'));
      return;
    }

    const visibleContextId = captureVisibleContextId();

    try {
      setBulkExporting(true);
      await performBulkExportSessions(selectedExportDir, visibleContextId);
    } catch (error) {
      if (!shouldShowVisibleFeedback(visibleContextId)) {
        return;
      }
      const errorMessage = error instanceof Error ? error.message : String(error);
      message.error(errorMessage || t('common.error'));
    } finally {
      setBulkExporting(false);
    }
  };

  const handleBulkDeleteSessions = () => {
    if (selectedSourcePaths.length === 0) {
      return;
    }

    const previewTitles = items
      .filter((session) => selectedSourcePaths.includes(session.sourcePath))
      .slice(0, 5)
      .map((session) => formatSessionTitle(session))
      .join('、');

    Modal.confirm({
      title: t('sessionManager.bulkDeleteConfirmTitle', { count: selectedSourcePaths.length }),
      content: previewTitles
        ? t('sessionManager.bulkDeleteConfirmContentWithPreview', {
          count: selectedSourcePaths.length,
          titles: previewTitles,
        })
        : t('sessionManager.bulkDeleteConfirmContent', { count: selectedSourcePaths.length }),
      icon: <ExclamationCircleOutlined />,
      okText: t('common.delete'),
      okButtonProps: { danger: true },
      cancelText: t('common.cancel'),
      onOk: async () => {
        const visibleContextId = captureVisibleContextId();
        try {
          setBulkDeleting(true);
          await performBulkDeleteSessions(visibleContextId);
        } catch (error) {
          if (!shouldShowVisibleFeedback(visibleContextId)) {
            return;
          }
          const errorMessage = error instanceof Error ? error.message : String(error);
          message.error(errorMessage || t('common.error'));
        } finally {
          setBulkDeleting(false);
        }
      },
    });
  };

  const handleDeleteSession = (session: SessionMeta) => {
    Modal.confirm({
      title: t('sessionManager.deleteConfirmTitle', { title: formatSessionTitle(session) }),
      content: t('sessionManager.deleteConfirmContent'),
      icon: <ExclamationCircleOutlined />,
      okText: t('common.delete'),
      okButtonProps: { danger: true },
      cancelText: t('common.cancel'),
      onOk: async () => {
        const visibleContextId = captureVisibleContextId();
        try {
          await performDeleteSession(session, visibleContextId);
        } catch (error) {
          if (!shouldShowVisibleFeedback(visibleContextId)) {
            return;
          }
          const errorMessage = error instanceof Error ? error.message : String(error);
          message.error(errorMessage || t('common.error'));
        }
      },
    });
  };

  const renderRuntimeSourceTag = (session: SessionMeta) => {
    if (!showRuntimeSourceTag || !session.runtimeSource) {
      return null;
    }

    const label = session.runtimeSource === 'wsl'
      ? session.runtimeDistro
        ? t('sessionManager.sourceMode.wslWithDistro', { distro: session.runtimeDistro })
        : t('sessionManager.sourceMode.wsl')
      : t('sessionManager.sourceMode.local');

    return (
      <Tag
        bordered={false}
        className={session.runtimeSource === 'wsl' ? styles.runtimeSourceTagWsl : styles.runtimeSourceTagLocal}
      >
        {label}
      </Tag>
    );
  };

  return (
    <>
      <div>
        <div className={styles.toolbar}>
          <div className={styles.toolbarMain}>
            <div className={styles.toolbarLeft}>
              <Input
                allowClear
                className={styles.searchInput}
                prefix={<SearchOutlined />}
                placeholder={t('sessionManager.searchPlaceholder')}
                value={query}
                onChange={(event) => setQuery(event.target.value)}
              />
              <Select
                allowClear
                showSearch={{ optionFilterProp: 'label' }}
                className={styles.pathFilterSelect}
                placeholder={t('sessionManager.pathFilterPlaceholder')}
                loading={pathOptionsLoading}
                value={pathFilter || (pathOptions.length > 0 ? ALL_PATHS_VALUE : undefined)}
                onChange={(value) => setPathFilter(value === ALL_PATHS_VALUE ? '' : (value ?? ''))}
                options={pathOptions}
              />
            </div>
            <Text className={styles.summaryText}>
              {t('sessionManager.totalSessions', { count: total })}
            </Text>
          </div>
          <Button
            type="link"
            size="small"
            className={styles.actionButton}
            icon={selectionMode ? <CloseOutlined /> : <CheckOutlined />}
            onClick={handleSelectionModeToggle}
          >
            {selectionMode ? t('sessionManager.cancelSelection') : t('sessionManager.select')}
          </Button>
          {selectionMode ? (
            <>
              <Button
                type="link"
                size="small"
                className={styles.actionButton}
                icon={<CheckOutlined />}
                onClick={handleSelectAllCurrentPage}
              >
                {t('sessionManager.selectLoaded')}
              </Button>
              <Button
                type="link"
                size="small"
                className={styles.actionButton}
                icon={<ExportOutlined />}
                disabled={selectedSourcePaths.length === 0}
                loading={bulkExporting}
                onClick={() => void handleBulkExportSessions()}
              >
                {t('sessionManager.bulkExport', { count: selectedSourcePaths.length })}
              </Button>
              <Button
                type="link"
                size="small"
                danger
                className={styles.actionButton}
                icon={<DeleteOutlined />}
                disabled={selectedSourcePaths.length === 0}
                loading={bulkDeleting}
                onClick={handleBulkDeleteSessions}
              >
                {t('sessionManager.bulkDelete', { count: selectedSourcePaths.length })}
              </Button>
            </>
          ) : null}
          {!selectionMode ? (
            <>
              <Button
                type="link"
                size="small"
                className={styles.actionButton}
                icon={<ReloadOutlined />}
                onClick={() => void handleRefresh()}
              >
                {t('common.refresh')}
              </Button>
              <Button
                type="link"
                size="small"
                className={styles.actionButton}
                icon={<ImportOutlined />}
                onClick={() => void handleImportSession()}
                loading={importing}
              >
                {t('sessionManager.import')}
              </Button>
            </>
          ) : null}
        </div>

        <Spin spinning={loading}>
          {items.length === 0 ? (
            <div className={styles.emptyState}>
              <Empty description={t(debouncedQuery || pathFilter ? 'sessionManager.emptyFiltered' : 'sessionManager.empty')} />
              {(debouncedQuery || pathFilter) ? (
                <Text className={styles.emptyHint}>
                  {t('sessionManager.emptyFilteredHint')}
                </Text>
              ) : null}
            </div>
          ) : (
            <div className={styles.list}>
              {items.map((session) => {
                const displayTime = session.lastActiveAt || session.createdAt;
                const selected = selectedSourcePaths.includes(session.sourcePath);
                return (
                  <div
                    key={`${session.providerId}-${session.sessionId}-${session.sourcePath}`}
                    className={`${styles.sessionCard}${selected ? ` ${styles.sessionCardSelected}` : ''}`}
                    onClick={() => {
                      if (selectionMode) {
                        toggleSessionSelection(session);
                        return;
                      }

                      handleOpenDetail(session);
                    }}
                  >
                    <div className={styles.sessionHeader}>
                      {selectionMode ? (
                        <Checkbox
                          className={styles.sessionCheckbox}
                          checked={selected}
                          onChange={() => toggleSessionSelection(session)}
                          onClick={(event) => event.stopPropagation()}
                        />
                      ) : null}
                      <div className={styles.sessionHeaderMain}>
                        <div className={styles.sessionTitleRow}>
                          <span className={styles.sessionTitle}>
                            {formatSessionTitle(session)}
                          </span>
                        </div>
                        <div className={styles.sessionMetaRow}>
                          <span><ClockCircleOutlined style={{ marginRight: 4 }} />{formatRelativeTime(displayTime, t)}</span>
                          {renderRuntimeSourceTag(session)}
                          <span>{shortSessionId(session.sessionId)}</span>
                          {session.projectDir ? (
                            <span><FolderOpenOutlined style={{ marginRight: 4 }} />{session.projectDir}</span>
                          ) : null}
                        </div>
                      </div>
                      <div className={styles.sessionActions} onClick={(event) => event.stopPropagation()}>
                        <Button
                          type="link"
                          size="small"
                          className={styles.actionButton}
                          icon={<CopyOutlined />}
                          disabled={!session.resumeCommand}
                          onClick={() => {
                            if (!session.resumeCommand) {
                              return;
                            }
                            void handleCopyText(session.resumeCommand, t('sessionManager.copyResumeSuccess'));
                          }}
                        >
                          {t('sessionManager.copyResume')}
                        </Button>
                        <Button
                          type="link"
                          size="small"
                          danger
                          className={styles.actionButton}
                          icon={<DeleteOutlined />}
                          disabled={selectionMode}
                          onClick={() => {
                            handleDeleteSession(session);
                          }}
                        >
                          {t('common.delete')}
                        </Button>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </Spin>

        <div ref={sentinelRef} className={styles.sentinel} />
        {(hasMore || loadingMore) ? (
          <div className={styles.loadMore}>
            <Button
              loading={loadingMore}
              disabled={loading || loadingMore}
              onClick={() => void loadSessions(page + 1, true)}
            >
              {t('sessionManager.loadMore')}
            </Button>
          </div>
        ) : null}
      </div>

    </>
  );
};

const SessionManagerPanel: React.FC<SessionManagerPanelProps> = ({
  tool,
  translationKey = 'sessionManager.title',
  expandNonce = 0,
  refreshNonce = 0,
  extra,
}) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = React.useState(false);
  const [sourceMode, setSourceMode] = React.useState<SessionSourceMode>(() => rememberedSessionSourceMode);
  const [availableSources, setAvailableSources] = React.useState<SessionSourceOption[]>([]);

  const handleAvailableSourcesChange = React.useCallback((sources: SessionSourceOption[]) => {
    setAvailableSources(sources);
  }, []);

  const hasLocalSource = availableSources.some((item) => item.source === 'local');
  const hasWslSource = availableSources.some((item) => item.source === 'wsl');
  const showSourceSwitcher = hasLocalSource && hasWslSource;
  const effectiveSourceMode = showSourceSwitcher ? sourceMode : 'all';

  const handleSourceModeChange = React.useCallback((value: string | number) => {
    const nextSourceMode = value as SessionSourceMode;
    rememberedSessionSourceMode = nextSourceMode;
    setSourceMode(nextSourceMode);
  }, []);

  React.useEffect(() => {
    if (expandNonce <= 0) {
      return;
    }

    setExpanded(true);
  }, [expandNonce]);

  const sourceModeOptions = React.useMemo(() => [
    { label: t('sessionManager.sourceMode.all'), value: 'all' as const },
    { label: t('sessionManager.sourceMode.local'), value: 'local' as const },
    { label: t('sessionManager.sourceMode.wsl'), value: 'wsl' as const },
  ], [t]);

  const sourceSwitcher = showSourceSwitcher ? (
    <div className={styles.sourceSegmented} role="tablist" aria-label={t('sessionManager.title')}>
      {sourceModeOptions.map((option) => {
        const selected = sourceMode === option.value;
        return (
          <button
            key={option.value}
            type="button"
            role="tab"
            aria-selected={selected}
            className={`${styles.sourceSegmentButton}${selected ? ` ${styles.sourceSegmentButtonActive}` : ''}`}
            onClick={() => handleSourceModeChange(option.value)}
          >
            <span>{option.label}</span>
          </button>
        );
      })}
    </div>
  ) : null;

  const headerExtra = sourceSwitcher || extra ? (
    <div
      className={styles.headerExtra}
      onClick={(event) => event.stopPropagation()}
      onMouseDown={(event) => event.stopPropagation()}
    >
      {extra}
      {sourceSwitcher}
    </div>
  ) : null;

  return (
    <Collapse
      className={styles.collapseCard}
      destroyOnHidden
      activeKey={expanded ? ['session-manager'] : []}
      onChange={(keys) => {
        const nextExpanded = keys.includes('session-manager');
        setExpanded(nextExpanded);
      }}
      items={[
        {
          key: 'session-manager',
          label: (
            <Text strong>
              <MessageOutlined style={{ marginRight: 8 }} />
              {t(translationKey)}
            </Text>
          ),
          extra: headerExtra,
          children: (
            <SessionManagerContent
              tool={tool}
              expanded={expanded}
              refreshNonce={refreshNonce}
              sourceMode={effectiveSourceMode}
              showRuntimeSourceTag={showSourceSwitcher && effectiveSourceMode === 'all'}
              onAvailableSourcesChange={handleAvailableSourcesChange}
            />
          ),
        },
      ]}
    />
  );
};

export default SessionManagerPanel;
