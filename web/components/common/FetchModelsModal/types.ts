/**
 * FetchModelsModal Types
 */

/** API type for fetching models */
export type ApiType = 'native' | 'openai_compat';

/** Fetched model info from API */
export interface FetchedModel {
  id: string;
  name?: string;
  ownedBy?: string;
  created?: number;
}

/** Response from fetch models API */
export interface FetchModelsResponse {
  models: FetchedModel[];
  total: number;
}

/** Props for FetchModelsModal component */
export interface FetchModelsModalProps {
  open: boolean;
  providerName: string;
  baseUrl: string;
  apiKey?: string;
  headers?: Record<string, string>;
  sdkType?: string;
  existingModelIds: string[];
  onCancel: () => void;
  onSuccess: (selectedModels: FetchedModel[]) => void;
}
