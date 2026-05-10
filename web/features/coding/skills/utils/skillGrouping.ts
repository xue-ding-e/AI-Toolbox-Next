import type { ManagedSkill, SkillGroup } from '../types';

export type SkillGroupingMode = 'custom' | 'source';

export const CUSTOM_UNGROUPED_GROUP_KEY = 'custom:__ungrouped__';

export interface SkillGroupLabels {
  groupLocal: string;
  groupImport: string;
  groupUngrouped: string;
}

export type GithubInfoResolver = (
  url: string | null | undefined,
) => { label: string; href: string } | null;

export function normalizeSkillMetadataText(value: string | null | undefined): string | null {
  const trimmed = value?.trim() ?? '';
  return trimmed ? trimmed : null;
}

export function getSkillGroupOptions(skills: ManagedSkill[]): string[] {
  const groups = new Set<string>();
  for (const skill of skills) {
    const group = normalizeSkillMetadataText(skill.user_group);
    if (group) {
      groups.add(group);
    }
  }
  return [...groups].sort((left, right) => left.localeCompare(right));
}

export function filterSkillsBySearch(skills: ManagedSkill[], searchText: string): ManagedSkill[] {
  const keyword = searchText.trim().toLowerCase();
  if (!keyword) {
    return skills;
  }

  return skills.filter((skill) => {
    const searchableValues = [
      skill.name,
      skill.source_ref,
      skill.user_group,
      skill.user_note,
    ];

    return searchableValues.some((value) => value?.toLowerCase().includes(keyword));
  });
}

export function buildSkillGroups(
  skills: ManagedSkill[],
  mode: SkillGroupingMode,
  labels: SkillGroupLabels,
  getGithubInfo: GithubInfoResolver,
): SkillGroup[] {
  const groupMap = new Map<string, SkillGroup>();

  for (const skill of skills) {
    const group = mode === 'custom'
      ? buildCustomGroup(skill, labels)
      : buildSourceGroup(skill, labels, getGithubInfo);

    const existing = groupMap.get(group.key);
    if (existing) {
      existing.skills.push(skill);
    } else {
      groupMap.set(group.key, { ...group, skills: [skill] });
    }
  }

  return Array.from(groupMap.values());
}

function buildCustomGroup(
  skill: ManagedSkill,
  labels: SkillGroupLabels,
): Omit<SkillGroup, 'skills'> {
  const userGroup = normalizeSkillMetadataText(skill.user_group);
  if (!userGroup) {
    return {
      key: CUSTOM_UNGROUPED_GROUP_KEY,
      label: labels.groupUngrouped,
      sourceType: 'custom',
    };
  }

  return {
    key: `custom:${userGroup}`,
    label: userGroup,
    sourceType: 'custom',
  };
}

function buildSourceGroup(
  skill: ManagedSkill,
  labels: SkillGroupLabels,
  getGithubInfo: GithubInfoResolver,
): Omit<SkillGroup, 'skills'> {
  if (skill.source_type === 'git' && skill.source_ref) {
    const github = getGithubInfo(skill.source_ref);
    if (github) {
      return {
        key: `git:${github.href}`,
        label: github.label,
        sourceType: 'git',
      };
    }

    const baseUrl = skill.source_ref.replace(/\/tree\/.*$/, '');
    return {
      key: `git:${baseUrl}`,
      label: baseUrl,
      sourceType: 'git',
    };
  }

  if (skill.source_type === 'local') {
    const path = skill.source_ref || '';
    const parts = path.split(/[\/\\]/).filter(Boolean);
    const parentPath = parts.slice(0, -1).join('/');
    return {
      key: `local:${parentPath || path}`,
      label: parts[parts.length - 2] || parts[parts.length - 1] || labels.groupLocal,
      sourceType: 'local',
    };
  }

  return {
    key: 'import',
    label: labels.groupImport,
    sourceType: 'import',
  };
}

export function getSkillToolIds(skill: ManagedSkill): string[] {
  return [...new Set(skill.targets.map((target) => target.tool))];
}

export function isSkillUngroupedCustomGroup(group: SkillGroup): boolean {
  return group.key === CUSTOM_UNGROUPED_GROUP_KEY;
}

export function getSkillGroupToolIds(group: SkillGroup): string[] {
  const toolIds = new Set<string>();
  for (const skill of group.skills) {
    for (const toolId of getSkillToolIds(skill)) {
      toolIds.add(toolId);
    }
  }
  return [...toolIds];
}

export function isSkillGroupToolsAligned(group: SkillGroup): boolean {
  if (group.skills.length <= 1) {
    return true;
  }

  const [firstSkill, ...restSkills] = group.skills;
  const firstToolKey = createToolSetKey(getSkillToolIds(firstSkill));
  return restSkills.every((skill) => createToolSetKey(getSkillToolIds(skill)) === firstToolKey);
}

export function getSkillIdsMissingTool(group: SkillGroup, toolId: string): string[] {
  return group.skills
    .filter((skill) => !skill.targets.some((target) => target.tool === toolId))
    .map((skill) => skill.id);
}

export function getSkillIdsWithTool(group: SkillGroup, toolId: string): string[] {
  return group.skills
    .filter((skill) => skill.targets.some((target) => target.tool === toolId))
    .map((skill) => skill.id);
}

function createToolSetKey(toolIds: string[]): string {
  return [...new Set(toolIds)].sort().join('\u0000');
}
