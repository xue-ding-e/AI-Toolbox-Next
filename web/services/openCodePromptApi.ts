import { createGlobalPromptApi } from './globalPromptApi';

export const openCodePromptApi = createGlobalPromptApi({
  list: 'list_opencode_prompt_configs',
  create: 'create_opencode_prompt_config',
  update: 'update_opencode_prompt_config',
  delete: 'delete_opencode_prompt_config',
  apply: 'apply_opencode_prompt_config',
  reorder: 'reorder_opencode_prompt_configs',
  saveLocal: 'save_opencode_local_prompt_config',
});

export const listOpenCodePromptConfigs = openCodePromptApi.listConfigs;
export const createOpenCodePromptConfig = openCodePromptApi.createConfig;
export const updateOpenCodePromptConfig = openCodePromptApi.updateConfig;
export const deleteOpenCodePromptConfig = openCodePromptApi.deleteConfig;
export const applyOpenCodePromptConfig = openCodePromptApi.applyConfig;
export const reorderOpenCodePromptConfigs = openCodePromptApi.reorderConfigs;
export const saveOpenCodeLocalPromptConfig = openCodePromptApi.saveLocalConfig;
