export interface GlobalPromptConfig {
  id: string;
  name: string;
  content: string;
  isApplied: boolean;
  sortIndex?: number;
  createdAt?: string;
  updatedAt?: string;
}

export interface GlobalPromptConfigInput {
  id?: string;
  name: string;
  content: string;
}
