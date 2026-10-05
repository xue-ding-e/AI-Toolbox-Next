import { createGlobalPromptApi } from './globalPromptApi';

export const zcodePromptApi = createGlobalPromptApi({
  list: 'list_zcode_prompt_configs',
  create: 'create_zcode_prompt_config',
  update: 'update_zcode_prompt_config',
  delete: 'delete_zcode_prompt_config',
  apply: 'apply_zcode_prompt_config',
  disable: 'disable_zcode_prompt_config',
  reorder: 'reorder_zcode_prompt_configs',
  saveLocal: 'save_zcode_local_prompt_config',
});
