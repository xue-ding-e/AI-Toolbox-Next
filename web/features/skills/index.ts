// Skills Hub Feature
// Entry point for the skills management feature

// Pages
export { default as SkillsPage } from './pages/SkillsPage';

// Components
export { SkillsHubButton } from './components/SkillsHubButton';
export { SkillsHubModal } from './components/SkillsHubModal';
export { SkillCard } from './components/SkillCard';
export { SkillsList } from './components/SkillsList';
export { FilterBar } from './components/FilterBar';
export { ToolBadge } from './components/ToolBadge';

// Modals
export { AddSkillModal } from './components/modals/AddSkillModal';
export { GitPickModal } from './components/modals/GitPickModal';
export { DeleteConfirmModal } from './components/modals/DeleteConfirmModal';
export { ImportModal } from './components/modals/ImportModal';
export { NewToolsModal } from './components/modals/NewToolsModal';
export { SkillsSettingsModal } from './components/modals/SkillsSettingsModal';

// Hooks
export { useSkillsHub } from './hooks/useSkillsHub';
export { useToolStatus } from './hooks/useToolStatus';

// Store
export { useSkillsStore } from './stores/skillsStore';

// Types
export type {
  ManagedSkill,
  SkillTarget,
  ToolInfo,
  ToolStatus,
  GitSkillCandidate,
  OnboardingPlan,
  OnboardingGroup,
  OnboardingVariant,
  SortMode,
} from './types';

// API (for direct access if needed)
export * as skillsApi from './services/skillsApi';
