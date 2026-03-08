import { invoke } from '@tauri-apps/api/core';
import type {
  GlobalPromptConfig,
  GlobalPromptConfigInput,
} from '@/types/globalPrompt';

export interface GlobalPromptCommandSet {
  list: string;
  create: string;
  update: string;
  delete: string;
  apply: string;
  reorder: string;
  saveLocal: string;
}

export interface GlobalPromptApi {
  listConfigs: () => Promise<GlobalPromptConfig[]>;
  createConfig: (input: GlobalPromptConfigInput) => Promise<GlobalPromptConfig>;
  updateConfig: (input: GlobalPromptConfigInput) => Promise<GlobalPromptConfig>;
  deleteConfig: (id: string) => Promise<void>;
  applyConfig: (configId: string) => Promise<void>;
  reorderConfigs: (ids: string[]) => Promise<void>;
  saveLocalConfig: (input: GlobalPromptConfigInput) => Promise<GlobalPromptConfig>;
}

export const createGlobalPromptApi = (commands: GlobalPromptCommandSet): GlobalPromptApi => ({
  listConfigs: async () => {
    return await invoke<GlobalPromptConfig[]>(commands.list);
  },
  createConfig: async (input) => {
    return await invoke<GlobalPromptConfig>(commands.create, { input });
  },
  updateConfig: async (input) => {
    return await invoke<GlobalPromptConfig>(commands.update, { input });
  },
  deleteConfig: async (id) => {
    await invoke(commands.delete, { id });
  },
  applyConfig: async (configId) => {
    await invoke(commands.apply, { configId });
  },
  reorderConfigs: async (ids) => {
    await invoke(commands.reorder, { ids });
  },
  saveLocalConfig: async (input) => {
    return await invoke<GlobalPromptConfig>(commands.saveLocal, { input });
  },
});
