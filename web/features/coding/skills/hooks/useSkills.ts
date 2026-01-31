import React from 'react';
import { listen } from '@tauri-apps/api/event';
import { useSkillsStore } from '../stores/skillsStore';
import * as api from '../services/skillsApi';
import type { ManagedSkill } from '../types';

export function useSkills() {
  const store = useSkillsStore();

  // Initialize on mount
  React.useEffect(() => {
    if (store.isModalOpen) {
      store.refresh();
    }
  }, [store.isModalOpen]);

  // Listen for skills-changed events from tray
  React.useEffect(() => {
    const unlisten = listen<string>('skills-changed', (event) => {
      if (event.payload === 'tray') {
        store.loadSkills();
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [store.loadSkills]);

  // Format relative time
  const formatRelative = React.useCallback((ms: number | null | undefined) => {
    if (!ms) return '—';

    const now = Date.now();
    const diff = now - ms;
    const minutes = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);

    if (minutes < 1) return 'just now';
    if (minutes < 60) return `${minutes}m ago`;
    if (hours < 24) return `${hours}h ago`;
    return `${days}d ago`;
  }, []);

  // Get GitHub info from URL
  const getGithubInfo = React.useCallback((url: string | null | undefined) => {
    if (!url) return null;

    const match = url.match(/github\.com[\/:]([^\/]+)\/([^\/\.]+)/);
    if (match) {
      const [, owner, repo] = match;
      return {
        label: `${owner}/${repo}`,
        href: `https://github.com/${owner}/${repo}`,
      };
    }
    return null;
  }, []);

  // Get skill source label
  const getSkillSourceLabel = React.useCallback((skill: ManagedSkill) => {
    if (skill.source_type === 'git') {
      const info = getGithubInfo(skill.source_ref);
      return info ? info.label : skill.source_ref || 'Git';
    }
    if (skill.source_type === 'local') {
      // Return just the folder name
      const path = skill.source_ref || '';
      const parts = path.split(/[\/\\]/);
      return parts[parts.length - 1] || 'Local';
    }
    return skill.source_type;
  }, [getGithubInfo]);

  // Update skill
  const updateSkill = React.useCallback(
    async (skill: ManagedSkill) => {
      try {
        await api.updateManagedSkill(skill.id);
        await store.loadSkills();
      } catch (error) {
        console.error('Failed to update skill:', error);
        throw error;
      }
    },
    [store]
  );

  // Delete skill
  const deleteSkill = React.useCallback(
    async (skillId: string) => {
      try {
        await api.deleteManagedSkill(skillId);
        await store.loadSkills();
      } catch (error) {
        console.error('Failed to delete skill:', error);
        throw error;
      }
    },
    [store]
  );

  return {
    ...store,
    formatRelative,
    getGithubInfo,
    getSkillSourceLabel,
    updateSkill,
    deleteSkill,
  };
}
