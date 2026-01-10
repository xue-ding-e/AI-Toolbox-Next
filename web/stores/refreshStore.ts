import { create } from 'zustand';

interface RefreshState {
  omoConfigRefreshKey: number;
  claudeProviderRefreshKey: number;
  incrementOmoConfigRefresh: () => void;
  incrementClaudeProviderRefresh: () => void;
}

export const useRefreshStore = create<RefreshState>((set) => ({
  omoConfigRefreshKey: 0,
  claudeProviderRefreshKey: 0,

  incrementOmoConfigRefresh: () =>
    set((state) => ({
      omoConfigRefreshKey: state.omoConfigRefreshKey + 1,
    })),

  incrementClaudeProviderRefresh: () =>
    set((state) => ({
      claudeProviderRefreshKey: state.claudeProviderRefreshKey + 1,
    })),
}));
