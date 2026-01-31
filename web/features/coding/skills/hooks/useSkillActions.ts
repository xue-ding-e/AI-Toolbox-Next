import React from 'react';
import { message } from 'antd';
import { arrayMove } from '@dnd-kit/sortable';
import type { DragEndEvent } from '@dnd-kit/core';
import { useTranslation } from 'react-i18next';
import * as api from '../services/skillsApi';
import { useSkills } from './useSkills';
import type { ManagedSkill, ToolOption } from '../types';
import { showGitError, confirmTargetOverwrite } from '../utils/errorHandlers';
import { refreshTrayMenu } from '@/services/appApi';

export interface UseSkillActionsOptions {
  allTools: ToolOption[];
}

export interface UseSkillActionsResult {
  actionLoading: boolean;
  deleteSkillId: string | null;
  setDeleteSkillId: (id: string | null) => void;
  skillToDelete: ManagedSkill | undefined;
  handleToggleTool: (skill: ManagedSkill, toolId: string) => Promise<void>;
  handleUpdate: (skill: ManagedSkill) => Promise<void>;
  handleDelete: (skillId: string) => void;
  confirmDelete: () => Promise<void>;
  handleDragEnd: (event: DragEndEvent) => Promise<void>;
}

export function useSkillActions({ allTools }: UseSkillActionsOptions): UseSkillActionsResult {
  const { t } = useTranslation();
  const { skills, refresh, updateSkill, deleteSkill, setSkills } = useSkills();

  const [deleteSkillId, setDeleteSkillId] = React.useState<string | null>(null);
  const [actionLoading, setActionLoading] = React.useState(false);

  const skillToDelete = deleteSkillId
    ? skills.find((s) => s.id === deleteSkillId)
    : undefined;

  const handleToggleTool = React.useCallback(async (skill: ManagedSkill, toolId: string) => {
    const target = skill.targets.find((t) => t.tool === toolId);
    const synced = Boolean(target);

    setActionLoading(true);
    try {
      if (synced) {
        await api.unsyncSkillFromTool(skill.id, toolId);
      } else {
        await api.syncSkillToTool(skill.central_path, skill.id, toolId, skill.name);
      }
      await refresh();
      await refreshTrayMenu();
    } catch (error) {
      const errMsg = String(error);
      if (errMsg.includes('TARGET_EXISTS|')) {
        const match = errMsg.match(/TARGET_EXISTS\|(.+)/);
        const targetPath = match ? match[1] : '';
        const toolLabel = allTools.find((t) => t.id === toolId)?.label || toolId;
        const shouldOverwrite = await confirmTargetOverwrite(skill.name, toolLabel, targetPath, t);
        if (shouldOverwrite) {
          try {
            await api.syncSkillToTool(skill.central_path, skill.id, toolId, skill.name, true);
            await refresh();
            await refreshTrayMenu();
          } catch (retryError) {
            message.error(String(retryError));
          }
        }
      } else {
        showGitError(errMsg, t, allTools);
      }
    } finally {
      setActionLoading(false);
    }
  }, [allTools, t, refresh]);

  const handleUpdate = React.useCallback(async (skill: ManagedSkill) => {
    setActionLoading(true);
    try {
      await updateSkill(skill);
    } catch (error) {
      showGitError(String(error), t, allTools);
    } finally {
      setActionLoading(false);
    }
  }, [updateSkill, t, allTools]);

  const handleDelete = React.useCallback((skillId: string) => {
    setDeleteSkillId(skillId);
  }, []);

  const confirmDelete = React.useCallback(async () => {
    if (!deleteSkillId) return;
    setActionLoading(true);
    try {
      await deleteSkill(deleteSkillId);
      setDeleteSkillId(null);
      await refreshTrayMenu();
    } catch (error) {
      showGitError(String(error), t, allTools);
    } finally {
      setActionLoading(false);
    }
  }, [deleteSkillId, deleteSkill, t, allTools]);

  const handleDragEnd = React.useCallback(async (event: DragEndEvent) => {
    const { active, over } = event;

    if (!over || active.id === over.id) {
      return;
    }

    const oldIndex = skills.findIndex((s) => s.id === active.id);
    const newIndex = skills.findIndex((s) => s.id === over.id);

    if (oldIndex === -1 || newIndex === -1) {
      return;
    }

    // Optimistic update
    const oldSkills = [...skills];
    const newSkills = arrayMove(skills, oldIndex, newIndex);
    setSkills(newSkills);

    try {
      await api.reorderSkills(newSkills.map((s) => s.id));
      await refreshTrayMenu();
    } catch (error) {
      // Rollback on error
      console.error('Failed to reorder skills:', error);
      setSkills(oldSkills);
      message.error(t('common.error'));
    }
  }, [skills, setSkills, t]);

  return {
    actionLoading,
    deleteSkillId,
    setDeleteSkillId,
    skillToDelete,
    handleToggleTool,
    handleUpdate,
    handleDelete,
    confirmDelete,
    handleDragEnd,
  };
}
