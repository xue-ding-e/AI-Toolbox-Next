import { invoke } from '@tauri-apps/api/core';
import type {
  OpenCodePromptConfig,
  OpenCodePromptConfigInput,
} from '@/types/openCodePrompt';

export const listOpenCodePromptConfigs = async (): Promise<OpenCodePromptConfig[]> => {
  return await invoke<OpenCodePromptConfig[]>('list_opencode_prompt_configs');
};

export const createOpenCodePromptConfig = async (
  input: OpenCodePromptConfigInput
): Promise<OpenCodePromptConfig> => {
  return await invoke<OpenCodePromptConfig>('create_opencode_prompt_config', { input });
};

export const updateOpenCodePromptConfig = async (
  input: OpenCodePromptConfigInput
): Promise<OpenCodePromptConfig> => {
  return await invoke<OpenCodePromptConfig>('update_opencode_prompt_config', { input });
};

export const deleteOpenCodePromptConfig = async (id: string): Promise<void> => {
  await invoke('delete_opencode_prompt_config', { id });
};

export const applyOpenCodePromptConfig = async (configId: string): Promise<void> => {
  await invoke('apply_opencode_prompt_config', { configId });
};

export const reorderOpenCodePromptConfigs = async (ids: string[]): Promise<void> => {
  await invoke('reorder_opencode_prompt_configs', { ids });
};

export const saveOpenCodeLocalPromptConfig = async (
  input: OpenCodePromptConfigInput
): Promise<OpenCodePromptConfig> => {
  return await invoke<OpenCodePromptConfig>('save_opencode_local_prompt_config', { input });
};
