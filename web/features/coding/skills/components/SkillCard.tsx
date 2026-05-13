import React from 'react';
import { message } from 'antd';
import {
  Copy,
  ExternalLink,
  Folder,
  Pencil,
  Power,
  MoreHorizontal,
  Plus,
  RefreshCw,
  Trash2,
  TriangleAlert,
} from 'lucide-react';
import { openPath, openUrl, revealItemInDir } from '@tauri-apps/plugin-opener';
import { useTranslation } from 'react-i18next';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import {
  ManagementCard,
  ManagementCardActions,
  ManagementCardCheckboxArea,
  ManagementCardDragHandle,
  ManagementCardHeader,
  ManagementCardIcon,
  ManagementCardMain,
  ManagementCardMetaRow,
  ManagementCardToolMatrix,
  ManagementCheckbox,
  ManagementIconButton,
  ManagementMenu,
  type ManagementMenuItem,
} from '@/features/coding/shared/management';
import type { ManagedSkill, ToolOption } from '../types';
import { getSkillManifestPath } from '../utils/skillPath';
import styles from './SkillCard.module.less';

interface SkillCardProps {
  skill: ManagedSkill;
  allTools: ToolOption[];
  loading: boolean;
  isUpdating?: boolean;
  dragDisabled?: boolean;
  showGroupTag?: boolean;
  selectable?: boolean;
  selected?: boolean;
  toolsReadOnly?: boolean;
  onSelectChange?: (skillId: string, checked: boolean) => void;
  getGithubInfo: (url: string | null | undefined) => { label: string; href: string } | null;
  getSkillSourceLabel: (skill: ManagedSkill) => string;
  formatRelative: (ms: number | null | undefined) => string;
  onUpdate: (skill: ManagedSkill) => void;
  onDelete: (skillId: string) => void;
  onToggleTool: (skill: ManagedSkill, toolId: string) => void;
  onEditMetadata: (skill: ManagedSkill) => void;
  onSetManagementEnabled: (skill: ManagedSkill, enabled: boolean) => void;
}

interface SkillCardContentProps extends Omit<SkillCardProps, 'dragDisabled'> {
  dragHandle?: React.ReactNode;
  containerRef?: (node: HTMLDivElement | null) => void;
  containerStyle?: React.CSSProperties;
}

const SkillCardContent = React.memo(function SkillCardContent({
  skill,
  allTools,
  loading,
  isUpdating = false,
  showGroupTag = true,
  selectable,
  selected,
  toolsReadOnly,
  onSelectChange,
  getGithubInfo,
  getSkillSourceLabel,
  formatRelative,
  onUpdate,
  onDelete,
  onToggleTool,
  onEditMetadata,
  onSetManagementEnabled,
  dragHandle,
  containerRef,
  containerStyle,
}: SkillCardContentProps) {
  const { t } = useTranslation();

  const typeKey = skill.source_type.toLowerCase();
  const sourceWarningMessage = skill.source_health === 'warning'
    ? (skill.source_error || t('skills.sourceWarningFallback'))
    : undefined;
  const cardClassName = [
    !skill.management_enabled ? styles.disabledCard : undefined,
    sourceWarningMessage ? styles.sourceWarningCard : undefined,
  ].filter(Boolean).join(' ') || undefined;
  const groupLabel = skill.user_group?.trim() ?? '';
  const descriptionText = skill.description?.trim() ?? '';
  const userNoteText = skill.user_note?.trim() ?? '';
  const shouldShowGroupTag = showGroupTag && groupLabel.length > 0;
  const hasDescription = descriptionText.length > 0;
  const hasUserNote = userNoteText.length > 0;
  const managementToggleLabel = skill.management_enabled ? t('skills.disableSkill') : t('skills.enableSkill');
  const sourceTypeLabel = skill.source_type === 'git'
    ? t('skills.card.sourceGit')
    : skill.source_type === 'local'
      ? t('skills.card.sourceLocal')
      : t('skills.card.sourceImport');

  // These values are derived from stable inputs and are recalculated for every card.
  // Memoizing them keeps scroll and hover interactions cheaper when many cards are on screen.
  const github = React.useMemo(
    () => getGithubInfo(skill.source_ref),
    [getGithubInfo, skill.source_ref],
  );

  const copyValue = React.useMemo(
    () => (skill.source_ref ?? '').trim(),
    [skill.source_ref],
  );

  const handleCopy = async () => {
    if (!copyValue) return;
    try {
      await navigator.clipboard.writeText(copyValue);
      message.success(t('skills.copied'));
    } catch {
      message.error(t('skills.copyFailed'));
    }
  };

  const handleReadOnlyToolClick = React.useCallback(() => {
    message.info(t('skills.groupTools.cardToolReadOnly'));
  }, [t]);

  const openFirstPath = React.useCallback(async (paths: string[]) => {
    for (const path of paths) {
      try {
        await openPath(path);
        return true;
      } catch {
        // Try the next candidate. Some local source paths may no longer exist,
        // while the central repository path remains the managed source of truth.
      }
    }

    return false;
  }, []);

  const handleOpenCentralPath = async () => {
    const manifestPath = getSkillManifestPath(skill.central_path);

    if (manifestPath) {
      try {
        await revealItemInDir(manifestPath);
        return;
      } catch {
        // If SKILL.md cannot be revealed, fall back to opening the managed folder.
      }
    }

    const opened = await openFirstPath([skill.central_path]);
    if (!opened) {
      message.error(t('skills.openFolderFailed'));
    }
  };

  const handleOpenOriginalSource = React.useCallback(async () => {
    const sourceUrl = github?.href ?? skill.source_ref?.trim();
    if (!sourceUrl) return;

    try {
      await openUrl(sourceUrl);
    } catch {
      message.error(t('skills.openFolderFailed'));
    }
  }, [github, skill.source_ref, t]);

  const handleToggleManagement = React.useCallback(() => {
    if (loading || isUpdating) return;
    onSetManagementEnabled(skill, !skill.management_enabled);
  }, [isUpdating, loading, onSetManagementEnabled, skill]);

  const iconTooltip = t('skills.openDataDir');

  const iconNode = <Folder size={18} className={styles.icon} />;

  // Tool grouping is pure derived data based on the skill targets and tool list.
  // Memoizing avoids rebuilding the same sets and filtered arrays on every parent render.
  const syncedToolIds = React.useMemo(
    () => new Set(skill.targets.map((target) => target.tool)),
    [skill.targets],
  );

  const syncedTools = React.useMemo(
    () => allTools.filter((tool) => syncedToolIds.has(tool.id)),
    [allTools, syncedToolIds],
  );

  const availableDropdownTools = React.useMemo(() => {
    return allTools.filter((tool) => tool.installed && !syncedToolIds.has(tool.id));
  }, [allTools, syncedToolIds]);

  // Dropdown items are also pure view data. Keep them memoized so large lists do not
  // recreate identical menu structures unless tools, translations, or handlers change.
  const dropdownItems = React.useMemo<ManagementMenuItem[]>(
    () =>
      availableDropdownTools.map((tool) => ({
        key: tool.id,
        label: tool.label,
        onSelect: () => onToggleTool(skill, tool.id),
      })),
    [availableDropdownTools, onToggleTool, skill],
  );

  const actionItems = React.useMemo<ManagementMenuItem[]>(
    () => [
      {
        key: 'management-enabled',
        icon: <Power size={14} />,
        label: managementToggleLabel,
        onSelect: handleToggleManagement,
      },
      ...(typeKey.includes('git') ? [{
        key: 'open-original-source',
        icon: <ExternalLink size={14} />,
        label: t('skills.openRepo'),
        onSelect: handleOpenOriginalSource,
        disabled: !skill.source_ref?.trim(),
      } satisfies ManagementMenuItem] : []),
      {
        key: 'delete',
        danger: true,
        icon: <Trash2 size={14} />,
        label: t('skills.remove'),
        onSelect: () => onDelete(skill.id),
      },
    ],
    [handleOpenOriginalSource, handleToggleManagement, managementToggleLabel, onDelete, skill, t, typeKey],
  );

  return (
    <ManagementCard
      containerRef={containerRef}
      containerStyle={containerStyle}
      selected={selected}
      selectable={selectable}
      className={[styles.skillCard, cardClassName].filter(Boolean).join(' ')}
    >
      {selectable && (
        <ManagementCardCheckboxArea>
          <ManagementCheckbox
            ariaLabel={`${t('common.select')} ${skill.name}`}
            checked={!!selected}
            onChange={(checked) => onSelectChange?.(skill.id, checked)}
          />
        </ManagementCardCheckboxArea>
      )}
      {dragHandle}
      <ManagementCardIcon
        icon={iconNode}
        asButton
        title={iconTooltip}
        onClick={handleOpenCentralPath}
      />
      <ManagementCardMain>
        <ManagementCardHeader
          title={(
            <span className={styles.titleInline}>
              <span className={`${styles.sourceTypeBadge} ${styles[`sourceType${skill.source_type}`]}`} title={sourceTypeLabel}>
                {sourceTypeLabel}
              </span>
              <span className={styles.skillNameText} title={skill.name}>{skill.name}</span>
              <button
                className={styles.titleCopyButton}
                type="button"
                title={t('common.copy')}
                aria-label={`${t('common.copy')} ${skill.name}`}
                onClick={handleCopy}
                disabled={!copyValue}
              >
                <Copy size={12} aria-hidden="true" />
              </button>
            </span>
          )}
          minWidth={120}
        />
        <ManagementCardMetaRow>
          <div className={styles.metaLine}>
            <span className={styles.sourceContext} title={skill.source_ref ?? undefined}>
              {github ? github.label : getSkillSourceLabel(skill)}
            </span>
            {shouldShowGroupTag && (
              <>
                <span className={styles.dot}>•</span>
                <span
                  className={styles.groupMeta}
                  title={groupLabel}
                  aria-label={`${t('skills.metadata.group')}: ${groupLabel}`}
                >
                  {groupLabel}
                </span>
              </>
            )}
            <span className={styles.dot}>•</span>
            {sourceWarningMessage ? (
              <span
                className={styles.sourceWarningMeta}
                title={sourceWarningMessage}
                aria-label={`${t('skills.sourceWarning')}: ${sourceWarningMessage}`}
              >
                <TriangleAlert size={12} aria-hidden="true" />
                <span>{t('skills.sourceWarning')}</span>
              </span>
            ) : (
              <span className={styles.time}>{formatRelative(skill.updated_at)}</span>
            )}
          </div>
        </ManagementCardMetaRow>
        {(hasDescription || hasUserNote) && (
          <ManagementCardMetaRow>
            <div className={styles.infoStack}>
              {hasUserNote && (
                <div className={styles.noteBlock} title={userNoteText}>
                  <span className={styles.noteText}>{userNoteText}</span>
                </div>
              )}
              {hasDescription && (
                <div className={styles.summaryBlock} title={descriptionText}>
                  {descriptionText}
                </div>
              )}
            </div>
          </ManagementCardMetaRow>
        )}
        <ManagementCardToolMatrix className={styles.toolSection}>
          {syncedTools.map((tool) => {
            const target = skill.targets.find((t) => t.tool === tool.id);
            return (
              <button
                key={`${skill.id}-${tool.id}`}
                title={`${tool.label} (${target?.mode ?? t('skills.unknown')})`}
                type="button"
                className={`${styles.toolPill} ${styles.active}${toolsReadOnly ? ` ${styles.readOnlyTool}` : ''}`}
                onClick={toolsReadOnly ? handleReadOnlyToolClick : () => onToggleTool(skill, tool.id)}
                disabled={loading || isUpdating || !skill.management_enabled}
                aria-disabled={toolsReadOnly || loading || isUpdating || !skill.management_enabled}
              >
                <span className={styles.statusBadge} />
                {tool.label}
              </button>
            );
          })}
          {!toolsReadOnly && dropdownItems.length > 0 && (
            <ManagementMenu
              items={dropdownItems}
              disabled={loading || isUpdating || !skill.management_enabled}
              title={t('skills.batch.addTool')}
              triggerClassName={styles.addToolBtn}
            >
              <Plus size={13} aria-hidden="true" />
            </ManagementMenu>
          )}
        </ManagementCardToolMatrix>
      </ManagementCardMain>
      <ManagementCardActions>
        <ManagementIconButton
          icon={<Pencil size={14} aria-hidden="true" />}
          onClick={() => onEditMetadata(skill)}
          disabled={loading || isUpdating}
          title={t('skills.metadata.edit')}
          controlSize="compact"
        />
        <ManagementIconButton
          icon={<RefreshCw size={15} aria-hidden="true" />}
          onClick={() => onUpdate(skill)}
          disabled={loading || isUpdating}
          title={t('skills.updateTooltip')}
          controlSize="compact"
        />
        <ManagementMenu
          items={actionItems}
          disabled={loading || isUpdating}
          title={t('skills.more')}
          controlSize="compact"
        >
          <MoreHorizontal size={16} aria-hidden="true" />
        </ManagementMenu>
      </ManagementCardActions>
    </ManagementCard>
  );
});

const SortableSkillCard: React.FC<Omit<SkillCardProps, 'dragDisabled'>> = (props) => {
  const {
    skill,
  } = props;

  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: skill.id });

  const sortableStyle: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
  };

  return (
    <SkillCardContent
      {...props}
      containerRef={setNodeRef}
      containerStyle={sortableStyle}
      dragHandle={(
        <ManagementCardDragHandle
          {...attributes}
          listeners={listeners}
        />
      )}
    />
  );
};

export const SkillCard = React.memo(function SkillCard({
  dragDisabled,
  ...props
}: SkillCardProps) {
  if (dragDisabled) {
    return <SkillCardContent {...props} />;
  }

  return <SortableSkillCard {...props} />;
});
