import { createGlobalPromptApi } from './globalPromptApi';

export const piPromptApi = createGlobalPromptApi({
  list: 'list_pi_prompt_configs',
  create: 'create_pi_prompt_config',
  update: 'update_pi_prompt_config',
  delete: 'delete_pi_prompt_config',
  apply: 'apply_pi_prompt_config',
  reorder: 'reorder_pi_prompt_configs',
  saveLocal: 'save_pi_local_prompt_config',
});
