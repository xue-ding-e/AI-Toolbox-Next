import { create } from 'zustand';
import type {
  ManagedSkill,
  ToolStatus,
  ToolOption,
  OnboardingPlan,
  SortMode,
} from '../types';
import * as api from '../services/skillsApi';

interface SkillsState {
  // Data
  skills: ManagedSkill[];
  toolStatus: ToolStatus | null;
  onboardingPlan: OnboardingPlan | null;
  centralRepoPath: string;

  // UI state
  loading: boolean;
  error: string | null;
  searchQuery: string;
  sortMode: SortMode;

  // Modal state
  isHubModalOpen: boolean;
  isAddModalOpen: boolean;
  isImportModalOpen: boolean;
  isSettingsModalOpen: boolean;
  isNewToolsModalOpen: boolean;

  // Actions
  setHubModalOpen: (open: boolean) => void;
  setAddModalOpen: (open: boolean) => void;
  setImportModalOpen: (open: boolean) => void;
  setSettingsModalOpen: (open: boolean) => void;
  setNewToolsModalOpen: (open: boolean) => void;
  setSearchQuery: (query: string) => void;
  setSortMode: (mode: SortMode) => void;

  // Data actions
  loadToolStatus: () => Promise<void>;
  loadSkills: () => Promise<void>;
  loadOnboardingPlan: () => Promise<void>;
  loadCentralRepoPath: () => Promise<void>;
  refresh: () => Promise<void>;

  // Computed
  getInstalledTools: () => ToolOption[];
  getAllTools: () => ToolOption[];
  getFilteredSkills: () => ManagedSkill[];
}

export const useSkillsStore = create<SkillsState>()((set, get) => ({
  // Data
  skills: [],
  toolStatus: null,
  onboardingPlan: null,
  centralRepoPath: '',

  // UI state
  loading: false,
  error: null,
  searchQuery: '',
  sortMode: 'updated',

  // Modal state
  isHubModalOpen: false,
  isAddModalOpen: false,
  isImportModalOpen: false,
  isSettingsModalOpen: false,
  isNewToolsModalOpen: false,

  // Actions
  setHubModalOpen: (open) => set({ isHubModalOpen: open }),
  setAddModalOpen: (open) => set({ isAddModalOpen: open }),
  setImportModalOpen: (open) => set({ isImportModalOpen: open }),
  setSettingsModalOpen: (open) => set({ isSettingsModalOpen: open }),
  setNewToolsModalOpen: (open) => set({ isNewToolsModalOpen: open }),
  setSearchQuery: (query) => set({ searchQuery: query }),
  setSortMode: (mode) => set({ sortMode: mode }),

  // Data actions
  loadToolStatus: async () => {
    try {
      const status = await api.getToolStatus();
      set({ toolStatus: status });
    } catch (error) {
      console.error('Failed to load tool status:', error);
      set({ error: String(error) });
    }
  },

  loadSkills: async () => {
    set({ loading: true, error: null });
    try {
      const skills = await api.getManagedSkills();
      set({ skills, loading: false });
    } catch (error) {
      console.error('Failed to load skills:', error);
      set({ error: String(error), loading: false });
    }
  },

  loadOnboardingPlan: async () => {
    try {
      const plan = await api.getOnboardingPlan();
      set({ onboardingPlan: plan });
    } catch (error) {
      console.error('Failed to load onboarding plan:', error);
    }
  },

  loadCentralRepoPath: async () => {
    try {
      const path = await api.getCentralRepoPath();
      set({ centralRepoPath: path });
    } catch (error) {
      console.error('Failed to load central repo path:', error);
    }
  },

  refresh: async () => {
    const { loadToolStatus, loadSkills, loadOnboardingPlan, loadCentralRepoPath } = get();
    await Promise.all([
      loadToolStatus(),
      loadSkills(),
      loadOnboardingPlan(),
      loadCentralRepoPath(),
    ]);
  },

  // Computed
  getInstalledTools: () => {
    const { toolStatus } = get();
    if (!toolStatus) return [];
    return toolStatus.tools
      .filter((t) => t.installed)
      .map((t) => ({
        id: t.key,
        label: t.label,
        installed: t.installed,
      }));
  },

  getAllTools: () => {
    const { toolStatus } = get();
    if (!toolStatus) return [];
    return toolStatus.tools.map((t) => ({
      id: t.key,
      label: t.label,
      installed: t.installed,
    }));
  },

  getFilteredSkills: () => {
    const { skills, searchQuery, sortMode } = get();
    let filtered = skills;

    // Filter by search query
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      filtered = filtered.filter(
        (s) =>
          s.name.toLowerCase().includes(query) ||
          (s.source_ref && s.source_ref.toLowerCase().includes(query))
      );
    }

    // Sort
    if (sortMode === 'name') {
      filtered = [...filtered].sort((a, b) => a.name.localeCompare(b.name));
    } else {
      // 'updated' - already sorted by updated_at DESC from backend
    }

    return filtered;
  },
}));
