import { createGlobalPromptApi } from './globalPromptApi';

export const antigravityPromptApi = createGlobalPromptApi({
  list: 'list_antigravity_prompt_configs',
  create: 'create_antigravity_prompt_config',
  update: 'update_antigravity_prompt_config',
  delete: 'delete_antigravity_prompt_config',
  apply: 'apply_antigravity_prompt_config',
  disable: 'disable_antigravity_prompt_config',
  reorder: 'reorder_antigravity_prompt_configs',
  saveLocal: 'save_antigravity_local_prompt_config',
});
