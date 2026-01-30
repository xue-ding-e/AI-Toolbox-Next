import React from 'react';
import { Empty } from 'antd';
import { useTranslation } from 'react-i18next';
import { SkillCard } from './SkillCard';
import type { ManagedSkill, ToolOption } from '../types';
import styles from './SkillsList.module.less';

interface SkillsListProps {
  skills: ManagedSkill[];
  installedTools: ToolOption[];
  loading: boolean;
  getGithubInfo: (url: string | null | undefined) => { label: string; href: string } | null;
  getSkillSourceLabel: (skill: ManagedSkill) => string;
  formatRelative: (ms: number | null | undefined) => string;
  onUpdate: (skill: ManagedSkill) => void;
  onDelete: (skillId: string) => void;
  onToggleTool: (skill: ManagedSkill, toolId: string) => void;
}

export const SkillsList: React.FC<SkillsListProps> = ({
  skills,
  installedTools,
  loading,
  getGithubInfo,
  getSkillSourceLabel,
  formatRelative,
  onUpdate,
  onDelete,
  onToggleTool,
}) => {
  const { t } = useTranslation();

  if (skills.length === 0) {
    return (
      <div className={styles.empty}>
        <Empty description={t('skills.skillsEmpty')} />
      </div>
    );
  }

  return (
    <div className={styles.list}>
      {skills.map((skill) => (
        <SkillCard
          key={skill.id}
          skill={skill}
          installedTools={installedTools}
          loading={loading}
          getGithubInfo={getGithubInfo}
          getSkillSourceLabel={getSkillSourceLabel}
          formatRelative={formatRelative}
          onUpdate={onUpdate}
          onDelete={onDelete}
          onToggleTool={onToggleTool}
        />
      ))}
    </div>
  );
};
