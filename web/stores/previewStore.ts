import { create } from 'zustand';

interface PreviewState {
  title: string;
  data: unknown;
  returnPath: string;
  setPreviewData: (title: string, data: unknown, returnPath: string) => void;
  clearPreviewData: () => void;
}

export const usePreviewStore = create<PreviewState>((set) => ({
  title: '',
  data: null,
  returnPath: '/',
  setPreviewData: (title, data, returnPath) => set({ title, data, returnPath }),
  clearPreviewData: () => set({ title: '', data: null, returnPath: '/' }),
}));
