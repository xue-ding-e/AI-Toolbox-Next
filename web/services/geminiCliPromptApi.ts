import { createGlobalPromptApi } from './globalPromptApi';

export const geminiCliPromptApi = createGlobalPromptApi({
  list: 'list_gemini_cli_prompt_configs',
  create: 'create_gemini_cli_prompt_config',
  update: 'update_gemini_cli_prompt_config',
  delete: 'delete_gemini_cli_prompt_config',
  apply: 'apply_gemini_cli_prompt_config',
  reorder: 'reorder_gemini_cli_prompt_configs',
  saveLocal: 'save_gemini_cli_local_prompt_config',
});
