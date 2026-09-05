import { createGlobalPromptApi } from './globalPromptApi';

export const dshPromptApi = createGlobalPromptApi({
  list: 'list_dsh_prompt_configs',
  create: 'create_dsh_prompt_config',
  update: 'update_dsh_prompt_config',
  delete: 'delete_dsh_prompt_config',
  apply: 'apply_dsh_prompt_config',
  disable: 'disable_dsh_prompt_config',
  reorder: 'reorder_dsh_prompt_configs',
  saveLocal: 'save_dsh_local_prompt_config',
});