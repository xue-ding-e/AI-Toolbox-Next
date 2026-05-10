import React from 'react';
import { Collapse, Empty, Checkbox, Dropdown, Tooltip } from 'antd';
import { PlusOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { SkillCard } from './SkillCard';
import type { SkillGroup, ToolOption, ManagedSkill } from '../types';
import { getSkillGroupToolIds, isSkillUngroupedCustomGroup } from '../utils/skillGrouping';
import styles from './SkillsGroupedList.module.less';

interface SkillsGroupedListProps {
  groups: SkillGroup[];
  allTools: ToolOption[];
  loading: boolean;
  updatingSkillIds: string[];
  activeKeys: string[];
  onActiveKeysChange: (keys: string[]) => void;
  selectedIds: Set<string>;
  onSelectChange: (skillId: string, checked: boolean) => void;
  onSelectAllGroup: (group: SkillGroup, checked: boolean) => void;
  getGithubInfo: (url: string | null | undefined) => { label: string; href: string } | null;
  getSkillSourceLabel: (skill: ManagedSkill) => string;
  formatRelative: (ms: number | null | undefined) => string;
  onUpdate: (skill: ManagedSkill) => void;
  onDelete: (skillId: string) => void;
  onToggleTool: (skill: ManagedSkill, toolId: string) => void;
  onEditMetadata: (skill: ManagedSkill) => void;
  groupToolMode?: boolean;
  onAddGroupTool?: (group: SkillGroup, toolId: string) => void;
  onRemoveGroupTool?: (group: SkillGroup, toolId: string) => void;
}

export const SkillsGroupedList: React.FC<SkillsGroupedListProps> = ({
  groups,
  allTools,
  loading,
  updatingSkillIds,
  activeKeys,
  onActiveKeysChange,
  selectedIds,
  onSelectChange,
  onSelectAllGroup,
  getGithubInfo,
  getSkillSourceLabel,
  formatRelative,
  onUpdate,
  onDelete,
  onToggleTool,
  onEditMetadata,
  groupToolMode = false,
  onAddGroupTool,
  onRemoveGroupTool,
}) => {
  const { t } = useTranslation();

  if (groups.length === 0) {
    return (
      <div className={styles.empty}>
        <Empty description={t('skills.skillsEmpty')} />
      </div>
    );
  }

  const isGroupAllSelected = (group: SkillGroup) =>
    group.skills.length > 0 && group.skills.every((s) => selectedIds.has(s.id));

  const isGroupPartialSelected = (group: SkillGroup) =>
    group.skills.some((s) => selectedIds.has(s.id)) && !isGroupAllSelected(group);

  const renderGroupTools = (group: SkillGroup) => {
    const activeToolIds = new Set(getSkillGroupToolIds(group));
    const activeTools = allTools.filter((tool) => activeToolIds.has(tool.id));
    const availableTools = allTools.filter((tool) => tool.installed && !activeToolIds.has(tool.id));

    return (
      <div className={styles.groupTools} onClick={(e) => e.stopPropagation()}>
        {activeTools.map((tool) => (
          <Tooltip
            key={tool.id}
            title={t('skills.groupTools.removeTool', { tool: tool.label })}
          >
            <button
              type="button"
              className={styles.groupToolPill}
              disabled={loading}
              onClick={() => onRemoveGroupTool?.(group, tool.id)}
            >
              <span className={styles.statusBadge} />
              {tool.label}
            </button>
          </Tooltip>
        ))}
        {availableTools.length > 0 && (
          <Dropdown
            menu={{
              items: availableTools.map((tool) => ({
                key: tool.id,
                label: tool.label,
                onClick: () => onAddGroupTool?.(group, tool.id),
              })),
            }}
            trigger={['click']}
            disabled={loading}
          >
            <button type="button" className={styles.groupToolAdd} disabled={loading}>
              <PlusOutlined />
            </button>
          </Dropdown>
        )}
      </div>
    );
  };

  const items = groups.map((group) => {
    const groupToolsEnabled = groupToolMode && !isSkillUngroupedCustomGroup(group);

    return {
      key: group.key,
      label: (
        <div className={styles.groupHeader}>
          <div className={styles.groupTitle}>
            <Checkbox
              checked={isGroupAllSelected(group)}
              indeterminate={isGroupPartialSelected(group)}
              onChange={(e) => {
                e.stopPropagation();
                onSelectAllGroup(group, e.target.checked);
              }}
              onClick={(e) => e.stopPropagation()}
            />
            <span className={styles.groupLabel}>
              {group.label}
              <span className={styles.groupCount}>
                ({t('skills.skillCount', { count: group.skills.length })})
              </span>
            </span>
          </div>
          {groupToolsEnabled && renderGroupTools(group)}
        </div>
      ),
      children: (
        <div className={styles.groupGrid}>
          {group.skills.map((skill) => (
            <SkillCard
              key={skill.id}
              skill={skill}
              allTools={allTools}
              loading={loading}
              isUpdating={updatingSkillIds.includes(skill.id)}
              dragDisabled
              selectable
              selected={selectedIds.has(skill.id)}
              toolsReadOnly={groupToolsEnabled}
              onSelectChange={onSelectChange}
              getGithubInfo={getGithubInfo}
              getSkillSourceLabel={getSkillSourceLabel}
              formatRelative={formatRelative}
              onUpdate={onUpdate}
              onDelete={onDelete}
              onToggleTool={onToggleTool}
              onEditMetadata={onEditMetadata}
            />
          ))}
        </div>
      ),
    };
  });

  return (
    <div className={styles.groupedList}>
      <Collapse
        activeKey={activeKeys}
        onChange={(keys) => onActiveKeysChange(keys as string[])}
        items={items}
      />
    </div>
  );
};
