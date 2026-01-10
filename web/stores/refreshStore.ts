import { create } from 'zustand';

interface RefreshState {
  omoConfigRefreshKey: number;
  claudeProviderRefreshKey: number;
  openCodeConfigRefreshKey: number;
  incrementOmoConfigRefresh: () => void;
  incrementClaudeProviderRefresh: () => void;
  incrementOpenCodeConfigRefresh: () => void;
}

export const useRefreshStore = create<RefreshState>((set) => ({
  omoConfigRefreshKey: 0,
  claudeProviderRefreshKey: 0,
  openCodeConfigRefreshKey: 0,

  incrementOmoConfigRefresh: () =>
    set((state) => ({
      omoConfigRefreshKey: state.omoConfigRefreshKey + 1,
    })),

  incrementClaudeProviderRefresh: () =>
    set((state) => ({
      claudeProviderRefreshKey: state.claudeProviderRefreshKey + 1,
    })),

  incrementOpenCodeConfigRefresh: () =>
    set((state) => ({
      openCodeConfigRefreshKey: state.openCodeConfigRefreshKey + 1,
    })),
}));
