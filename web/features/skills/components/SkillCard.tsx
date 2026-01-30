import React from 'react';
import { Button, Tooltip, message } from 'antd';
import {
  GithubOutlined,
  FolderOutlined,
  AppstoreOutlined,
  SyncOutlined,
  DeleteOutlined,
  CopyOutlined,
} from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import type { ManagedSkill, ToolOption } from '../types';
import styles from './SkillCard.module.less';

interface SkillCardProps {
  skill: ManagedSkill;
  installedTools: ToolOption[];
  loading: boolean;
  getGithubInfo: (url: string | null | undefined) => { label: string; href: string } | null;
  getSkillSourceLabel: (skill: ManagedSkill) => string;
  formatRelative: (ms: number | null | undefined) => string;
  onUpdate: (skill: ManagedSkill) => void;
  onDelete: (skillId: string) => void;
  onToggleTool: (skill: ManagedSkill, toolId: string) => void;
}

export const SkillCard: React.FC<SkillCardProps> = ({
  skill,
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
  const typeKey = skill.source_type.toLowerCase();
  const github = getGithubInfo(skill.source_ref);
  const copyValue = (github?.href ?? skill.source_ref ?? '').trim();

  const handleCopy = async () => {
    if (!copyValue) return;
    try {
      await navigator.clipboard.writeText(copyValue);
      message.success(t('skills.copied'));
    } catch {
      message.error(t('skills.copyFailed'));
    }
  };

  const iconNode = typeKey.includes('git') ? (
    <GithubOutlined className={styles.icon} />
  ) : typeKey.includes('local') ? (
    <FolderOutlined className={styles.icon} />
  ) : (
    <AppstoreOutlined className={styles.icon} />
  );

  return (
    <div className={styles.card}>
      <div className={styles.iconArea}>{iconNode}</div>
      <div className={styles.main}>
        <div className={styles.headerRow}>
          <div className={styles.name}>{skill.name}</div>
          <Tooltip title={t('common.copy')}>
            <button
              className={styles.sourcePill}
              type="button"
              onClick={handleCopy}
              disabled={!copyValue}
            >
              <span className={styles.sourceText}>
                {github ? github.label : getSkillSourceLabel(skill)}
              </span>
              <CopyOutlined className={styles.copyIcon} />
            </button>
          </Tooltip>
          <span className={styles.dot}>•</span>
          <span className={styles.time}>{formatRelative(skill.updated_at)}</span>
        </div>
        <div className={styles.toolMatrix}>
          {installedTools.map((tool) => {
            const target = skill.targets.find((t) => t.tool === tool.id);
            const synced = Boolean(target);
            return (
              <Tooltip
                key={`${skill.id}-${tool.id}`}
                title={
                  synced
                    ? `${tool.label} (${target?.mode ?? t('skills.unknown')})`
                    : tool.label
                }
              >
                <button
                  type="button"
                  className={`${styles.toolPill} ${synced ? styles.active : styles.inactive}`}
                  onClick={() => onToggleTool(skill, tool.id)}
                >
                  {synced && <span className={styles.statusBadge} />}
                  {tool.label}
                </button>
              </Tooltip>
            );
          })}
        </div>
      </div>
      <div className={styles.actions}>
        <Button
          type="text"
          icon={<SyncOutlined />}
          onClick={() => onUpdate(skill)}
          disabled={loading}
          title={t('skills.update')}
        />
        <Button
          type="text"
          danger
          icon={<DeleteOutlined />}
          onClick={() => onDelete(skill.id)}
          disabled={loading}
          title={t('skills.remove')}
        />
      </div>
    </div>
  );
};
