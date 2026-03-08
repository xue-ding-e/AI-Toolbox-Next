export interface OpenCodePromptConfig {
  id: string;
  name: string;
  content: string;
  isApplied: boolean;
  sortIndex?: number;
  createdAt?: string;
  updatedAt?: string;
}

export interface OpenCodePromptConfigInput {
  id?: string;
  name: string;
  content: string;
}
