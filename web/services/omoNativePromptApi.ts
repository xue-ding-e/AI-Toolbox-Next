import { createGlobalPromptApi } from './globalPromptApi';

export const omoNativePromptApi = createGlobalPromptApi({
  list: 'list_omo_native_prompt_configs',
  create: 'create_omo_native_prompt_config',
  update: 'update_omo_native_prompt_config',
  delete: 'delete_omo_native_prompt_config',
  apply: 'apply_omo_native_prompt_config',
  disable: 'disable_omo_native_prompt_config',
  reorder: 'reorder_omo_native_prompt_configs',
  saveLocal: 'save_omo_native_local_prompt_config',
});
