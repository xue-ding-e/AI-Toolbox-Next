import { invoke } from '@tauri-apps/api/core';
import type {
  ManagedSkill,
  ToolStatus,
  InstallResult,
  SyncResult,
  UpdateResult,
  GitSkillCandidate,
  OnboardingPlan,
} from '../types';

// Tool Status
export const getToolStatus = async (): Promise<ToolStatus> => {
  return invoke<ToolStatus>('skills_get_tool_status');
};

// Central Repo Path
export const getCentralRepoPath = async (): Promise<string> => {
  return invoke<string>('skills_get_central_repo_path');
};

export const setCentralRepoPath = async (path: string): Promise<string> => {
  return invoke<string>('skills_set_central_repo_path', { path });
};

// Managed Skills
export const getManagedSkills = async (): Promise<ManagedSkill[]> => {
  return invoke<ManagedSkill[]>('skills_get_managed_skills');
};

// Install Skills
export const installLocalSkill = async (
  sourcePath: string,
  overwrite?: boolean
): Promise<InstallResult> => {
  return invoke<InstallResult>('skills_install_local', { sourcePath, overwrite });
};

export const installGitSkill = async (
  repoUrl: string,
  branch?: string,
  overwrite?: boolean
): Promise<InstallResult> => {
  return invoke<InstallResult>('skills_install_git', { repoUrl, branch, overwrite });
};

export const listGitSkills = async (repoUrl: string, branch?: string): Promise<GitSkillCandidate[]> => {
  return invoke<GitSkillCandidate[]>('skills_list_git_skills', { repoUrl, branch });
};

export const installGitSelection = async (
  repoUrl: string,
  subpath: string,
  branch?: string,
  overwrite?: boolean
): Promise<InstallResult> => {
  return invoke<InstallResult>('skills_install_git_selection', { repoUrl, subpath, branch, overwrite });
};

// Sync Skills
export const syncSkillToTool = async (
  sourcePath: string,
  skillId: string,
  tool: string,
  name: string,
  overwrite?: boolean
): Promise<SyncResult> => {
  return invoke<SyncResult>('skills_sync_to_tool', {
    sourcePath,
    skillId,
    tool,
    name,
    overwrite,
  });
};

export const unsyncSkillFromTool = async (
  skillId: string,
  tool: string
): Promise<void> => {
  return invoke('skills_unsync_from_tool', { skillId, tool });
};

// Update/Delete Skills
export const updateManagedSkill = async (skillId: string): Promise<UpdateResult> => {
  return invoke<UpdateResult>('skills_update_managed', { skillId });
};

export const deleteManagedSkill = async (skillId: string): Promise<void> => {
  return invoke('skills_delete_managed', { skillId });
};

// Onboarding
export const getOnboardingPlan = async (): Promise<OnboardingPlan> => {
  return invoke<OnboardingPlan>('skills_get_onboarding_plan');
};

export const importExistingSkill = async (
  sourcePath: string
): Promise<InstallResult> => {
  return invoke<InstallResult>('skills_import_existing', { sourcePath });
};

// Git Cache
export const getGitCacheCleanupDays = async (): Promise<number> => {
  return invoke<number>('skills_get_git_cache_cleanup_days');
};

export const setGitCacheCleanupDays = async (days: number): Promise<number> => {
  return invoke<number>('skills_set_git_cache_cleanup_days', { days });
};

export const getGitCacheTtlSecs = async (): Promise<number> => {
  return invoke<number>('skills_get_git_cache_ttl_secs');
};

export const clearGitCache = async (): Promise<number> => {
  return invoke<number>('skills_clear_git_cache');
};

export const getGitCachePath = async (): Promise<string> => {
  return invoke<string>('skills_get_git_cache_path');
};

// Preferred Tools
export const getPreferredTools = async (): Promise<string[] | null> => {
  return invoke<string[] | null>('skills_get_preferred_tools');
};

export const setPreferredTools = async (tools: string[]): Promise<void> => {
  return invoke('skills_set_preferred_tools', { tools });
};
