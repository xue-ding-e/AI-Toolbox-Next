import React from 'react';
import { message } from 'antd';
import {
  Copy,
  Eye,
  Folder,
  Github,
  Grid2X2,
  MoreHorizontal,
  Plus,
  RefreshCw,
  Tags,
  Trash2,
} from 'lucide-react';
import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener';
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
import styles from './SkillCard.module.less';

interface SkillCardProps {
  skill: ManagedSkill;
  allTools: ToolOption[];
  loading: boolean;
  isUpdating?: boolean;
  dragDisabled?: boolean;
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
  dragHandle,
  containerRef,
  containerStyle,
}: SkillCardContentProps) {
  const { t } = useTranslation();

  const typeKey = skill.source_type.toLowerCase();

  // These values are derived from stable inputs and are recalculated for every card.
  // Memoizing them keeps scroll and hover interactions cheaper when many cards are on screen.
  const github = React.useMemo(
    () => getGithubInfo(skill.source_ref),
    [getGithubInfo, skill.source_ref],
  );

  const copyValue = React.useMemo(
    () => (github?.href ?? skill.source_ref ?? '').trim(),
    [github, skill.source_ref],
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

  const handleIconClick = async () => {
    if (github) {
      await openUrl(github.href);
    } else if (skill.source_type === 'local' && skill.source_ref) {
      try {
        await revealItemInDir(skill.source_ref);
      } catch {
        message.error(t('skills.openFolderFailed'));
      }
    }
  };

  const handleOpenCentralPath = async () => {
    try {
      await revealItemInDir(`${skill.central_path}\\SKILL.md`);
    } catch {
      try {
        await revealItemInDir(skill.central_path);
      } catch {
        message.error(t('skills.openFolderFailed'));
      }
    }
  };

  const iconTooltip = React.useMemo(() => {
    if (github) {
      return t('skills.openRepo');
    }
    if (skill.source_type === 'local' && skill.source_ref) {
      return t('skills.openFolder');
    }
    return undefined;
  }, [github, skill.source_ref, skill.source_type, t]);

  const iconClickable = !!iconTooltip;

  const iconNode = typeKey.includes('git') ? (
    <Github size={18} className={`${styles.icon}${iconClickable ? ` ${styles.clickableIcon}` : ''}`} />
  ) : typeKey.includes('local') ? (
    <Folder size={18} className={`${styles.icon}${iconClickable ? ` ${styles.clickableIcon}` : ''}`} />
  ) : (
    <Grid2X2 size={18} className={styles.icon} />
  );

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
        key: 'metadata',
        icon: <Tags size={14} />,
        label: t('skills.metadata.edit'),
        onSelect: () => onEditMetadata(skill),
      },
      {
        key: 'delete',
        danger: true,
        icon: <Trash2 size={14} />,
        label: t('skills.remove'),
        onSelect: () => onDelete(skill.id),
      },
    ],
    [onDelete, onEditMetadata, skill, t],
  );

  return (
    <ManagementCard
      containerRef={containerRef}
      containerStyle={containerStyle}
      selected={selected}
      selectable={selectable}
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
        asButton={iconClickable}
        title={iconTooltip}
        onClick={iconClickable ? handleIconClick : undefined}
        disabled={!iconClickable}
      />
      <ManagementCardMain>
        <ManagementCardHeader
          title={skill.name}
          minWidth={120}
          meta={
            <>
              <button
                type="button"
                className={styles.detailButton}
                title={t('skills.openDataDir')}
                aria-label={t('skills.openDataDir')}
                onClick={handleOpenCentralPath}
              >
                <Eye size={13} aria-hidden="true" />
              </button>
              <button
                className={styles.sourcePill}
                type="button"
                title={t('common.copy')}
                aria-label={t('common.copy')}
                onClick={handleCopy}
                disabled={!copyValue}
              >
                <span className={styles.sourceText}>
                  {github ? github.label : getSkillSourceLabel(skill)}
                </span>
                <Copy size={11} className={styles.copyIcon} aria-hidden="true" />
              </button>
              <span className={styles.dot}>•</span>
              <span className={styles.time}>{formatRelative(skill.updated_at)}</span>
            </>
          }
        />
        {(skill.user_group || skill.user_note) && (
          <ManagementCardMetaRow>
            {skill.user_group && (
              <span className={styles.groupTag} title={skill.user_group}>{skill.user_group}</span>
            )}
            {skill.user_note && (
              <span className={styles.note} title={skill.user_note}>{skill.user_note}</span>
            )}
          </ManagementCardMetaRow>
        )}
        <ManagementCardToolMatrix>
          {syncedTools.map((tool) => {
            const target = skill.targets.find((t) => t.tool === tool.id);
            return (
              <button
                key={`${skill.id}-${tool.id}`}
                title={`${tool.label} (${target?.mode ?? t('skills.unknown')})`}
                type="button"
                className={`${styles.toolPill} ${styles.active}${toolsReadOnly ? ` ${styles.readOnlyTool}` : ''}`}
                onClick={toolsReadOnly ? handleReadOnlyToolClick : () => onToggleTool(skill, tool.id)}
                disabled={loading || isUpdating}
                aria-disabled={toolsReadOnly || loading || isUpdating}
              >
                <span className={styles.statusBadge} />
                {tool.label}
              </button>
            );
          })}
          {!toolsReadOnly && dropdownItems.length > 0 && (
            <ManagementMenu
              items={dropdownItems}
              disabled={loading || isUpdating}
              title={t('skills.batch.addTool')}
              triggerClassName={styles.addToolBtn}
            >
              <Plus size={13} aria-hidden="true" />
            </ManagementMenu>
          )}
        </ManagementCardToolMatrix>
      </ManagementCardMain>
      <ManagementCardActions>
        <ManagementMenu
          items={actionItems}
          disabled={loading || isUpdating}
          title={t('skills.more')}
          controlSize="compact"
        >
          <MoreHorizontal size={16} aria-hidden="true" />
        </ManagementMenu>
        <ManagementIconButton
          icon={<RefreshCw size={15} aria-hidden="true" />}
          onClick={() => onUpdate(skill)}
          disabled={loading || isUpdating}
          title={t('skills.updateTooltip')}
          controlSize="compact"
        />
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
