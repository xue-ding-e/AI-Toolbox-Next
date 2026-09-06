import React from 'react';
import {
  App,
  Button,
  Empty,
  Input,
  Modal,
  Popconfirm,
  Spin,
  Table,
  Tag,
  Tooltip,
  Typography,
} from 'antd';
import {
  DeleteOutlined,
  EditOutlined,
  FileAddOutlined,
  FileTextOutlined,
  FolderOpenOutlined,
  FolderOutlined,
  RedoOutlined,
  RollbackOutlined,
} from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import MarkdownPreview from '@/components/common/MarkdownPreview';
import MarkdownEditor from '@/components/common/MarkdownEditor';
import PlainTextEditor from '@/components/common/PlainTextEditor';
import {
  clearCodexMemories,
  deleteCodexMemoryEntries,
  listCodexMemories,
  readCodexMemoryFile,
  renameCodexMemoryEntry,
  revealCodexMemoriesFolder,
  writeCodexMemoryFile,
} from '@/services/codexApi';
import type {
  CodexMemoriesEntry,
  CodexMemoriesListResult,
  CodexMemoriesSourceMode,
} from '@/types/codex';
import styles from './CodexMemoriesPanel.module.less';

const { Text } = Typography;

interface CodexMemoriesPanelProps {
  refreshToken?: number;
}

// Remembered across panel remounts within the page lifetime, matching the
// session manager source-mode behavior on the Codex page.
let rememberedMemoriesSourceMode: CodexMemoriesSourceMode = 'local';

interface BreadcrumbSegment {
  label: string;
  path: string;
}

const AUTO_REGENERATED_RELATIVE_PATHS = [
  'MEMORY.md',
  'memory_summary.md',
  'raw_memories.md',
];

const isAutoRegeneratedPath = (relativePath: string): boolean => {
  return (
    AUTO_REGENERATED_RELATIVE_PATHS.includes(relativePath) ||
    relativePath.startsWith('rollout_summaries/')
  );
};

const formatMemorySize = (size: number): string => {
  if (size < 1024) {
    return `${size} B`;
  }
  if (size < 1024 * 1024) {
    return `${(size / 1024).toFixed(1)} KB`;
  }
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
};

const formatMemoryTime = (modifiedAtMs?: number | null): string => {
  if (!modifiedAtMs) {
    return '-';
  }
  return new Date(modifiedAtMs).toLocaleString();
};

const buildBreadcrumbSegments = (currentDir: string): BreadcrumbSegment[] => {
  const segments: BreadcrumbSegment[] = [{ label: 'memories', path: '' }];
  let accumulated = '';
  for (const part of currentDir.split('/').filter(Boolean)) {
    accumulated = accumulated ? `${accumulated}/${part}` : part;
    segments.push({ label: part, path: accumulated });
  }
  return segments;
};

const CodexMemoriesPanel: React.FC<CodexMemoriesPanelProps> = ({ refreshToken }) => {
  const { t } = useTranslation();
  const { message, modal } = App.useApp();

  const [sourceMode, setSourceMode] = React.useState<CodexMemoriesSourceMode>(
    () => rememberedMemoriesSourceMode
  );
  const [listResult, setListResult] = React.useState<CodexMemoriesListResult | null>(null);
  const [currentDir, setCurrentDir] = React.useState('');
  const [loading, setLoading] = React.useState(false);

  const [selectedFilePath, setSelectedFilePath] = React.useState<string | null>(null);
  const [fileContent, setFileContent] = React.useState<string | null>(null);
  const [fileLoading, setFileLoading] = React.useState(false);
  const [editing, setEditing] = React.useState(false);
  const [editValue, setEditValue] = React.useState('');
  const [saving, setSaving] = React.useState(false);

  const [selectedRowKeys, setSelectedRowKeys] = React.useState<React.Key[]>([]);
  const [renameTarget, setRenameTarget] = React.useState<{ path: string; name: string } | null>(
    null
  );
  const [renameValue, setRenameValue] = React.useState('');
  const [newFileModalOpen, setNewFileModalOpen] = React.useState(false);
  const [newFileName, setNewFileName] = React.useState('');

  const sourceOptions = listResult?.availableSources ?? [];
  const hasLocalSource = sourceOptions.some((option) => option.source === 'local');
  const hasWslSource = sourceOptions.some((option) => option.source === 'wsl');
  const effectiveSourceMode: CodexMemoriesSourceMode =
    sourceMode === 'local' && !hasLocalSource && hasWslSource ? 'wsl' : sourceMode;
  const sourceUnavailable = Boolean(listResult?.unavailable);

  const loadList = React.useCallback(
    async (mode: CodexMemoriesSourceMode, dir: string, silent = false) => {
      if (!silent) {
        setLoading(true);
      }
      try {
        const result = await listCodexMemories(mode, dir);
        setListResult(result);
      } catch (error) {
        setListResult(null);
        console.error('Failed to list Codex memories:', error);
        message.error(error instanceof Error ? error.message : String(error));
      } finally {
        setLoading(false);
      }
    },
    [message]
  );

  React.useEffect(() => {
    loadList(effectiveSourceMode, currentDir);
  }, [effectiveSourceMode, currentDir, refreshToken, loadList]);

  // Reset selection when the source or directory actually changes, but keep
  // the browsing position across page-driven refreshToken bumps.
  const navigationKeyRef = React.useRef(`${effectiveSourceMode}|${currentDir}`);
  React.useEffect(() => {
    const navigationKey = `${effectiveSourceMode}|${currentDir}`;
    if (navigationKeyRef.current === navigationKey) {
      return;
    }
    navigationKeyRef.current = navigationKey;
    setSelectedFilePath(null);
    setSelectedRowKeys([]);
  }, [effectiveSourceMode, currentDir]);

  React.useEffect(() => {
    if (!selectedFilePath) {
      setFileContent(null);
      setEditing(false);
      return undefined;
    }
    let cancelled = false;
    setFileLoading(true);
    readCodexMemoryFile(effectiveSourceMode, selectedFilePath)
      .then((content) => {
        if (!cancelled) {
          setFileContent(content.content);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setSelectedFilePath(null);
          setFileContent(null);
          console.error('Failed to read Codex memory file:', error);
          message.error(error instanceof Error ? error.message : String(error));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setFileLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [selectedFilePath, effectiveSourceMode, message]);

  const handleSourceModeChange = (mode: CodexMemoriesSourceMode) => {
    rememberedMemoriesSourceMode = mode;
    setSourceMode(mode);
    setCurrentDir('');
  };

  const handleEntryClick = (entry: CodexMemoriesEntry) => {
    if (entry.entryType === 'directory') {
      setCurrentDir(entry.relativePath);
      return;
    }
    setEditing(false);
    setSelectedFilePath(entry.relativePath);
  };

  const handleSave = async () => {
    if (!selectedFilePath) {
      return;
    }
    setSaving(true);
    try {
      await writeCodexMemoryFile(effectiveSourceMode, selectedFilePath, editValue);
      message.success(t('codex.memories.messages.saveSuccess'));
      setEditing(false);
      setFileContent(editValue);
      await loadList(effectiveSourceMode, currentDir, true);
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setSaving(false);
    }
  };

  const handleCreateFile = async () => {
    const trimmedName = newFileName.trim();
    if (!trimmedName) {
      return;
    }
    const relativePath = currentDir ? `${currentDir}/${trimmedName}` : trimmedName;
    try {
      await writeCodexMemoryFile(effectiveSourceMode, relativePath, '');
      message.success(t('codex.memories.messages.createSuccess'));
      setNewFileModalOpen(false);
      setNewFileName('');
      await loadList(effectiveSourceMode, currentDir, true);
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    }
  };

  const handleRename = async () => {
    if (!renameTarget) {
      return;
    }
    const trimmedName = renameValue.trim();
    if (!trimmedName || trimmedName === renameTarget.name) {
      setRenameTarget(null);
      return;
    }
    try {
      await renameCodexMemoryEntry(effectiveSourceMode, renameTarget.path, trimmedName);
      message.success(t('codex.memories.messages.renameSuccess'));
      setRenameTarget(null);
      if (selectedFilePath === renameTarget.path) {
        const parentDir = renameTarget.path.split('/').slice(0, -1).join('/');
        setSelectedFilePath(null);
        setCurrentDir(parentDir);
      }
      await loadList(effectiveSourceMode, currentDir, true);
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    }
  };

  const handleDelete = async (relativePaths: string[]) => {
    if (relativePaths.length === 0) {
      return;
    }
    try {
      await deleteCodexMemoryEntries(effectiveSourceMode, relativePaths);
      message.success(t('codex.memories.messages.deleteSuccess'));
      if (selectedFilePath && relativePaths.includes(selectedFilePath)) {
        setSelectedFilePath(null);
      }
      setSelectedRowKeys([]);
      await loadList(effectiveSourceMode, currentDir, true);
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    }
  };

  const handleClearAll = () => {
    modal.confirm({
      title: t('codex.memories.clearConfirmTitle'),
      content: t('codex.memories.clearConfirmContent'),
      okText: t('common.confirm'),
      cancelText: t('common.cancel'),
      okButtonProps: { danger: true },
      onOk: async () => {
        try {
          await clearCodexMemories(effectiveSourceMode);
          message.success(t('codex.memories.messages.clearSuccess'));
          setSelectedFilePath(null);
          setSelectedRowKeys([]);
          setCurrentDir('');
          await loadList(effectiveSourceMode, '', true);
        } catch (error) {
          message.error(error instanceof Error ? error.message : String(error));
        }
      },
    });
  };

  const handleOpenFolder = async () => {
    try {
      await revealCodexMemoriesFolder(effectiveSourceMode);
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    }
  };

  const selectedFileName = selectedFilePath?.split('/').pop() ?? '';
  const isSelectedFileMarkdown = Boolean(selectedFilePath?.endsWith('.md'));
  const showRegeneratedHint = selectedFilePath ? isAutoRegeneratedPath(selectedFilePath) : false;
  const fileContentSizeBytes = React.useMemo(
    () => (fileContent === null ? null : new Blob([fileContent]).size),
    [fileContent]
  );

  const columns = [
    {
      title: t('codex.memories.column.name'),
      dataIndex: 'name',
      key: 'name',
      render: (_: unknown, entry: CodexMemoriesEntry) => (
        <span className={styles.detailName}>
          {entry.entryType === 'directory' ? (
            <FolderOutlined className={styles.tableActionIcon} />
          ) : (
            <FileTextOutlined className={styles.tableActionIcon} />
          )}
          {entry.name}
        </span>
      ),
    },
    {
      title: t('codex.memories.column.size'),
      dataIndex: 'size',
      key: 'size',
      width: 90,
      render: (_: unknown, entry: CodexMemoriesEntry) =>
        entry.entryType === 'file' ? formatMemorySize(entry.size) : '-',
    },
    {
      title: t('codex.memories.column.modified'),
      dataIndex: 'modifiedAtMs',
      key: 'modifiedAtMs',
      width: 160,
      render: (_: unknown, entry: CodexMemoriesEntry) => formatMemoryTime(entry.modifiedAtMs),
    },
    {
      title: '',
      key: 'actions',
      width: 64,
      render: (_: unknown, entry: CodexMemoriesEntry) => (
        <span>
          <Tooltip title={t('codex.memories.actions.rename')}>
            <Button
              type="text"
              size="small"
              icon={<EditOutlined />}
              onClick={(event) => {
                event.stopPropagation();
                setRenameValue(entry.name);
                setRenameTarget({ path: entry.relativePath, name: entry.name });
              }}
            />
          </Tooltip>
          <Popconfirm
            title={t('codex.memories.deleteConfirmTitle')}
            description={
              entry.entryType === 'directory'
                ? t('codex.memories.deleteDirConfirmContent', { name: entry.name })
                : t('codex.memories.deleteConfirmContent', { name: entry.name })
            }
            okText={t('common.confirm')}
            cancelText={t('common.cancel')}
            okButtonProps={{ danger: true }}
            onConfirm={(event) => {
              event?.stopPropagation();
              handleDelete([entry.relativePath]);
            }}
            onCancel={(event) => event?.stopPropagation()}
          >
            <Button
              type="text"
              size="small"
              danger
              icon={<DeleteOutlined />}
              onClick={(event) => event.stopPropagation()}
            />
          </Popconfirm>
        </span>
      ),
    },
  ];

  const entries = listResult?.entries ?? [];
  const listEmpty = loading ? (
    <span />
  ) : sourceUnavailable ? (
    <Empty
      image={Empty.PRESENTED_IMAGE_SIMPLE}
      description={t('codex.memories.sourceUnavailableDescription')}
      style={{ padding: '32px 0' }}
    />
  ) : entries.length === 0 ? (
    <Empty
      image={Empty.PRESENTED_IMAGE_SIMPLE}
      description={t('codex.memories.emptyDescription')}
      style={{ padding: '32px 0' }}
    />
  ) : undefined;

  return (
    <div>
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 8,
          flexWrap: 'wrap',
          marginBottom: 12,
        }}
      >
        <div className={styles.sourceSegmented} role="tablist" aria-label={t('codex.memories.title')}>
          <button
            type="button"
            role="tab"
            aria-selected={effectiveSourceMode === 'local'}
            className={`${styles.sourceSegmentButton}${effectiveSourceMode === 'local' ? ` ${styles.sourceSegmentButtonActive}` : ''}`}
            disabled={!hasLocalSource || loading}
            onClick={() => handleSourceModeChange('local')}
          >
            <span>{t('sessionManager.sourceMode.local')}</span>
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={effectiveSourceMode === 'wsl'}
            className={`${styles.sourceSegmentButton}${effectiveSourceMode === 'wsl' ? ` ${styles.sourceSegmentButtonActive}` : ''}`}
            disabled={!hasWslSource || loading}
            onClick={() => handleSourceModeChange('wsl')}
          >
            <span>{t('sessionManager.sourceMode.wsl')}</span>
          </button>
        </div>
        {listResult && !sourceUnavailable && (
          <Tooltip title={listResult.rootPath}>
            <Tag style={{ marginInlineEnd: 0 }}>
              <span className={styles.rootPathText}>{listResult.rootPath}</span>
            </Tag>
          </Tooltip>
        )}
        <div style={{ flex: 1 }} />
        <Button
          size="small"
          icon={<FileAddOutlined />}
          disabled={sourceUnavailable}
          onClick={() => {
            setNewFileName('');
            setNewFileModalOpen(true);
          }}
        >
          {t('codex.memories.actions.newFile')}
        </Button>
        <Button
          size="small"
          icon={<FolderOpenOutlined />}
          disabled={sourceUnavailable}
          onClick={handleOpenFolder}
        >
          {t('codex.memories.actions.openFolder')}
        </Button>
        <Button
          size="small"
          danger
          icon={<DeleteOutlined />}
          disabled={sourceUnavailable || selectedRowKeys.length === 0}
          onClick={() => {
            const paths = selectedRowKeys.map((key) => String(key));
            modal.confirm({
              title: t('codex.memories.batchDeleteConfirmTitle', { total: paths.length }),
              content: t('codex.memories.batchDeleteConfirmContent'),
              okText: t('common.confirm'),
              cancelText: t('common.cancel'),
              okButtonProps: { danger: true },
              onOk: () => handleDelete(paths),
            });
          }}
          >
          {t('common.delete')}
        </Button>
        <Button
          size="small"
          danger
          icon={<RollbackOutlined />}
          disabled={sourceUnavailable}
          onClick={handleClearAll}
        >
          {t('codex.memories.actions.clearAll')}
        </Button>
        <Button
          size="small"
          icon={<RedoOutlined />}
          onClick={() => loadList(effectiveSourceMode, currentDir)}
        />
      </div>

      <div style={{ marginBottom: 8 }}>
        {buildBreadcrumbSegments(currentDir).map((segment, index, all) => (
          <span key={segment.path || 'root'} className={styles.breadcrumbPath}>
            {index > 0 && <span style={{ margin: '0 4px', color: 'var(--color-text-tertiary)' }}>/</span>}
            {index === all.length - 1 ? (
              <Text strong>{segment.label}</Text>
            ) : (
              <a onClick={() => setCurrentDir(segment.path)}>{segment.label}</a>
            )}
          </span>
        ))}
      </div>

      <Spin spinning={loading}>
        <Table<CodexMemoriesEntry>
          size="small"
          rowKey="relativePath"
          columns={columns}
          dataSource={entries}
          locale={{ emptyText: listEmpty }}
          pagination={false}
          scroll={{ y: 420 }}
          rowSelection={{
            selectedRowKeys,
            onChange: (keys) => setSelectedRowKeys(keys),
            getCheckboxProps: () => ({ disabled: loading }),
          }}
          onRow={(entry) => ({
            onClick: () => handleEntryClick(entry),
            style: { cursor: 'pointer' },
          })}
        />
      </Spin>

      <Modal
        open={Boolean(selectedFilePath)}
        onCancel={() => setSelectedFilePath(null)}
        width="80%"
        destroyOnHidden
        title={
          <div className={styles.modalTitleBlock}>
            <span className={styles.detailName}>{selectedFileName}</span>
            {fileContent !== null && fileContentSizeBytes !== null && (
              <span className={styles.detailMeta}>
                {`${formatMemorySize(fileContentSizeBytes)} · ${t(
                  'codex.memories.detail.characters',
                  { length: fileContent.length }
                )}`}
              </span>
            )}
          </div>
        }
        footer={
          editing
            ? [
                <Button
                  key="cancel"
                  onClick={() => {
                    setEditing(false);
                    setEditValue('');
                  }}
                >
                  {t('common.cancel')}
                </Button>,
                <Button key="save" type="primary" loading={saving} onClick={handleSave}>
                  {t('common.save')}
                </Button>,
              ]
            : [
                <Button
                  key="edit"
                  type="primary"
                  icon={<EditOutlined />}
                  onClick={() => {
                    setEditValue(fileContent ?? '');
                    setEditing(true);
                  }}
                >
                  {t('common.edit')}
                </Button>,
              ]
        }
      >
        {showRegeneratedHint && !editing && (
          <div className={styles.detailHint}>
            <Text type="secondary" style={{ fontSize: 10 }}>
              {t('codex.memories.regeneratedHint')}
            </Text>
          </div>
        )}
        <div className={editing ? styles.modalEditorBody : styles.modalPreviewBody}>
          {fileLoading ? (
            <div style={{ textAlign: 'center', padding: 48 }}>
              <Spin />
            </div>
          ) : editing ? (
            <MarkdownEditor
              value={editValue}
              onChange={setEditValue}
              height={560}
              resizable={false}
            />
          ) : isSelectedFileMarkdown ? (
            <MarkdownPreview content={fileContent} />
          ) : (
            <PlainTextEditor value={fileContent ?? ''} readOnly height={560} />
          )}
        </div>
      </Modal>

      <Modal
        title={t('codex.memories.newFileTitle')}
        open={newFileModalOpen}
        okText={t('common.confirm')}
        cancelText={t('common.cancel')}
        onOk={handleCreateFile}
        onCancel={() => setNewFileModalOpen(false)}
        destroyOnHidden
      >
        <Input
          value={newFileName}
          placeholder={t('codex.memories.newFileNamePlaceholder')}
          onChange={(event) => setNewFileName(event.target.value)}
          onPressEnter={handleCreateFile}
        />
      </Modal>

      <Modal
        title={t('codex.memories.renameTitle')}
        open={Boolean(renameTarget)}
        okText={t('common.confirm')}
        cancelText={t('common.cancel')}
        onOk={handleRename}
        onCancel={() => setRenameTarget(null)}
        destroyOnHidden
      >
        <Input
          value={renameValue}
          onChange={(event) => setRenameValue(event.target.value)}
          onPressEnter={handleRename}
        />
      </Modal>
    </div>
  );
};

export default CodexMemoriesPanel;
