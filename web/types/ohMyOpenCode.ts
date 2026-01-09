/**
 * Oh My OpenCode Configuration Types
 * 
 * Type definitions for oh-my-opencode plugin configuration management.
 */

/**
 * Agent configuration
 */
export interface OhMyOpenCodeAgentConfig {
  model?: string;
  temperature?: number;
  top_p?: number;
  prompt?: string;
  prompt_append?: string;
  disable?: boolean;
  description?: string;
  mode?: 'subagent' | 'primary' | 'all';
  color?: string;
  [key: string]: unknown;
}

/**
 * Sisyphus agent specific configuration
 */
export interface OhMyOpenCodeSisyphusConfig {
  disabled?: boolean;
  default_builder_enabled?: boolean;
  planner_enabled?: boolean;
  replace_plan?: boolean;
}

/**
 * Agent types supported by oh-my-opencode
 */
export type OhMyOpenCodeAgentType = 
  | 'Sisyphus'
  | 'oracle'
  | 'librarian'
  | 'explore'
  | 'frontend-ui-ux-engineer'
  | 'document-writer'
  | 'multimodal-looker';

/**
 * Oh My OpenCode configuration stored in database
 */
export interface OhMyOpenCodeConfig {
  id: string;
  name: string;
  isApplied: boolean;
  schema?: string;
  agents: {
    Sisyphus?: OhMyOpenCodeAgentConfig;
    oracle?: OhMyOpenCodeAgentConfig;
    librarian?: OhMyOpenCodeAgentConfig;
    explore?: OhMyOpenCodeAgentConfig;
    'frontend-ui-ux-engineer'?: OhMyOpenCodeAgentConfig;
    'document-writer'?: OhMyOpenCodeAgentConfig;
    'multimodal-looker'?: OhMyOpenCodeAgentConfig;
  };
  sisyphusAgent?: OhMyOpenCodeSisyphusConfig;
  disabledAgents?: string[];
  disabledMcps?: string[];
  disabledHooks?: string[];
  disabledSkills?: string[];
  disabledCommands?: string[];
  createdAt?: string;
  updatedAt?: string;
}

/**
 * Form values for oh-my-opencode configuration modal
 */
export interface OhMyOpenCodeConfigFormValues {
  name: string;
  schema?: string;
  agents: {
    Sisyphus?: string;
    oracle?: string;
    librarian?: string;
    explore?: string;
    'frontend-ui-ux-engineer'?: string;
    'document-writer'?: string;
    'multimodal-looker'?: string;
  };
  sisyphusAgent?: OhMyOpenCodeSisyphusConfig;
  disabledAgents?: string[];
  disabledMcps?: string[];
  disabledHooks?: string[];
  disabledSkills?: string[];
  disabledCommands?: string[];
}

/**
 * Oh My OpenCode JSON file structure
 */
export interface OhMyOpenCodeJsonConfig {
  $schema?: string;
  agents?: {
    [key: string]: OhMyOpenCodeAgentConfig;
  };
  sisyphus_agent?: OhMyOpenCodeSisyphusConfig;
  disabled_agents?: string[];
  disabled_mcps?: string[];
  disabled_hooks?: string[];
  disabled_skills?: string[];
  disabled_commands?: string[];
}
