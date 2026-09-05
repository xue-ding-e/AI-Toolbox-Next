import { createGlobalPromptApi } from './globalPromptApi';

export const hermesPromptApi = createGlobalPromptApi({
  list: 'list_hermes_prompt_configs',
  create: 'create_hermes_prompt_config',
  update: 'update_hermes_prompt_config',
  delete: 'delete_hermes_prompt_config',
  apply: 'apply_hermes_prompt_config',
  disable: 'disable_hermes_prompt_config',
  reorder: 'reorder_hermes_prompt_configs',
  saveLocal: 'save_hermes_local_prompt_config',
});
