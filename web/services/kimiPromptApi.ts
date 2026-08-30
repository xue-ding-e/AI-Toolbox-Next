import { createGlobalPromptApi } from './globalPromptApi';

export const kimiPromptApi = createGlobalPromptApi({
  list: 'list_kimi_prompt_configs',
  create: 'create_kimi_prompt_config',
  update: 'update_kimi_prompt_config',
  delete: 'delete_kimi_prompt_config',
  apply: 'apply_kimi_prompt_config',
  reorder: 'reorder_kimi_prompt_configs',
  saveLocal: 'save_kimi_local_prompt_config',
});
