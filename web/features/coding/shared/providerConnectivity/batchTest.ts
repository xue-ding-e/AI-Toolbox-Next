import type { ProviderConnectivityStatusItem } from '@/components/common/ProviderCard/types';
import { testProviderModelConnectivity, type ConnectivityTestRequest } from '@/services/opencodeApi';
import type { OpenCodeProvider } from '@/types/opencode';

export interface ProviderConnectivityInfo {
  providerId: string;
  providerName: string;
  providerConfig: OpenCodeProvider;
  modelIds: string[];
}

export interface ProviderConnectivityBatchTarget {
  providerId: string;
  request?: ConnectivityTestRequest;
  errorMessage?: string;
}

interface ProviderConnectivityBatchTargetOptions {
  requireBaseUrl?: boolean;
  requireApiKey?: boolean;
  prompt?: string;
  timeoutSecs?: number;
  errorMessages: {
    missingBaseUrl: string;
    missingApiKey: string;
    missingModel: string;
  };
}

const DEFAULT_CONNECTIVITY_PROMPT = 'say hi!';

export const CONNECTIVITY_BATCH_CONCURRENCY = 5;

export function buildProviderConnectivityBatchTarget(
  info: ProviderConnectivityInfo,
  options: ProviderConnectivityBatchTargetOptions,
): ProviderConnectivityBatchTarget {
  const providerOptions = info.providerConfig.options || {};
  const npm = info.providerConfig.npm || '@ai-sdk/openai-compatible';
  const baseUrl = providerOptions.baseURL?.trim() || '';
  const apiKey = providerOptions.apiKey?.trim();
  const modelId = info.modelIds[0];

  if (options.requireBaseUrl && !baseUrl) {
    return {
      providerId: info.providerId,
      errorMessage: options.errorMessages.missingBaseUrl,
    };
  }

  if (options.requireApiKey && !apiKey) {
    return {
      providerId: info.providerId,
      errorMessage: options.errorMessages.missingApiKey,
    };
  }

  if (!modelId) {
    return {
      providerId: info.providerId,
      errorMessage: options.errorMessages.missingModel,
    };
  }

  return {
    providerId: info.providerId,
    request: {
      npm,
      baseUrl,
      ...(apiKey ? { apiKey } : {}),
      prompt: options.prompt || DEFAULT_CONNECTIVITY_PROMPT,
      stream: true,
      modelIds: [modelId],
      timeoutSecs: options.timeoutSecs ?? 30,
    },
  };
}

export async function probeProviderConnectivity(
  target: ProviderConnectivityBatchTarget,
): Promise<ProviderConnectivityStatusItem> {
  if (!target.request) {
    return {
      status: 'error',
      errorMessage: target.errorMessage,
    };
  }

  try {
    const response = await testProviderModelConnectivity(target.request);
    const result = response.results[0];

    if (!result) {
      return {
        status: 'error',
        errorMessage: target.errorMessage || 'No test result returned',
      };
    }

    if (result.status === 'success') {
      return {
        status: 'success',
        modelId: result.modelId,
        totalMs: result.totalMs,
      };
    }

    return {
      status: 'error',
      modelId: result.modelId,
      totalMs: result.totalMs,
      errorMessage: result.errorMessage || `Connectivity test failed: ${result.status}`,
    };
  } catch (error) {
    return {
      status: 'error',
      errorMessage: error instanceof Error ? error.message : String(error),
    };
  }
}

export async function runProviderConnectivityBatch(
  targets: ProviderConnectivityBatchTarget[],
  onUpdate: (providerId: string, status: ProviderConnectivityStatusItem) => void,
  concurrency: number = CONNECTIVITY_BATCH_CONCURRENCY,
): Promise<void> {
  let currentIndex = 0;

  const worker = async () => {
    while (currentIndex < targets.length) {
      const target = targets[currentIndex];
      currentIndex += 1;

      const status = await probeProviderConnectivity(target);
      onUpdate(target.providerId, status);
    }
  };

  const workerCount = Math.min(Math.max(concurrency, 1), targets.length);
  await Promise.all(Array.from({ length: workerCount }, () => worker()));
}
