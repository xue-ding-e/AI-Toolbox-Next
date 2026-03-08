import { createGlobalPromptApi } from './globalPromptApi';

export const claudeCodePromptApi = createGlobalPromptApi({
  list: 'list_claude_prompt_configs',
  create: 'create_claude_prompt_config',
  update: 'update_claude_prompt_config',
  delete: 'delete_claude_prompt_config',
  apply: 'apply_claude_prompt_config',
  reorder: 'reorder_claude_prompt_configs',
  saveLocal: 'save_claude_local_prompt_config',
});
