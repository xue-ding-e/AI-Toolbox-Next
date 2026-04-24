import React from 'react';
import {
  CheckOutlined,
  CloseOutlined,
  ClockCircleOutlined,
  CopyOutlined,
  DeleteOutlined,
  DownloadOutlined,
  EditOutlined,
  ExclamationCircleOutlined,
  ImportOutlined,
  FolderOpenOutlined,
  MessageOutlined,
  ReloadOutlined,
  SearchOutlined,
  UnorderedListOutlined,
} from '@ant-design/icons';
import {
  Button,
  Checkbox,
  Collapse,
  Drawer,
  Empty,
  Form,
  Input,
  Modal,
  Select,
  Space,
  Spin,
  Tag,
  Typography,
  message,
} from 'antd';
import { useTranslation } from 'react-i18next';
import { open, save } from '@tauri-apps/plugin-dialog';

import {
  deleteToolSessions,
  deleteToolSession,
  exportToolSession,
  getToolSessionDetail,
  importToolSession,
  listToolSessions,
  renameToolSession,
} from './sessionManagerApi';
import type {
  DeleteToolSessionsResult,
  SessionDetail,
  SessionMessage,
  SessionMeta,
  SessionPathOption,
  SessionTocItem,
  SessionTool,
} from './types';
import {
  advanceVisibleContextId,
  buildSessionTocItems,
  formatRelativeTime,
  formatSessionTitle,
  formatTimestamp,
  getRoleLabel,
  shortSessionId,
  shouldCollapseMessage,
  shouldShowVisibleFeedback as shouldShowVisibleFeedbackForContext,
  usesCompactMessageCollapse,
} from './utils';
import { useKeepAlive } from '@/components/layout/KeepAliveOutlet';
import styles from './SessionManagerPanel.module.less';

const { Text } = Typography;

interface SessionManagerPanelProps {
  tool: SessionTool;
  translationKey?: string;
  expandNonce?: number;
}

const PAGE_SIZE = 10;
const ALL_PATHS_VALUE = '__all_paths__';

interface SessionManagerContentProps {
  tool: SessionTool;
  expanded: boolean;
}

const SessionManagerContent: React.FC<SessionManagerContentProps> = ({
  tool,
  expanded,
}) => {
  const { t } = useTranslation();
  const { isActive } = useKeepAlive();
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
  const [detailOpen, setDetailOpen] = React.useState(false);
  const [detailLoading, setDetailLoading] = React.useState(false);
  const [exporting, setExporting] = React.useState(false);
  const [importing, setImporting] = React.useState(false);
  const [detail, setDetail] = React.useState<SessionDetail | null>(null);
  const [detailQuery, setDetailQuery] = React.useState('');
  const [renameModalOpen, setRenameModalOpen] = React.useState(false);
  const [renaming, setRenaming] = React.useState(false);
  const [mobileTocOpen, setMobileTocOpen] = React.useState(false);
  const [activeMessageIndex, setActiveMessageIndex] = React.useState<number | null>(null);
  const [selectionMode, setSelectionMode] = React.useState(false);
  const [selectedSourcePaths, setSelectedSourcePaths] = React.useState<string[]>([]);
  const [bulkDeleting, setBulkDeleting] = React.useState(false);
  const messageRefs = React.useRef<Map<number, HTMLDivElement>>(new Map());
  const [expandedMessages, setExpandedMessages] = React.useState<Record<number, boolean>>({});
  const listContextIdRef = React.useRef(0);
  const listReplaceRequestIdRef = React.useRef(0);
  const listAppendRequestIdRef = React.useRef(0);
  const detailRequestIdRef = React.useRef(0);
  const activePageRef = React.useRef(isActive);
  const visibleContextIdRef = React.useRef(0);
  const [renameForm] = Form.useForm<{ title: string }>();
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
  }, [expanded]);

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
    t,
    tool,
  ]);

  React.useEffect(() => {
    if (!expanded) {
      return;
    }
    void loadSessions(1, false);
  }, [expanded, debouncedQuery, loadSessions, pathFilter]);

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

  const tocItems = React.useMemo<SessionTocItem[]>(() => {
    return buildSessionTocItems(detail?.messages ?? []);
  }, [detail?.messages]);

  const detailSummary = detail?.meta.summary?.trim() || t('sessionManager.noSummary');
  const totalMessageCount = detail?.messages.length ?? 0;

  const filteredMessages = React.useMemo(() => {
    const messages = detail?.messages ?? [];
    const normalizedQuery = detailQuery.trim().toLowerCase();
    if (!normalizedQuery) {
      return messages.map((message, index) => ({ message, index }));
    }

    return messages
      .map((message, index) => ({ message, index }))
      .filter(({ message }) => {
        return (
          message.content.toLowerCase().includes(normalizedQuery)
          || message.role.toLowerCase().includes(normalizedQuery)
        );
      });
  }, [detail?.messages, detailQuery]);

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

  const resetDetailState = React.useCallback(() => {
    detailRequestIdRef.current += 1;
    setDetail(null);
    setDetailLoading(false);
    setDetailQuery('');
    setExpandedMessages({});
    setMobileTocOpen(false);
    setActiveMessageIndex(null);
    setRenameModalOpen(false);
    setRenaming(false);
    setImporting(false);
    setExporting(false);
    renameForm.resetFields();
    messageRefs.current.clear();
  }, [renameForm]);

  const fetchSessionDetail = React.useCallback(async (session: SessionMeta) => {
    const visibleContextId = captureVisibleContextId();
    const requestId = detailRequestIdRef.current + 1;
    detailRequestIdRef.current = requestId;

    try {
      const result = await getToolSessionDetail(tool, session.sourcePath);
      if (requestId !== detailRequestIdRef.current) {
        return;
      }
      if (!shouldShowVisibleFeedback(visibleContextId)) {
        return;
      }
      setDetail(result);
      setExpandedMessages({});
      setDetailQuery('');
      setMobileTocOpen(false);
      setActiveMessageIndex(null);
      messageRefs.current.clear();
    } catch (error) {
      if (requestId !== detailRequestIdRef.current) {
        return;
      }
      if (!shouldShowVisibleFeedback(visibleContextId)) {
        return;
      }
      const errorMessage = error instanceof Error ? error.message : String(error);
      message.error(errorMessage || t('common.error'));
    }
  }, [captureVisibleContextId, shouldShowVisibleFeedback, t, tool]);

  const handleOpenDetail = async (session: SessionMeta) => {
    setDetailOpen(true);
    setDetail(null);
    setDetailLoading(true);

    try {
      await fetchSessionDetail(session);
    } finally {
      setDetailLoading(false);
    }
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

  const buildSessionExportFileName = (session: SessionMeta) => {
    return `${tool}-session-${session.sessionId}.json`;
  };

  const exportSessionDetail = async (sessionDetail: SessionDetail) => {
    const exportMessageKey = `session-export-${tool}`;
    const visibleContextId = captureVisibleContextId();
    try {
      const exportPath = await save({
        title: t('sessionManager.exportDialogTitle'),
        defaultPath: buildSessionExportFileName(sessionDetail.meta),
        filters: [
          {
            name: 'JSON',
            extensions: ['json'],
          },
        ],
      });

      if (!exportPath) {
        return;
      }

      setExporting(true);
      if (shouldShowVisibleFeedback(visibleContextId)) {
        message.open({
          key: exportMessageKey,
          type: 'loading',
          content: t('sessionManager.exporting'),
          duration: 0,
        });
      }
      await exportToolSession(tool, sessionDetail.meta.sourcePath, exportPath);
      if (shouldShowVisibleFeedback(visibleContextId)) {
        message.success({
          key: exportMessageKey,
          content: t('sessionManager.exportSuccess'),
        });
      } else {
        message.destroy(exportMessageKey);
      }
    } catch (error) {
      if (!shouldShowVisibleFeedback(visibleContextId)) {
        message.destroy(exportMessageKey);
        return;
      }
      const errorMessage = error instanceof Error ? error.message : String(error);
      message.error({
        key: exportMessageKey,
        content: errorMessage || t('common.error'),
      });
    } finally {
      setExporting(false);
    }
  };

  const performDeleteSession = async (session: SessionMeta, visibleContextId: number) => {
    await deleteToolSession(tool, session.sourcePath);

    if (detail?.meta.sourcePath === session.sourcePath) {
      resetDetailState();
      setDetailOpen(false);
    }

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

    if (
      detail
      && selectedSourcePaths.includes(detail.meta.sourcePath)
      && !failedSourcePathSet.has(detail.meta.sourcePath)
    ) {
      resetDetailState();
      setDetailOpen(false);
    }

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

  const canRenameSession = tool === 'opencode' || tool === 'codex';

  const openRenameModal = () => {
    if (!detail || !canRenameSession) {
      return;
    }
    renameForm.setFieldsValue({
      title: detail.meta.title?.trim() || '',
    });
    setRenameModalOpen(true);
  };

  const handleRenameSession = async () => {
    if (!detail || !canRenameSession) {
      return;
    }

    const visibleContextId = captureVisibleContextId();
    try {
      const values = await renameForm.validateFields();
      setRenaming(true);
      await renameToolSession(tool, detail.meta.sourcePath, values.title);
      if (shouldShowVisibleFeedback(visibleContextId)) {
        message.success(t('sessionManager.renameSuccess'));
      }
      setRenameModalOpen(false);
      await Promise.all([
        fetchSessionDetail(detail.meta),
        loadSessions(1, false, true),
      ]);
    } catch (error) {
      if (!shouldShowVisibleFeedback(visibleContextId)) {
        return;
      }
      if (error instanceof Error) {
        message.error(error.message || t('common.error'));
      } else if (!('errorFields' in (error as object))) {
        message.error(String(error) || t('common.error'));
      }
    } finally {
      setRenaming(false);
    }
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

  const scrollToMessage = (index: number) => {
    const target = messageRefs.current.get(index);
    if (!target) {
      return;
    }

    target.scrollIntoView({ behavior: 'smooth', block: 'center' });
    setActiveMessageIndex(index);
    setMobileTocOpen(false);
    window.setTimeout(() => {
      setActiveMessageIndex((current) => (current === index ? null : current));
    }, 1800);
  };

  const getMessageCardRoleClassName = (role: string) => {
    switch (role.toLowerCase()) {
      case 'user':
        return styles.messageCardUser;
      case 'assistant':
        return styles.messageCardAssistant;
      case 'tool':
        return styles.messageCardTool;
      case 'system':
        return styles.messageCardSystem;
      default:
        return '';
    }
  };

  const getMessageRoleTagClassName = (role: string) => {
    switch (role.toLowerCase()) {
      case 'user':
        return styles.messageRoleTagUser;
      case 'assistant':
        return styles.messageRoleTagAssistant;
      case 'tool':
        return styles.messageRoleTagTool;
      case 'system':
        return styles.messageRoleTagSystem;
      default:
        return '';
    }
  };

  const renderMessage = (messageItem: SessionMessage, index: number) => {
    const isCollapsible = shouldCollapseMessage(messageItem.role, messageItem.content);
    const isExpanded = expandedMessages[index] ?? false;
    const useCompactCollapse = usesCompactMessageCollapse(messageItem.role);
    const messageRoleClassName = getMessageCardRoleClassName(messageItem.role);
    const messageRoleTagClassName = getMessageRoleTagClassName(messageItem.role);
    const messageOrder = index + 1;

    return (
      <div
        key={`${index}-${messageItem.ts ?? 'no-ts'}`}
        ref={(node) => {
          if (node) {
            messageRefs.current.set(index, node);
          } else {
            messageRefs.current.delete(index);
          }
        }}
        className={`${styles.messageCard}${messageRoleClassName ? ` ${messageRoleClassName}` : ''}${activeMessageIndex === index ? ` ${styles.messageCardActive}` : ''}`}
      >
        <div className={styles.messageRail}>
          <div className={styles.messageNode}>
            <span>{messageOrder}</span>
          </div>
          <div className={styles.messageLine} />
        </div>
        <div className={styles.messageHeader}>
          <div className={styles.messageHeaderLeft}>
            <Tag
              bordered={false}
              className={`${styles.messageRoleTag}${messageRoleTagClassName ? ` ${messageRoleTagClassName}` : ''}`}
            >
              {getRoleLabel(messageItem.role, t)}
            </Tag>
            {messageItem.ts ? <Text className={styles.messageTimestamp}>{formatTimestamp(messageItem.ts)}</Text> : null}
          </div>
          <Button
            size="small"
            type="link"
            className={styles.messageCopyButton}
            icon={<CopyOutlined />}
            onClick={() => void handleCopyText(messageItem.content, t('sessionManager.copyMessageSuccess'))}
          >
            {t('common.copy')}
          </Button>
        </div>
        <div
          className={`${styles.messageContent}${isCollapsible && !isExpanded ? ` ${useCompactCollapse ? styles.messageCollapsedCompact : styles.messageCollapsed}` : ''}`}
        >
          {messageItem.content}
        </div>
        {isCollapsible ? (
          <div className={styles.messageFooter}>
            <Button
              type="link"
              size="small"
              className={styles.messageExpandButton}
              onClick={() => {
                setExpandedMessages((current) => ({
                  ...current,
                  [index]: !isExpanded,
                }));
              }}
            >
              {isExpanded ? t('sessionManager.collapseMessage') : t('sessionManager.expandMessage')}
            </Button>
          </div>
        ) : null}
      </div>
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
                showSearch
                optionFilterProp="label"
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

                      void handleOpenDetail(session);
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

      <Modal
        open={detailOpen}
        onCancel={() => {
          resetDetailState();
          setDetailOpen(false);
        }}
        width={1080}
        className={styles.detailModal}
        footer={null}
        destroyOnHidden
        title={null}
      >
        <Spin spinning={detailLoading}>
          {detail ? (
            <div className={styles.detailShell}>
              <div className={styles.detailHero}>
                <div className={styles.detailHeroTop}>
                  <div className={styles.detailHeroMain}>
                    <div className={styles.detailHeroTitle}>{formatSessionTitle(detail.meta)}</div>
                    <div className={styles.detailHeroSummary}>{detailSummary}</div>
                  </div>
                  <Space wrap className={styles.detailHeroActions}>
                    {canRenameSession ? (
                      <Button
                        className={styles.detailPrimaryAction}
                        icon={<EditOutlined />}
                        onClick={openRenameModal}
                      >
                        {t('sessionManager.rename')}
                      </Button>
                    ) : null}
                    <Button
                      className={styles.detailSecondaryAction}
                      icon={<DownloadOutlined />}
                      loading={exporting}
                      disabled={exporting}
                      onClick={() => void exportSessionDetail(detail)}
                    >
                      {t(exporting ? 'sessionManager.exporting' : 'sessionManager.export')}
                    </Button>
                    <Button
                      className={styles.detailSecondaryAction}
                      icon={<CopyOutlined />}
                      disabled={!detail.meta.resumeCommand}
                      onClick={() => {
                        if (!detail.meta.resumeCommand) {
                          return;
                        }
                        void handleCopyText(detail.meta.resumeCommand, t('sessionManager.copyResumeSuccess'));
                      }}
                    >
                      {t('sessionManager.copyResume')}
                    </Button>
                    <Button
                      danger
                      className={styles.detailSecondaryAction}
                      icon={<DeleteOutlined />}
                      onClick={() => {
                        handleDeleteSession(detail.meta);
                      }}
                    >
                      {t('common.delete')}
                    </Button>
                  </Space>
                </div>

                <div className={styles.detailMetaGrid}>
                  <div className={styles.detailMetaCard}>
                    <span className={styles.detailMetaLabel}>{t('sessionManager.sessionId')}</span>
                    <div className={`${styles.detailMetaValue} ${styles.detailMetaMono}`}>{detail.meta.sessionId}</div>
                  </div>
                  {detail.meta.projectDir ? (
                    <div className={styles.detailMetaCard}>
                      <span className={styles.detailMetaLabel}>{t('sessionManager.projectDir')}</span>
                      <div className={styles.detailMetaValue}>{detail.meta.projectDir}</div>
                    </div>
                  ) : null}
                  {detail.meta.createdAt ? (
                    <div className={styles.detailMetaCard}>
                      <span className={styles.detailMetaLabel}>{t('sessionManager.createdAt')}</span>
                      <div className={styles.detailMetaValue}>{formatTimestamp(detail.meta.createdAt)}</div>
                    </div>
                  ) : null}
                  {detail.meta.lastActiveAt ? (
                    <div className={styles.detailMetaCard}>
                      <span className={styles.detailMetaLabel}>{t('sessionManager.lastActiveAt')}</span>
                      <div className={styles.detailMetaValue}>{formatTimestamp(detail.meta.lastActiveAt)}</div>
                    </div>
                  ) : null}
                </div>
              </div>

              <div className={styles.detailLayout}>
              <div className={styles.tocPane}>
                <div className={styles.tocHeader}>
                  <Text strong>{t('sessionManager.tocTitle')}</Text>
                  <span className={styles.tocCount}>{tocItems.length}</span>
                </div>
                <div className={styles.tocList}>
                  {tocItems.length === 0 ? (
                    <Text type="secondary">{t('sessionManager.tocEmpty')}</Text>
                  ) : tocItems.map((item, tocIndex) => (
                    <button
                      key={`${item.index}-${tocIndex}`}
                      type="button"
                      className={styles.tocButton}
                      onClick={() => scrollToMessage(item.index)}
                    >
                      <div className={styles.tocIndex}>{tocIndex + 1}</div>
                      <div className={styles.tocPreview}>{item.preview}</div>
                    </button>
                  ))}
                </div>
              </div>

              <div className={styles.detailMain}>
                <div className={styles.detailToolbar}>
                  <div className={styles.detailToolbarLeft}>
                    <Input
                      allowClear
                      className={styles.detailSearchInput}
                      prefix={<SearchOutlined />}
                      placeholder={t('sessionManager.searchInSession')}
                      value={detailQuery}
                      onChange={(event) => setDetailQuery(event.target.value)}
                    />
                    <Button
                      className={styles.mobileTocButton}
                      icon={<UnorderedListOutlined />}
                      onClick={() => setMobileTocOpen(true)}
                    >
                      {t('sessionManager.tocTitle')}
                    </Button>
                  </div>
                  <span className={styles.detailCountBadge}>
                    <MessageOutlined />
                    {totalMessageCount}
                  </span>
                </div>

                <div className={styles.messagesPanel}>
                  <div className={styles.messagesList}>
                    {filteredMessages.length === 0 ? (
                      <Empty description={t('sessionManager.noMessagesMatched')} />
                    ) : filteredMessages.map(({ message: messageItem, index }) => renderMessage(messageItem, index))}
                  </div>
                </div>
              </div>
            </div>
            </div>
          ) : (
            <Empty description={t('sessionManager.emptyDetail')} />
          )}
        </Spin>
      </Modal>

      <Drawer
        open={mobileTocOpen}
        onClose={() => setMobileTocOpen(false)}
        title={t('sessionManager.tocTitle')}
        placement="right"
      >
        <div className={styles.tocList}>
          {tocItems.length === 0 ? (
            <Text type="secondary">{t('sessionManager.tocEmpty')}</Text>
          ) : tocItems.map((item, tocIndex) => (
            <button
              key={`${item.index}-${tocIndex}-drawer`}
              type="button"
              className={styles.tocButton}
              onClick={() => scrollToMessage(item.index)}
            >
              <div className={styles.tocIndex}>{tocIndex + 1}</div>
              <div className={styles.tocPreview}>{item.preview}</div>
            </button>
          ))}
        </div>
      </Drawer>

      <Modal
        open={renameModalOpen}
        title={t('sessionManager.renameTitle')}
        okText={t('common.save')}
        cancelText={t('common.cancel')}
        onOk={() => void handleRenameSession()}
        confirmLoading={renaming}
        onCancel={() => {
          setRenameModalOpen(false);
          renameForm.resetFields();
        }}
        destroyOnHidden
      >
        <Form form={renameForm} layout="horizontal" labelCol={{ span: 5 }} wrapperCol={{ span: 19 }}>
          <Form.Item
            label={t('sessionManager.renameField')}
            name="title"
            rules={[
              {
                required: true,
                whitespace: true,
                message: t('sessionManager.renameRequired'),
              },
            ]}
          >
            <Input maxLength={200} placeholder={t('sessionManager.renamePlaceholder')} />
          </Form.Item>
        </Form>
      </Modal>
    </>
  );
};

const SessionManagerPanel: React.FC<SessionManagerPanelProps> = ({
  tool,
  translationKey = 'sessionManager.title',
  expandNonce = 0,
}) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = React.useState(false);

  React.useEffect(() => {
    if (expandNonce <= 0) {
      return;
    }

    setExpanded(true);
  }, [expandNonce]);

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
          children: <SessionManagerContent tool={tool} expanded={expanded} />,
        },
      ]}
    />
  );
};

export default SessionManagerPanel;
