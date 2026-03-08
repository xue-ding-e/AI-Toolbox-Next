import { createGlobalPromptApi } from './globalPromptApi';

export const codexPromptApi = createGlobalPromptApi({
  list: 'list_codex_prompt_configs',
  create: 'create_codex_prompt_config',
  update: 'update_codex_prompt_config',
  delete: 'delete_codex_prompt_config',
  apply: 'apply_codex_prompt_config',
  reorder: 'reorder_codex_prompt_configs',
  saveLocal: 'save_codex_local_prompt_config',
});
