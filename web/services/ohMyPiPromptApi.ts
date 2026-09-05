import { createGlobalPromptApi } from './globalPromptApi';

export const ohMyPiPromptApi = createGlobalPromptApi({
  list: 'list_omp_prompt_configs',
  create: 'create_omp_prompt_config',
  update: 'update_omp_prompt_config',
  delete: 'delete_omp_prompt_config',
  apply: 'apply_omp_prompt_config',
  disable: 'disable_omp_prompt_config',
  reorder: 'reorder_omp_prompt_configs',
  saveLocal: 'save_omp_local_prompt_config',
});