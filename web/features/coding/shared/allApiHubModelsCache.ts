import { getAllApiHubProviderModels, type AllApiHubProviderModelsResult } from '@/services/allApiHubApi';

export interface AllApiHubProviderModelsState {
  models: string[];
  status: 'idle' | 'loading' | 'loaded' | 'error' | 'unsupported';
  error?: string;
  updatedAt?: number;
}

const modelsCache = new Map<string, AllApiHubProviderModelsState>();
const inflightRequests = new Map<string, Promise<AllApiHubProviderModelsState>>();

const normalizeResult = (
  result: AllApiHubProviderModelsResult | undefined
): AllApiHubProviderModelsState => {
  if (!result) {
    return {
      models: [],
      status: 'error',
      error: 'No response returned',
      updatedAt: Date.now(),
    };
  }

  return {
    models: result.models || [],
    status: result.status,
    error: result.error,
    updatedAt: Date.now(),
  };
};

export const getCachedAllApiHubProviderModelsState = (
  providerId: string
): AllApiHubProviderModelsState | undefined => {
  return modelsCache.get(providerId);
};

export const refreshAllApiHubProviderModelsState = async (
  providerId: string
): Promise<AllApiHubProviderModelsState> => {
  const existing = inflightRequests.get(providerId);
  if (existing) {
    return existing;
  }

  const request = getAllApiHubProviderModels([providerId])
    .then((results) => {
      const nextState = normalizeResult(results[0]);
      modelsCache.set(providerId, nextState);
      return nextState;
    })
    .catch((error) => {
      const nextState: AllApiHubProviderModelsState = {
        models: modelsCache.get(providerId)?.models || [],
        status: 'error',
        error: error instanceof Error ? error.message : String(error),
        updatedAt: Date.now(),
      };
      modelsCache.set(providerId, nextState);
      return nextState;
    })
    .finally(() => {
      inflightRequests.delete(providerId);
    });

  inflightRequests.set(providerId, request);
  return request;
};

export const refreshAllApiHubProviderModelsInBackground = async (
  providerIds: string[],
  onResolved: (providerId: string, state: AllApiHubProviderModelsState) => void,
  concurrency: number = 4
): Promise<void> => {
  const queue = [...providerIds];
  const workerCount = Math.max(1, Math.min(concurrency, queue.length));

  await Promise.all(
    Array.from({ length: workerCount }, async () => {
      while (queue.length > 0) {
        const providerId = queue.shift();
        if (!providerId) {
          return;
        }

        const state = await refreshAllApiHubProviderModelsState(providerId);
        onResolved(providerId, state);
      }
    })
  );
};
