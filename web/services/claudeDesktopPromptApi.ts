import { createGlobalPromptApi } from './globalPromptApi';

export const claudeDesktopPromptApi = createGlobalPromptApi({
  list: 'list_claude_desktop_prompt_configs',
  create: 'create_claude_desktop_prompt_config',
  update: 'update_claude_desktop_prompt_config',
  delete: 'delete_claude_desktop_prompt_config',
  apply: 'apply_claude_desktop_prompt_config',
  disable: 'disable_claude_desktop_prompt_config',
  reorder: 'reorder_claude_desktop_prompt_configs',
  saveLocal: 'save_claude_desktop_local_prompt_config',
});