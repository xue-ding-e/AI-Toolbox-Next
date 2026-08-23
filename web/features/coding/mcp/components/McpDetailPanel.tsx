import React from 'react';
import { message } from 'antd';
import {
  Code2,
  Copy,
  Globe2,
  Pencil,
  Plus,
  Power,
  PowerOff,
  Tags,
  Trash2,
  X,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { ToolIcon } from '@/features/coding/shared/toolIcon/ToolIcon';
import { formatRelativeTime } from '@/features/coding/shared/sessionManager/utils';
import type { McpServer, McpTool } from '../types';
import {
  getMcpCommandPackageVersion,
  getMcpCommandPackageVersionKey,
} from '../utils/mcpCommandPackageVersion';
import {
  hashTagColorIndex,
  normalizeTagList,
} from '../utils/mcpTags';
import styles from './McpDetailPanel.module.less';

const TAG_COLOR_CLASS_NAMES: readonly string[] = [
  styles.tagColor0,
  styles.tagColor1,
  styles.tagColor2,
  styles.tagColor3,
  styles.tagColor4,
  styles.tagColor5,
  styles.tagColor6,
  styles.tagColor7,
];
const tagPillColorClass = (tag: string): string =>
  TAG_COLOR_CLASS_NAMES[hashTagColorIndex(tag)] ?? styles.tagColor0;

interface McpDetailPanelProps {
  server: McpServer | null;
  tools: McpTool[];
  loading: boolean;
  toolsReadOnly?: boolean;
  resolvedPackageVersions?: Record<string, string>;
  /** Distinct tag names across all MCP servers, for inline autocomplete. */
  allTags?: string[];
  /** Persist an updated tag list; omit to render tags read-only. */
  onUpdateTags?: (serverId: string, nextTags: string[]) => void;
  onClose: () => void;
  onEdit: (server: McpServer) => void;
  onEditMetadata: (server: McpServer) => void;
  onDelete: (serverId: string) => void;
  onToggleTool: (serverId: string, toolKey: string) => void;
  onSetManagementEnabled?: (server: McpServer, enabled: boolean) => void;
}

// The detail drawer is a read-only derivation of the same server record the
// list renders; every action routes through the existing page callbacks.
export const McpDetailPanel = React.memo(function McpDetailPanel({
  server,
  tools,
  loading,
  toolsReadOnly,
  resolvedPackageVersions,
  allTags = [],
  onUpdateTags,
  onClose,
  onEdit,
  onEditMetadata,
  onDelete,
  onToggleTool,
  onSetManagementEnabled,
}: McpDetailPanelProps) {
  const { t } = useTranslation();
  const [tagEditorOpen, setTagEditorOpen] = React.useState(false);
  const [tagDraft, setTagDraft] = React.useState('');

  const serverTagList = React.useMemo(
    () => (server ? normalizeTagList(server.tags ?? []) : []),
    [server],
  );

  // The drawer stays mounted while the user switches servers; reset per-server
  // view state so a previous server's editor does not leak into the next.
  React.useEffect(() => {
    setTagEditorOpen(false);
    setTagDraft('');
  }, [server?.id]);

  const closeTagEditor = React.useCallback(() => {
    setTagEditorOpen(false);
    setTagDraft('');
  }, []);

  const commitTagDraft = React.useCallback(() => {
    if (!onUpdateTags || !server) return;
    const trimmed = tagDraft.trim();
    closeTagEditor();
    if (!trimmed) return;
    const next = normalizeTagList([...(server.tags ?? []), trimmed]);
    if (next.length !== serverTagList.length) {
      onUpdateTags(server.id, next);
    }
  }, [closeTagEditor, onUpdateTags, server, serverTagList.length, tagDraft]);

  const removeServerTag = React.useCallback((removedTag: string) => {
    if (!onUpdateTags || !server) return;
    onUpdateTags(server.id, serverTagList.filter((item) => item !== removedTag));
  }, [onUpdateTags, server, serverTagList]);

  const handleCopyValue = React.useCallback(async (value: string) => {
    try {
      await navigator.clipboard.writeText(value);
      message.success(t('mcp.copiedConfig'));
    } catch {
      message.error(t('mcp.copyFailed'));
    }
  }, [t]);

  const mcpTools = React.useMemo(() => {
    if (!server) return [];
    const enabledToolIdsForSort = new Set(server.enabled_tools ?? []);
    return tools
      .filter((tool) => tool.supports_mcp)
      .sort((left, right) => {
        const leftEnabled = enabledToolIdsForSort.has(left.key) ? 1 : 0;
        const rightEnabled = enabledToolIdsForSort.has(right.key) ? 1 : 0;
        return rightEnabled - leftEnabled;
      });
  }, [server, tools]);

  const enabledToolIds = React.useMemo(
    () => new Set(server?.enabled_tools ?? []),
    [server],
  );

  const syncDetailByTool = React.useMemo(() => {
    const detailMap = new Map<string, string>();
    if (!server) return detailMap;
    for (const detail of server.sync_details) {
      detailMap.set(detail.tool, detail.status);
    }
    return detailMap;
  }, [server]);

  const stdioConfig = server?.server_type === 'stdio'
    ? (server.server_config as { command?: string; args?: string[]; env?: Record<string, string> })
    : null;

  // Shared guard for metadata/tag editing while the server is management-disabled
  // (mirrors SkillDetailPanel's metaEditDisabled).
  const metaEditDisabled = loading || !server?.management_enabled;

  const httpConfig = server && server.server_type !== 'stdio'
    ? (server.server_config as { url?: string; headers?: Record<string, string> })
    : null;

  const envKeys = Object.keys(stdioConfig?.env ?? {});
  const headerKeys = Object.keys(httpConfig?.headers ?? {});

  const packageVersion = React.useMemo(
    () => (server?.server_type === 'stdio' ? getMcpCommandPackageVersion(server.server_config) : null),
    [server],
  );

  const packageVersionDisplayText = React.useMemo(() => {
    if (!server || !packageVersion) {
      return null;
    }
    if (packageVersion.versionLabel !== 'latest') {
      // For pinned versions displayText is already just the version label
      // (e.g. "v1.2.3"), without a package-name prefix.
      return packageVersion.displayText;
    }

    const resolvedVersion = resolvedPackageVersions?.[
      getMcpCommandPackageVersionKey(packageVersion.manager, packageVersion.packageName)
    ];
    if (!resolvedVersion) {
      return null;
    }
    // Show only the resolved version number; the package name is already
    // visible in the args/command line.
    return resolvedVersion;
  }, [packageVersion, resolvedPackageVersions, server]);

  const handleDeleteClick = () => {
    if (!server) return;
    onDelete(server.id);
  };

  const handleSetManagementEnabled = () => {
    if (!server || !onSetManagementEnabled) return;
    onSetManagementEnabled(server, !server.management_enabled);
  };

  const handleReadOnlyToolClick = React.useCallback(() => {
    message.info(t('mcp.groupTools.cardToolReadOnly'));
  }, [t]);

  const handleToolCellClick = (toolKey: string) => {
    if (!server) return;
    if (toolsReadOnly) {
      handleReadOnlyToolClick();
      return;
    }
    if (metaEditDisabled) return;
    onToggleTool(server.id, toolKey);
  };

  // Tooltip copy follows the three-segment convention:
  // "tool display name (resolved mcp config path) — status".
  const buildToolTooltip = (tool: McpTool): string => {
    const status = syncDetailByTool.get(tool.key);
    const statusText = status === 'error'
      ? t('mcp.toolSync.error')
      : enabledToolIds.has(tool.key)
        ? t('mcp.toolSync.synced')
        : t('mcp.toolSync.unsynced');
    const pathSegment = tool.mcp_config_path ? ` (${tool.mcp_config_path})` : '';
    return `${tool.display_name}${pathSegment} — ${statusText}`;
  };

  if (!server) {
    return null;
  }

  return (
    <aside className={styles.panel} aria-label={`${server.name} details`}>
      <button
        type="button"
        className={styles.panelCloseBtn}
        title={t('common.cancel')}
        aria-label={t('common.cancel')}
        onClick={onClose}
      >
        <X size={15} aria-hidden="true" />
      </button>

      <div className={styles.scrollArea}>
        <div className={styles.titleRow}>
          <span
            className={`${styles.statusDot}${server.management_enabled ? ` ${styles.statusDotEnabled}` : ''}`}
            title={server.management_enabled ? t('mcp.enableServer') : t('mcp.disableServer')}
            aria-hidden="true"
          />
          <h2 className={styles.panelTitle}>{server.name}</h2>
        </div>

        <div className={styles.sourceLine}>
          {server.server_type === 'stdio'
            ? <Code2 size={13} aria-hidden="true" />
            : <Globe2 size={13} aria-hidden="true" />}
          {/* Detail panel keeps long references fully visible (no ellipsis). */}
          <span>
            {stdioConfig?.command || httpConfig?.url || server.server_type}
            {packageVersionDisplayText && (
              <> · {packageVersionDisplayText}</>
            )}
            {envKeys.length > 0 && (
              <> · {t('mcp.detail.envCount', { count: envKeys.length })}</>
            )}
          </span>
          <span className={styles.sourceSep} aria-hidden="true">·</span>
          <span>{formatRelativeTime(server.updated_at, t)}</span>
        </div>

        {server.description?.trim() && (
          <p className={styles.descriptionLead}>{server.description.trim()}</p>
        )}

        {(stdioConfig || httpConfig) && (
          <section className={styles.section}>
            <h3 className={styles.sectionTitle}>{t('mcp.detail.configSection')}</h3>
            <div className={styles.configCard}>
              {stdioConfig && (
                <>
                  <div className={styles.configLine}>
                    <span className={styles.configKeyLabel}>command</span>
                    <span className={styles.configValueMono}>{stdioConfig.command || '—'}</span>
                  </div>
                  {(stdioConfig.args?.length ?? 0) > 0 && (
                    <div className={styles.configLine}>
                      <span className={styles.configKeyLabel}>args</span>
                      <span className={styles.configValueList}>
                        {(stdioConfig.args ?? []).map((arg, index) => (
                          <code key={`${index}-${arg}`} className={styles.configArgPill} title={arg}>{arg}</code>
                        ))}
                      </span>
                    </div>
                  )}
                  {envKeys.length > 0 && (
                    <div className={styles.configLine}>
                      <span className={styles.configKeyLabel}>env</span>
                      <div className={styles.secretList}>
                        {envKeys.map((key) => {
                          const value = stdioConfig?.env?.[key] ?? '';
                          return (
                            <div key={key} className={styles.secretRow}>
                              <span className={styles.secretKey}>{key}:</span>
                              <span className={styles.secretValue} title={value}>{value}</span>
                              <button
                                type="button"
                                className={styles.secretAction}
                                title={t('common.copy')}
                                aria-label={`${t('common.copy')}: ${key}`}
                                onClick={() => handleCopyValue(value)}
                              >
                                <Copy size={12} aria-hidden="true" />
                              </button>
                            </div>
                          );
                        })}
                      </div>
                    </div>
                  )}
                </>
              )}
              {httpConfig && (
                <>
                  <div className={styles.configLine}>
                    <span className={styles.configKeyLabel}>url</span>
                    <span className={styles.configValueMono}>{httpConfig.url || '—'}</span>
                  </div>
                  {headerKeys.length > 0 && (
                    <div className={styles.configLine}>
                      <span className={styles.configKeyLabel}>headers</span>
                      <div className={styles.secretList}>
                        {headerKeys.map((key) => {
                          const value = httpConfig?.headers?.[key] ?? '';
                          return (
                            <div key={key} className={styles.secretRow}>
                              <span className={styles.secretKey}>{key}:</span>
                              <span className={styles.secretValue} title={value}>{value}</span>
                              <button
                                type="button"
                                className={styles.secretAction}
                                title={t('common.copy')}
                                aria-label={`${t('common.copy')}: ${key}`}
                                onClick={() => handleCopyValue(value)}
                              >
                                <Copy size={12} aria-hidden="true" />
                              </button>
                            </div>
                          );
                        })}
                      </div>
                    </div>
                  )}
                </>
              )}
            </div>
          </section>
        )}

        <section className={styles.section}>
          <div className={styles.metaCard}>
            <div className={styles.metaRows}>
              <div className={styles.metaRow}>
                <span className={styles.metaLabel}>{t('mcp.metadata.group')}</span>
                {server.user_group?.trim()
                  ? <span className={styles.metaGroupTag}>{server.user_group.trim()}</span>
                  : <span className={styles.metaEmptyText}>{t('mcp.metadata.ungrouped')}</span>}
              </div>
              {server.user_note?.trim() && (
                <div className={styles.metaRow}>
                  <span className={styles.metaLabel}>{t('mcp.metadata.note')}</span>
                  <span className={styles.metaValueText}>{server.user_note.trim()}</span>
                </div>
              )}
            </div>
            <button
              type="button"
              className={styles.metaEditBtn}
              disabled={metaEditDisabled}
              onClick={() => onEditMetadata(server)}
            >
              <Pencil size={11} aria-hidden="true" />
              {t('common.edit')}
            </button>
          </div>
        </section>

        <div className={styles.tagRow}>
          {serverTagList.map((tagItem) => (
            <span key={tagItem} className={`${styles.tagPill} ${tagPillColorClass(tagItem)}`}>
              <span className={styles.tagPillText} title={tagItem}>{tagItem}</span>
              {onUpdateTags && (
                <button
                  type="button"
                  className={styles.tagRemoveBtn}
                  title={t('mcp.tags.remove')}
                  aria-label={`${t('mcp.tags.remove')}: ${tagItem}`}
                  disabled={metaEditDisabled}
                  onClick={() => removeServerTag(tagItem)}
                >
                  <X size={10} aria-hidden="true" />
                </button>
              )}
            </span>
          ))}
          {tagEditorOpen && onUpdateTags ? (
            <>
              <input
                className={styles.tagAddInput}
                list={`mcp-detail-tag-options-${server.id}`}
                autoFocus
                value={tagDraft}
                placeholder={t('mcp.tags.addPlaceholder')}
                aria-label={t('mcp.tags.addPlaceholder')}
                onChange={(event) => setTagDraft(event.target.value)}
                onBlur={commitTagDraft}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') commitTagDraft();
                  if (event.key === 'Escape') closeTagEditor();
                }}
              />
              <datalist id={`mcp-detail-tag-options-${server.id}`}>
                {allTags.map((option) => (<option key={option} value={option} />))}
              </datalist>
            </>
          ) : onUpdateTags && serverTagList.length === 0 ? (
            <button
              type="button"
              className={styles.tagAddEmptyPill}
              title={t('mcp.tags.add')}
              aria-label={t('mcp.tags.add')}
              disabled={metaEditDisabled}
              onClick={() => setTagEditorOpen(true)}
            >
              <Plus size={11} aria-hidden="true" />
              {t('mcp.tags.add')}
            </button>
          ) : onUpdateTags && (
            <button
              type="button"
              className={styles.tagAddBtn}
              title={t('mcp.tags.add')}
              aria-label={t('mcp.tags.add')}
              disabled={metaEditDisabled}
              onClick={() => setTagEditorOpen(true)}
            >
              <Plus size={11} aria-hidden="true" />
            </button>
          )}
        </div>

        {mcpTools.length > 0 && (
          <section className={styles.section}>
            <h3 className={styles.sectionTitle}>{t('mcp.detail.toolsSection')}</h3>
            <div className={styles.toolGrid}>
              {mcpTools.map((tool) => {
                const isEnabled = enabledToolIds.has(tool.key);
                const isError = syncDetailByTool.get(tool.key) === 'error';
                const tooltip = buildToolTooltip(tool);
                const cellClassName = [
                  styles.toolCell,
                  isEnabled ? styles.toolCellSynced : '',
                  isError ? styles.toolCellError : '',
                  toolsReadOnly ? styles.readOnlyTool : '',
                ].filter(Boolean).join(' ');

                return (
                  <button
                    key={tool.key}
                    type="button"
                    className={cellClassName}
                    title={tooltip}
                    aria-label={tooltip}
                    aria-pressed={isEnabled}
                    disabled={metaEditDisabled}
                    onClick={() => handleToolCellClick(tool.key)}
                  >
                    <span className={styles.toolCellHead}>
                      {isEnabled ? (
                        <span className={styles.toolCellDot} aria-hidden="true" />
                      ) : (
                        <span className={styles.toolCellDotOff} aria-hidden="true" />
                      )}
                      <ToolIcon
                        toolKey={tool.key}
                        label={tool.display_name}
                        size={16}
                        iconUrl={tool.icon_url ?? undefined}
                      />
                    </span>
                    <span className={styles.toolCellLabel}>{tool.display_name}</span>
                  </button>
                );
              })}
            </div>
          </section>
        )}

        </div>

      <div className={styles.footerBar}>
        <button
          type="button"
          className={styles.footerActionBtn}
          disabled={loading}
          onClick={() => onEdit(server)}
        >
          <Pencil size={12} aria-hidden="true" />
          {t('mcp.edit')}
        </button>
        <button
          type="button"
          className={styles.footerActionBtn}
          disabled={loading}
          onClick={() => onEditMetadata(server)}
        >
          <Tags size={12} aria-hidden="true" />
          {t('mcp.metadata.edit')}
        </button>
        <button
          type="button"
          className={styles.footerActionBtn}
          disabled={loading}
          onClick={handleSetManagementEnabled}
        >
          {server.management_enabled
            ? <PowerOff size={12} aria-hidden="true" />
            : <Power size={12} aria-hidden="true" />}
          {server.management_enabled ? t('mcp.disable') : t('mcp.enable')}
        </button>
        <button
          type="button"
          className={`${styles.footerActionBtn} ${styles.footerDangerBtn}`}
          disabled={loading}
          onClick={handleDeleteClick}
        >
          <Trash2 size={12} aria-hidden="true" />
          {t('common.delete')}
        </button>
      </div>
    </aside>
  );
});

export default McpDetailPanel;
