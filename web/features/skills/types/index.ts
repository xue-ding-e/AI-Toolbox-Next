// Skills feature types

export interface ManagedSkill {
  id: string;
  name: string;
  source_type: 'local' | 'git' | 'import';
  source_ref: string | null;
  central_path: string;
  created_at: number;
  updated_at: number;
  last_sync_at: number | null;
  status: string;
  targets: SkillTarget[];
}

export interface SkillTarget {
  tool: string;
  mode: string;
  status: string;
  target_path: string;
  synced_at: number | null;
}

export interface ToolInfo {
  key: string;
  label: string;
  installed: boolean;
}

export interface ToolStatus {
  tools: ToolInfo[];
  installed: string[];
  newly_installed: string[];
}

export interface InstallResult {
  skill_id: string;
  name: string;
  central_path: string;
  content_hash: string | null;
}

export interface SyncResult {
  mode_used: string;
  target_path: string;
}

export interface UpdateResult {
  skill_id: string;
  name: string;
  content_hash: string | null;
  source_revision: string | null;
  updated_targets: string[];
}

export interface GitSkillCandidate {
  name: string;
  description: string | null;
  subpath: string;
}

export interface OnboardingVariant {
  tool: string;
  name: string;
  path: string;
  fingerprint: string | null;
  is_link: boolean;
  link_target: string | null;
}

export interface OnboardingGroup {
  name: string;
  variants: OnboardingVariant[];
  has_conflict: boolean;
}

export interface OnboardingPlan {
  total_tools_scanned: number;
  total_skills_found: number;
  groups: OnboardingGroup[];
}

export interface ToolOption {
  id: string;
  label: string;
  installed: boolean;
}
